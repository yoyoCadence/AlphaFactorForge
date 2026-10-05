[CmdletBinding()]
param(
    [string]$Executable = (Join-Path $PSScriptRoot '../src-tauri/target/debug/alpha-factor-forge.exe'),
    [string]$ArtifactDirectory = (Join-Path $PSScriptRoot '../test-results/native-smoke')
)

$ErrorActionPreference = 'Stop'
$exe = Get-Item -LiteralPath $Executable
if ($exe.Name -ne 'alpha-factor-forge.exe' -or $exe.Directory.Name -ne 'debug') {
    throw 'The smoke requires the expected debug binary; release ignores the isolated registry seam.'
}
if (Get-Process -Name 'alpha-factor-forge' -ErrorAction SilentlyContinue) {
    throw 'An AlphaFactorForge desktop is already running; the smoke will not reuse or stop it.'
}
$nativeIdentity = [Security.Principal.WindowsIdentity]::GetCurrent()
try {
    $nativePrincipal = [Security.Principal.WindowsPrincipal]::new($nativeIdentity)
    $elevated = $nativePrincipal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
}
finally { $nativeIdentity.Dispose() }
$hostedRunner = $env:GITHUB_ACTIONS -eq 'true' -and $env:RUNNER_ENVIRONMENT -eq 'github-hosted'
if ($elevated -and -not $hostedRunner) {
    throw 'Run the local smoke from a non-elevated terminal; machine policy is only changed on a GitHub-hosted runner.'
}
$artifacts = [IO.Path]::GetFullPath($ArtifactDirectory)
New-Item -ItemType Directory -Path $artifacts -Force | Out-Null
$tempRoot = [IO.Path]::GetFullPath([IO.Path]::GetTempPath()).TrimEnd('\', '/')
$runId = [guid]::NewGuid().ToString('N')
$workspace = Join-Path $tempRoot "aff-native-smoke-$runId"
$registry = Join-Path $tempRoot "aff-native-registry-$runId"
$nativeProfileDir = Join-Path $tempRoot "aff-native-webview-$runId"
$ownedDirectories = @($workspace, $registry, $nativeProfileDir)
foreach ($directory in $ownedDirectories) {
    New-Item -ItemType Directory -Path $directory | Out-Null
}

$probe = [Net.Sockets.TcpListener]::new([Net.IPAddress]::Loopback, 0)
$probe.Start()
$port = $probe.LocalEndpoint.Port
$probe.Stop()
$variables = @('AFF_DATA_DIR', 'AFF_TEST_TRIAL_REGISTRY_DIR',
    'WEBVIEW2_USER_DATA_FOLDER', 'WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS')
$original = @{}
foreach ($name in $variables) {
    $original[$name] = [Environment]::GetEnvironmentVariable($name, 'Process')
}
$app = $null
$launch = @{}
$dbPath = Join-Path $workspace 'alphafactorforge.sqlite3'
$policyKey = $null
$policyChanged = $false
$hadPolicyValue = $false
try {
    $env:AFF_DATA_DIR = $workspace
    $env:AFF_TEST_TRIAL_REGISTRY_DIR = $registry
    $env:WEBVIEW2_USER_DATA_FOLDER = $nativeProfileDir
    $env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS = "--remote-debugging-port=$port --remote-debugging-address=127.0.0.1"
    # Elevated WebView2 hosts ignore environment/HKCU browser arguments:
    # https://learn.microsoft.com/en-us/microsoft-edge/webview2/concepts/security
    # Only the disposable hosted runner gets a temporary HKLM value for this
    # exact executable (never '*'). Preserve its original value/type in finally.
    if ($elevated) {
        $policyKey = [Microsoft.Win32.Registry]::LocalMachine.CreateSubKey(
            'SOFTWARE\Policies\Microsoft\Edge\WebView2\AdditionalBrowserArguments')
        $hadPolicyValue = $policyKey.GetValueNames() -contains $exe.Name
        if ($hadPolicyValue) {
            $oldPolicyValue = $policyKey.GetValue($exe.Name, $null,
                [Microsoft.Win32.RegistryValueOptions]::DoNotExpandEnvironmentNames)
            $oldPolicyKind = $policyKey.GetValueKind($exe.Name)
        }
        $policyKey.SetValue($exe.Name, $env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS,
            [Microsoft.Win32.RegistryValueKind]::String)
        $policyChanged = $true
    }
    if (Test-Path -LiteralPath $dbPath) { throw 'The smoke database must start absent.' }
    $launch = @{
        FilePath = $exe.FullName
        WindowStyle = 'Hidden'
        PassThru = $true
        RedirectStandardOutput = (Join-Path $artifacts 'native-stdout.log')
        RedirectStandardError = (Join-Path $artifacts 'native-stderr.log')
    }
    # PowerShell 7.4+ supports an explicit child environment; retain the existing
    # inherited-environment path for Windows PowerShell 5.
    if ((Get-Command Start-Process).Parameters.ContainsKey('Environment')) {
        $launch.Environment = @{}
        foreach ($name in $variables) {
            $launch.Environment[$name] = [Environment]::GetEnvironmentVariable($name, 'Process')
        }
    }
    $app = Start-Process @launch
    Write-Host "Native smoke PID $($app.Id), loopback CDP port $port, elevated=$elevated, hostedPolicy=$policyChanged"

    $deadline = [DateTime]::UtcNow.AddSeconds(45)
    $listeners = @()
    do {
        if ($app.HasExited) { throw "The app exited before readiness with code $($app.ExitCode)." }
        $listeners = @(Get-NetTCPConnection -LocalPort $port -State Listen -ErrorAction SilentlyContinue)
        # WebView2 can expose CDP before Tauri's user setup opens the database.
        # Both startup conditions must meet the same finite deadline.
        $readyFiles = @(@($dbPath, "$dbPath-wal") | Where-Object {
            (Test-Path -LiteralPath $_) -and (Get-Item -LiteralPath $_).Length -gt 0
        })
        if ($listeners.Count -gt 0 -and $readyFiles.Count -eq 2) { break }
        Start-Sleep -Milliseconds 200
    } while ([DateTime]::UtcNow -lt $deadline)
    if ($listeners.Count -eq 0) { throw 'The isolated WebView2 did not expose CDP within 45 seconds.' }
    foreach ($listener in $listeners) {
        if ($listener.LocalAddress -notin @('127.0.0.1', '::1')) { throw 'CDP must listen only on loopback.' }
        $owner = Get-CimInstance Win32_Process -Filter "ProcessId=$($listener.OwningProcess)"
        if ($owner.Name -ne 'msedgewebview2.exe' -or -not $owner.CommandLine.Contains($nativeProfileDir)) {
            throw 'CDP belongs to a different process/profile; refusing to attach.'
        }
    }
    foreach ($file in @($dbPath, "$dbPath-wal")) {
        if (-not (Test-Path -LiteralPath $file) -or (Get-Item -LiteralPath $file).Length -le 0) {
            throw "Startup did not create a non-empty SQLite/WAL file: $file"
        }
    }
    Write-Host 'Native launch ready: isolated CDP and non-empty SQLite/WAL.'
    & node (Join-Path $PSScriptRoot 'native-bridge-smoke.mjs') "http://127.0.0.1:$port" $artifacts
    if ($LASTEXITCODE -ne 0) { throw "Native bridge assertions failed with code $LASTEXITCODE." }
    if ($app.HasExited) { throw 'The app exited during the native bridge smoke.' }
}
catch {
    # Preserve useful pre-teardown state for CI failures without dumping process
    # environments or unrelated browser command lines.
    $startupWebviews = @(Get-CimInstance Win32_Process -Filter "Name='msedgewebview2.exe'" |
        Where-Object {
            ($_.CommandLine -and $_.CommandLine.Contains($nativeProfileDir)) -or
            ($app -and $_.ParentProcessId -eq $app.Id)
        } | ForEach-Object {
            @{
                pid = $_.ProcessId
                parentPid = $_.ParentProcessId
                profileMatches = [bool]($_.CommandLine -and $_.CommandLine.Contains($nativeProfileDir))
                debugPortMatches = [bool]($_.CommandLine -and $_.CommandLine.Contains("--remote-debugging-port=$port"))
            }
        })
    $diagnostic = @{
        error = $_.Exception.Message
        appAlive = [bool]($app -and -not $app.HasExited)
        databaseExists = [bool](Test-Path -LiteralPath $dbPath)
        walExists = [bool](Test-Path -LiteralPath "$dbPath-wal")
        webviews = $startupWebviews
        explicitChildEnvironment = $launch.ContainsKey('Environment')
        elevated = $elevated
        hostedRunner = $hostedRunner
        hostedPolicy = $policyChanged
    }
    $diagnostic | ConvertTo-Json -Depth 4 | Set-Content -LiteralPath (Join-Path $artifacts 'native-launch-failure.json')
    Write-Host ($diagnostic | ConvertTo-Json -Depth 4 -Compress)
    throw
}
finally {
    foreach ($name in $variables) {
        [Environment]::SetEnvironmentVariable($name, $original[$name], 'Process')
    }
    $policyRestoreError = $null
    if ($null -ne $policyKey) {
        try {
            if ($policyChanged) {
                if ($hadPolicyValue) {
                    $policyKey.SetValue($exe.Name, $oldPolicyValue, $oldPolicyKind)
                }
                else { $policyKey.DeleteValue($exe.Name, $false) }
                Write-Host 'Native smoke restored the exact hosted-runner browser policy value.'
            }
        }
        catch { $policyRestoreError = $_ }
        finally { $policyKey.Dispose() }
    }
    # Snapshot descendants before stopping the app: utility/render children
    # may omit the profile argument and become orphaned when their parent exits.
    $allWebviews = @(Get-CimInstance Win32_Process -Filter "Name='msedgewebview2.exe'")
    $ownedWebviews = @($allWebviews | Where-Object { $_.CommandLine -and $_.CommandLine.Contains($nativeProfileDir) })
    do {
        $ownedIds = @($ownedWebviews | ForEach-Object { $_.ProcessId })
        $children = @($allWebviews | Where-Object {
            $_.ParentProcessId -in $ownedIds -and $_.ProcessId -notin $ownedIds
        })
        $ownedWebviews += $children
    } while ($children.Count -gt 0)
    if ($null -ne $app -and -not $app.HasExited) {
        $owner = Get-CimInstance Win32_Process -Filter "ProcessId=$($app.Id)"
        if ($owner.ExecutablePath -ne $exe.FullName) { throw 'App PID changed identity; refusing to stop it.' }
        Stop-Process -Id $app.Id -Force -ErrorAction SilentlyContinue
        $app.WaitForExit(5000) | Out-Null
    }
    foreach ($webview in $ownedWebviews) {
        $current = Get-CimInstance Win32_Process -Filter "ProcessId=$($webview.ProcessId)"
        if ($current -and $current.CreationDate -eq $webview.CreationDate -and
            $current.CommandLine -eq $webview.CommandLine) {
            Stop-Process -Id $webview.ProcessId -Force -ErrorAction SilentlyContinue
            Wait-Process -Id $webview.ProcessId -Timeout 5 -ErrorAction SilentlyContinue
        }
    }
    foreach ($directory in $ownedDirectories) {
        if (Test-Path -LiteralPath $directory) {
            $resolved = (Resolve-Path -LiteralPath $directory).Path
            if (-not $resolved.StartsWith("$tempRoot\", [StringComparison]::OrdinalIgnoreCase) -or
                [IO.Path]::GetFileName($resolved) -notlike "aff-native-*-$runId") {
                throw 'Refusing to delete a path outside this smoke run in the temp directory.'
            }
            # WebView can release file handles just after process exit. A finite
            # retry keeps cleanup real without treating a locked profile as success.
            $cleanupDeadline = [DateTime]::UtcNow.AddSeconds(5)
            do {
                try {
                    Remove-Item -LiteralPath $resolved -Recurse -Force
                    break
                }
                catch [IO.IOException] {
                    if ([DateTime]::UtcNow -ge $cleanupDeadline) { throw }
                    Start-Sleep -Milliseconds 200
                }
            } while ($true)
        }
    }
    if ($null -ne $policyRestoreError) { throw $policyRestoreError }
}
Write-Host 'Native smoke cleanup complete: owned processes stopped and temp directories removed.'

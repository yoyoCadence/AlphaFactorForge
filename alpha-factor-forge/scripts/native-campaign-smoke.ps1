[CmdletBinding()]
param(
    [string]$Settings = (Join-Path $PSScriptRoot '../fixtures/binance/listing-btcusdt-settings-v1.json'),
    [string]$SourceFile = (Join-Path $PSScriptRoot '../fixtures/binance/listing-usdt-announcement-excerpt.txt'),
    [string]$ServiceExecutable = (Join-Path $PSScriptRoot '../src-tauri/target/debug/alpha-factor-forge-service.exe'),
    [string]$Executable = (Join-Path $PSScriptRoot '../src-tauri/target/debug/alpha-factor-forge.exe'),
    [string]$ArtifactDirectory = (Join-Path $PSScriptRoot '../node_modules/.cache/native-campaign-smoke')
)

# A live, one-instrument admission smoke. The supplied original evidence is
# never rewritten. No user workspace, registry, credential or machine policy
# is opened; every CLI owner and desktop launch uses this run's temp paths.
$ErrorActionPreference = 'Stop'
$exe = Get-Item -LiteralPath $Executable
$serviceExe = Get-Item -LiteralPath $ServiceExecutable
if ($exe.Name -ne 'alpha-factor-forge.exe' -or $exe.Directory.Name -ne 'debug' -or
    $serviceExe.Name -ne 'alpha-factor-forge-service.exe' -or $serviceExe.Directory.Name -ne 'debug') {
    throw 'Expected debug app/service binaries; release does not support the isolated trial registry.'
}
if (Get-Process -Name 'alpha-factor-forge' -ErrorAction SilentlyContinue) {
    throw 'An existing desktop is running; this smoke will not reuse or stop it.'
}
$nativeIdentity = [Security.Principal.WindowsIdentity]::GetCurrent()
try {
    $principal = [Security.Principal.WindowsPrincipal]::new($nativeIdentity)
    if ($principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
        throw 'Run this local smoke from a non-elevated terminal; it never changes WebView2 machine policy.'
    }
}
finally { $nativeIdentity.Dispose() }
$settingsFile = Get-Item -LiteralPath $Settings
$source = Get-Item -LiteralPath $SourceFile
$settingsDocument = Get-Content -LiteralPath $settingsFile.FullName -Raw -Encoding UTF8 | ConvertFrom-Json
if ($settingsDocument.instrumentId -ne 'crypto:binance:BTCUSDT') {
    throw 'This bounded smoke supports one BTCUSDT campaign only.'
}
$declaredSource = [string]$settingsDocument.rawFile
if (-not [IO.Path]::IsPathRooted($declaredSource)) {
    $declaredSource = Join-Path $settingsFile.Directory.FullName $declaredSource
}
if ((Get-Item -LiteralPath $declaredSource).FullName -ne $source.FullName) {
    throw 'SourceFile must be the exact rawFile named by the supplied settings.'
}
$artifacts = [IO.Path]::GetFullPath($ArtifactDirectory)
New-Item -ItemType Directory -Path $artifacts -Force | Out-Null
$tempRoot = [IO.Path]::GetFullPath([IO.Path]::GetTempPath()).TrimEnd('\', '/')
$runId = [guid]::NewGuid().ToString('N')
$workspace = Join-Path $tempRoot "aff-campaign-workspace-$runId"
$registry = Join-Path $tempRoot "aff-campaign-registry-$runId"
$nativeProfileDir = Join-Path $tempRoot "aff-campaign-webview-$runId"
$ownedDirectories = @($workspace, $registry, $nativeProfileDir)
foreach ($directory in $ownedDirectories) { New-Item -ItemType Directory -Path $directory | Out-Null }
$variables = @('AFF_DATA_DIR', 'AFF_TEST_TRIAL_REGISTRY_DIR',
    'WEBVIEW2_USER_DATA_FOLDER', 'WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS')
$original = @{}
foreach ($name in $variables) { $original[$name] = [Environment]::GetEnvironmentVariable($name, 'Process') }
$script:campaignApp = $null
$script:campaignCli = $null
$script:campaignCliIdentity = $null
$script:campaignAppIdentity = $null
$script:campaignPort = $null
$script:campaignPorts = @()
$script:campaignAppIds = @()
$dbPath = Join-Path $workspace 'alphafactorforge.sqlite3'

function Quote-NativeArgument([string]$Value) {
    # Windows CreateProcess quoting, including trailing backslashes. These
    # arguments contain file paths and declared market identifiers, no secrets.
    '"' + ([regex]::Replace([regex]::Replace($Value, '(\\*)"', '$1$1\"'), '(\\+)$', '$1$1')) + '"'
}

function Invoke-OwnedCli([string]$Name, [string[]]$Arguments) {
    $stdout = Join-Path $artifacts "$Name.json"
    $stderr = Join-Path $artifacts "$Name-stderr.log"
    $argsQuoted = @($Arguments | ForEach-Object { Quote-NativeArgument $_ })
    $script:campaignCli = Start-Process -FilePath $serviceExe.FullName -ArgumentList $argsQuoted -WindowStyle Hidden -PassThru -RedirectStandardOutput $stdout -RedirectStandardError $stderr
    $script:campaignCliIdentity = Get-CimInstance Win32_Process -Filter "ProcessId=$($script:campaignCli.Id)"
    $deadline = [DateTime]::UtcNow.AddSeconds(90)
    while (-not $script:campaignCli.WaitForExit(200)) {
        if ([DateTime]::UtcNow -ge $deadline) {
            $current = Get-CimInstance Win32_Process -Filter "ProcessId=$($script:campaignCli.Id)"
            if ($current -and $current.ExecutablePath -eq $serviceExe.FullName -and $current.CreationDate -eq $script:campaignCliIdentity.CreationDate) {
                Stop-Process -Id $script:campaignCli.Id -Force
                $script:campaignCli.WaitForExit(5000) | Out-Null
            }
            throw "$Name exceeded its 90-second deadline."
        }
    }
    $script:campaignCli.WaitForExit()
    $exitCode = $script:campaignCli.ExitCode
    $script:campaignCli = $null
    if ($exitCode -ne 0) { throw "$Name failed with exit $exitCode; see its captured stderr." }
    Get-Content -LiteralPath $stdout -Raw -Encoding UTF8 | ConvertFrom-Json
}

function Assert-Fetch($Report) {
    if ($Report.outcome -ne 'snapshot' -or $Report.bars -ne 744 -or
        $Report.rangeCoverage.blocking -ne $false -or $Report.coverage.blocking -ne $false -or
        $Report.status -ne 'ok' -or $Report.kind -ne 'historical' -or
        $Report.costProfileVersion -ne 'native-campaign-smoke-cost-v1') {
        throw 'Official fetch did not produce the declared clean 744-bar historical snapshot.'
    }
}

function Stop-OwnedDesktop {
    # Find profile owners and their descendants before the app exits; some
    # WebView utility children omit the profile argument after launch.
    $all = @(Get-CimInstance Win32_Process -Filter "Name='msedgewebview2.exe'")
    $owned = @($all | Where-Object { $_.CommandLine -and $_.CommandLine.Contains($nativeProfileDir) })
    do {
        $ids = @($owned | ForEach-Object { $_.ProcessId })
        $children = @($all | Where-Object { $_.ParentProcessId -in $ids -and $_.ProcessId -notin $ids })
        $owned += $children
    } while ($children.Count -gt 0)
    if ($script:campaignApp -and -not $script:campaignApp.HasExited) {
        $current = Get-CimInstance Win32_Process -Filter "ProcessId=$($script:campaignApp.Id)"
        if ($current.ExecutablePath -ne $exe.FullName -or
            $current.CreationDate -ne $script:campaignAppIdentity.CreationDate) {
            throw 'App PID changed identity; refusing to stop it.'
        }
        Stop-Process -Id $script:campaignApp.Id -Force -ErrorAction SilentlyContinue
        $script:campaignApp.WaitForExit(5000) | Out-Null
    }
    foreach ($webview in $owned) {
        $current = Get-CimInstance Win32_Process -Filter "ProcessId=$($webview.ProcessId)"
        if ($current -and $current.CreationDate -eq $webview.CreationDate -and $current.CommandLine -eq $webview.CommandLine) {
            Stop-Process -Id $webview.ProcessId -Force -ErrorAction SilentlyContinue
            Wait-Process -Id $webview.ProcessId -Timeout 5 -ErrorAction SilentlyContinue
        }
    }
    $deadline = [DateTime]::UtcNow.AddSeconds(5)
    do {
        $listener = if ($script:campaignPort) { Get-NetTCPConnection -LocalPort $script:campaignPort -State Listen -ErrorAction SilentlyContinue }
        if (-not $listener) { break }
        Start-Sleep -Milliseconds 100
    } while ([DateTime]::UtcNow -lt $deadline)
    if ($listener) {
        throw 'The owned CDP port remains open after process teardown.'
    }
    $script:campaignApp = $null
    $script:campaignAppIdentity = $null
}

function Start-OwnedDesktop([string]$Phase) {
    $probe = [Net.Sockets.TcpListener]::new([Net.IPAddress]::Loopback, 0)
    $probe.Start()
    $script:campaignPort = $probe.LocalEndpoint.Port
    $script:campaignPorts += $script:campaignPort
    $probe.Stop()
    $env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS = "--remote-debugging-port=$script:campaignPort --remote-debugging-address=127.0.0.1"
    $launch = @{
        FilePath = $exe.FullName; WindowStyle = 'Hidden'; PassThru = $true
        RedirectStandardOutput = (Join-Path $artifacts "$Phase-stdout.log")
        RedirectStandardError = (Join-Path $artifacts "$Phase-stderr.log")
    }
    if ((Get-Command Start-Process).Parameters.ContainsKey('Environment')) {
        $launch.Environment = @{}
        foreach ($name in $variables) { $launch.Environment[$name] = [Environment]::GetEnvironmentVariable($name, 'Process') }
    }
    $script:campaignApp = Start-Process @launch
    $script:campaignAppIds += $script:campaignApp.Id
    $script:campaignAppIdentity = Get-CimInstance Win32_Process -Filter "ProcessId=$($script:campaignApp.Id)"
    Write-Host "Campaign $Phase PID $($script:campaignApp.Id), loopback CDP $script:campaignPort"
    $deadline = [DateTime]::UtcNow.AddSeconds(45)
    $listeners = @()
    do {
        if ($script:campaignApp.HasExited) { throw "Desktop exited before $Phase readiness." }
        $listeners = @(Get-NetTCPConnection -LocalPort $script:campaignPort -State Listen -ErrorAction SilentlyContinue)
        $readyFiles = @(@($dbPath, "$dbPath-wal") | Where-Object { (Test-Path -LiteralPath $_) -and (Get-Item -LiteralPath $_).Length -gt 0 })
        if ($listeners.Count -gt 0 -and $readyFiles.Count -eq 2) { break }
        Start-Sleep -Milliseconds 200
    } while ([DateTime]::UtcNow -lt $deadline)
    if ($listeners.Count -eq 0 -or $readyFiles.Count -ne 2) { throw "Desktop $Phase did not expose its isolated CDP/SQLite within 45 seconds." }
    foreach ($listener in $listeners) {
        $owner = Get-CimInstance Win32_Process -Filter "ProcessId=$($listener.OwningProcess)"
        if ($listener.LocalAddress -notin @('127.0.0.1', '::1') -or $owner.Name -ne 'msedgewebview2.exe' -or
            -not $owner.CommandLine.Contains($nativeProfileDir)) {
            throw 'CDP is not this run''s loopback WebView2/profile; refusing to attach.'
        }
    }
    & node (Join-Path $PSScriptRoot 'native-campaign-smoke.mjs') "http://127.0.0.1:$script:campaignPort" $artifacts (Join-Path $artifacts 'campaign-input.json') $Phase
    if ($LASTEXITCODE -ne 0) { throw "Native campaign $Phase assertions failed with exit $LASTEXITCODE." }
    if ($script:campaignApp.HasExited) { throw "Desktop exited during $Phase assertions." }
}

try {
    $env:AFF_DATA_DIR = $workspace
    $env:AFF_TEST_TRIAL_REGISTRY_DIR = $registry
    $env:WEBVIEW2_USER_DATA_FOLDER = $nativeProfileDir
    if (Test-Path -LiteralPath $dbPath) { throw 'Isolated workspace must start absent.' }
    $fetchArguments = @('fetch', '--instrument', 'crypto:binance:BTCUSDT', '--interval', '1h',
        '--from', '2025-01-01', '--to', '2025-02-01', '--no-rest',
        '--cost-profile', 'native-campaign-smoke-cost-v1', '--json', '--data-dir', $workspace)
    $oldFetch = Invoke-OwnedCli 'old-fetch' $fetchArguments
    Assert-Fetch $oldFetch
    $registrationArguments = @('register-crypto-listing', '--settings', $settingsFile.FullName, '--data-dir', $workspace)
    $receipt = Invoke-OwnedCli 'registration' $registrationArguments
    $retry = Invoke-OwnedCli 'registration-retry' $registrationArguments
    $expectedListing = [DateTimeOffset]::Parse($settingsDocument.listedFrom).ToUnixTimeMilliseconds()
    if ($receipt.version -ne 'crypto-listing-registration-v1' -or $receipt.instrumentId -ne 'crypto:binance:BTCUSDT' -or
        $receipt.created -ne $true -or $retry.created -ne $false -or
        $receipt.rowId -eq $oldFetch.instrumentRowId -or $receipt.listedFrom -ne $expectedListing -or
        $receipt.rawSha256 -ne $settingsDocument.rawSha256) { throw 'Registration did not append the asserted listing revision.' }
    foreach ($key in @('version', 'instrumentId', 'rowId', 'revision', 'contentHash', 'listedFrom', 'rawSha256')) {
        if ($receipt.$key -ne $retry.$key) { throw "Exact registration retry changed $key." }
    }
    $newFetch = Invoke-OwnedCli 'new-fetch' $fetchArguments
    Assert-Fetch $newFetch
    if ($newFetch.instrumentRowId -ne $receipt.rowId -or $newFetch.snapshotId -eq $oldFetch.snapshotId -or
        $newFetch.datasetId -ne $oldFetch.datasetId -or $newFetch.datasetHash -ne $oldFetch.datasetHash) {
        throw 'Re-fetch must preserve the dataset and pin a distinct snapshot to the new instrument revision.'
    }
    @{
        scope = 'one BTCUSDT admission/exploration smoke; no research PASS or calibrated trading claim'
        oldFetch = $oldFetch; receipt = $receipt; retry = $retry; newFetch = $newFetch
        expectedListing = $expectedListing
        sourceFile = $source.FullName
        binaries = @{
            appSha256 = (Get-FileHash -LiteralPath $exe.FullName -Algorithm SHA256).Hash.ToLowerInvariant()
            serviceSha256 = (Get-FileHash -LiteralPath $serviceExe.FullName -Algorithm SHA256).Hash.ToLowerInvariant()
        }
    } | ConvertTo-Json -Depth 30 | Set-Content -LiteralPath (Join-Path $artifacts 'campaign-input.json') -Encoding UTF8
    Start-OwnedDesktop 'initial'
    Stop-OwnedDesktop
    Start-OwnedDesktop 'reopen'
    Stop-OwnedDesktop
    Write-Host 'Native campaign passed: official fetch, listing revision/retry, UI admission/run/history and process reopen.'
}
catch {
    @{ error = $_.Exception.Message; appAlive = [bool]($script:campaignApp -and -not $script:campaignApp.HasExited) } |
        ConvertTo-Json | Set-Content -LiteralPath (Join-Path $artifacts 'launcher-failure.json') -Encoding UTF8
    throw
}
finally {
    foreach ($name in $variables) { [Environment]::SetEnvironmentVariable($name, $original[$name], 'Process') }
    if ($script:campaignCli -and -not $script:campaignCli.HasExited) {
        $current = Get-CimInstance Win32_Process -Filter "ProcessId=$($script:campaignCli.Id)"
        if ($current.ExecutablePath -ne $serviceExe.FullName -or
            $current.CreationDate -ne $script:campaignCliIdentity.CreationDate) {
            throw 'CLI PID changed identity; refusing to stop it.'
        }
        Stop-Process -Id $script:campaignCli.Id -Force -ErrorAction SilentlyContinue
        $script:campaignCli.WaitForExit(5000) | Out-Null
    }
    Stop-OwnedDesktop
    foreach ($directory in $ownedDirectories) {
        if (Test-Path -LiteralPath $directory) {
            $resolved = (Resolve-Path -LiteralPath $directory).Path
            if (-not $resolved.StartsWith("$tempRoot\", [StringComparison]::OrdinalIgnoreCase) -or
                [IO.Path]::GetFileName($resolved) -notlike "aff-campaign-*-$runId") {
                throw 'Refusing to delete a path outside this run''s owned temp directory.'
            }
            $deadline = [DateTime]::UtcNow.AddSeconds(5)
            do {
                try { Remove-Item -LiteralPath $resolved -Recurse -Force; break }
                catch [IO.IOException] {
                    if ([DateTime]::UtcNow -ge $deadline) { throw }
                    Start-Sleep -Milliseconds 200
                }
            } while ($true)
        }
    }
    $remainingListeners = @($script:campaignPorts | ForEach-Object {
        Get-NetTCPConnection -LocalPort $_ -State Listen -ErrorAction SilentlyContinue
    })
    $remainingApps = @($script:campaignAppIds | ForEach-Object {
        Get-CimInstance Win32_Process -Filter "ProcessId=$_" | Where-Object { $_.ExecutablePath -eq $exe.FullName }
    })
    $remainingWebviews = @(Get-CimInstance Win32_Process -Filter "Name='msedgewebview2.exe'" |
        Where-Object { $_.CommandLine -and $_.CommandLine.Contains($nativeProfileDir) })
    $directoryChecks = @($ownedDirectories | ForEach-Object { @{ path = $_; exists = [bool](Test-Path -LiteralPath $_) } })
    @{
        runId = $runId; ownedDirectories = $directoryChecks; cdpPorts = $script:campaignPorts
        ownedAppPids = $script:campaignAppIds; remainingListeners = $remainingListeners.Count
        remainingApps = $remainingApps.Count; remainingWebviews = $remainingWebviews.Count
        environmentRestored = $true
    } | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath (Join-Path $artifacts 'cleanup.json') -Encoding UTF8
    if ($remainingListeners.Count -gt 0 -or $remainingApps.Count -gt 0 -or $remainingWebviews.Count -gt 0 -or
        @($directoryChecks | Where-Object { $_.exists }).Count -gt 0) { throw 'Owned campaign smoke resources remain after cleanup.' }
}
Write-Host 'Native campaign cleanup complete: owned processes/ports stopped and temp directories removed.'

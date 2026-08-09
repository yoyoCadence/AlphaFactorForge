param([string]$ScriptHome = $PSScriptRoot, [string]$OutputRoot = '')
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

Add-Type -AssemblyName PresentationCore
Add-Type -AssemblyName PresentationFramework
Add-Type -AssemblyName WindowsBase

$RepoRoot = (Resolve-Path (Join-Path $ScriptHome '..\..\..\..')).Path
$RunRoot = Split-Path -Parent $ScriptHome
$CaptureRoot = Join-Path $RunRoot 'raw-captures'
$ShowcaseRoot = if ($OutputRoot) { $OutputRoot } else { Join-Path $RepoRoot 'showcase' }
New-Item -ItemType Directory -Force -Path $ShowcaseRoot | Out-Null

$Width = 2400
$Height = 1350
$Dpi = 96.0
$Culture = [System.Globalization.CultureInfo]::GetCultureInfo('zh-TW')
$TypefaceSans = [System.Windows.Media.Typeface]::new('Microsoft JhengHei UI')
$TypefaceSansBold = [System.Windows.Media.Typeface]::new('Microsoft JhengHei UI Bold')
$TypefaceMono = [System.Windows.Media.Typeface]::new('Cascadia Mono')

function Brush([string]$Hex) {
  return [System.Windows.Media.BrushConverter]::new().ConvertFromString($Hex)
}

$Ink = Brush '#171A1D'
$PaperSoft = Brush '#FFFDF7'
$Orange = Brush '#FF6B1A'
$Teal = Brush '#2C8178'
$Muted = Brush '#6D6A62'
$White = Brush '#FFFFFF'
$Shadow = Brush '#48171A1D'

function Load-Bitmap([string]$Path) {
  $bitmap = [System.Windows.Media.Imaging.BitmapImage]::new()
  $bitmap.BeginInit()
  $bitmap.CacheOption = [System.Windows.Media.Imaging.BitmapCacheOption]::OnLoad
  $bitmap.UriSource = [Uri]::new((Resolve-Path $Path).Path)
  $bitmap.EndInit()
  $bitmap.Freeze()
  return $bitmap
}

function Crop-Bitmap($Bitmap, [int]$X, [int]$Y, [int]$W, [int]$H) {
  $crop = [System.Windows.Media.Imaging.CroppedBitmap]::new($Bitmap, [System.Windows.Int32Rect]::new($X, $Y, $W, $H))
  $crop.Freeze()
  return $crop
}

function Text-Block($Dc, [string]$Text, [double]$X, [double]$Y, [double]$Size, $Color, [double]$MaxWidth, $Typeface = $TypefaceSans, [double]$LineHeight = 0) {
  $ft = [System.Windows.Media.FormattedText]::new($Text, $Culture, [System.Windows.FlowDirection]::LeftToRight, $Typeface, $Size, $Color, $Dpi / 96.0)
  $ft.MaxTextWidth = $MaxWidth
  if ($LineHeight -gt 0) { $ft.LineHeight = $LineHeight }
  $Dc.DrawText($ft, [System.Windows.Point]::new($X, $Y))
  return $ft.Height
}

function Draw-Rule($Dc, [double]$X1, [double]$Y1, [double]$X2, [double]$Y2, $Color, [double]$Thickness = 2) {
  $Dc.DrawLine([System.Windows.Media.Pen]::new($Color, $Thickness), [System.Windows.Point]::new($X1, $Y1), [System.Windows.Point]::new($X2, $Y2))
}

function Draw-RoundRect($Dc, [double]$X, [double]$Y, [double]$W, [double]$H, $Fill, $Stroke = $null, [double]$Radius = 14, [double]$Thickness = 2) {
  $pen = if ($null -ne $Stroke) { [System.Windows.Media.Pen]::new($Stroke, $Thickness) } else { $null }
  $Dc.DrawRoundedRectangle($Fill, $pen, [System.Windows.Rect]::new($X, $Y, $W, $H), $Radius, $Radius)
}

function Draw-Chip($Dc, [string]$Text, [double]$X, [double]$Y, [double]$W, $Fill, $TextColor = $Ink) {
  Draw-RoundRect $Dc $X $Y $W 52 $Fill $Ink 4 2
  Text-Block $Dc $Text ($X + 18) ($Y + 13) 20 $TextColor ($W - 36) $TypefaceMono | Out-Null
}

function Draw-ImagePlate($Dc, $Bitmap, [double]$X, [double]$Y, [double]$W, [double]$H, [string]$Label) {
  Draw-RoundRect $Dc ($X + 18) ($Y + 22) $W $H $Shadow $null 12 0
  Draw-RoundRect $Dc ($X - 14) ($Y - 54) ($W + 28) ($H + 68) $PaperSoft $Ink 8 3
  Draw-RoundRect $Dc ($X + 18) ($Y - 38) 650 34 $Ink $null 2 0
  Text-Block $Dc $Label ($X + 34) ($Y - 33) 16 $White 620 $TypefaceMono | Out-Null
  $clip = [System.Windows.Media.RectangleGeometry]::new([System.Windows.Rect]::new($X, $Y, $W, $H), 5, 5)
  $Dc.PushClip($clip)
  $Dc.DrawImage($Bitmap, [System.Windows.Rect]::new($X, $Y, $W, $H))
  $Dc.Pop()
  $Dc.DrawRoundedRectangle($null, [System.Windows.Media.Pen]::new($Ink, 3), [System.Windows.Rect]::new($X, $Y, $W, $H), 5, 5)
}

function Draw-CertificateBase($Dc, $Background, [string]$Folio, [string]$Section, [string]$Index) {
  $Dc.DrawImage($Background, [System.Windows.Rect]::new(0, 0, $Width, $Height))
  $Dc.DrawRectangle((Brush '#D9FFFDF7'), $null, [System.Windows.Rect]::new(0, 0, $Width, $Height))
  $Dc.DrawRectangle($Orange, $null, [System.Windows.Rect]::new(1885, 0, 12, $Height))
  Draw-Rule $Dc 112 92 2288 92 $Ink 3
  Draw-Rule $Dc 112 1262 2288 1262 $Ink 3
  Text-Block $Dc 'ALPHAFACTORFORGE' 112 34 25 $Ink 560 $TypefaceMono | Out-Null
  Text-Block $Dc $Section 870 37 18 $Muted 700 $TypefaceMono | Out-Null
  Text-Block $Dc $Folio 1918 34 18 $Ink 340 $TypefaceMono | Out-Null
  Draw-RoundRect $Dc 2184 26 104 44 $Ink $null 3 0
  Text-Block $Dc $Index 2208 34 20 $White 70 $TypefaceMono | Out-Null
  Text-Block $Dc 'DEMO STATE · SAMPLE 1H · 600 BARS · NON-PRODUCTION DATA' 112 1280 17 $Muted 1300 $TypefaceMono | Out-Null
  Text-Block $Dc 'EVIDENCE DOSSIER / 2026-08-09' 1910 1280 17 $Ink 380 $TypefaceMono | Out-Null
}

function Save-Canvas([string]$OutputPath, [scriptblock]$Draw) {
  $visual = [System.Windows.Media.DrawingVisual]::new()
  $dc = $visual.RenderOpen()
  try { & $Draw $dc } finally { $dc.Close() }
  $target = [System.Windows.Media.Imaging.RenderTargetBitmap]::new($Width, $Height, $Dpi, $Dpi, [System.Windows.Media.PixelFormats]::Pbgra32)
  $target.Render($visual)
  $encoder = [System.Windows.Media.Imaging.PngBitmapEncoder]::new()
  $encoder.Frames.Add([System.Windows.Media.Imaging.BitmapFrame]::Create($target))
  $stream = [System.IO.File]::Open($OutputPath, [System.IO.FileMode]::Create)
  try { $encoder.Save($stream) } finally { $stream.Dispose() }
}

$Background = Load-Bitmap (Join-Path $ScriptHome 'mill-test-report-background.png')
$Overview = Load-Bitmap (Join-Path $CaptureRoot 'overview-chart-results-existing.png')
$OverviewInputs = Load-Bitmap (Join-Path $CaptureRoot 'overview-existing.png')
$Holdout = Load-Bitmap (Join-Path $CaptureRoot 'holdout-results-existing.png')
$Sweep = Load-Bitmap (Join-Path $CaptureRoot 'sweep-2d-existing.png')
$ChartCrop = Crop-Bitmap $Overview 15 64 1698 540

Save-Canvas (Join-Path $ShowcaseRoot '01-hero.png') {
  param($dc)
  Draw-CertificateBase $dc $Background 'MTR-AFF-001' 'PRODUCT PROOF / CORE WORKFLOW' '01'
  Text-Block $dc '把訊號鍛成' 116 154 92 $Ink 980 $TypefaceSansBold 112 | Out-Null
  Text-Block $dc '可驗證的證據' 116 256 92 $Ink 980 $TypefaceSansBold 112 | Out-Null
  $dc.DrawRectangle($Orange, $null, [System.Windows.Rect]::new(120, 385, 300, 13))
  Text-Block $dc '從 K 線、策略與執行假設，到樣本外檢驗、參數掃描與可追溯結果。' 120 430 30 $Ink 690 $TypefaceSans 46 | Out-Null
  Draw-Chip $dc 'LOCAL-FIRST WORKSTATION' 120 574 370 $PaperSoft $Ink
  Draw-Chip $dc 'REAL UI / ISOLATED DEMO' 120 646 370 $Teal $White
  Draw-Chip $dc 'HOLDOUT + PARAM SWEEP' 120 718 370 $Orange $Ink
  Draw-ImagePlate $dc $Overview 824 334 1430 924 'EVIDENCE 01 · CURRENT WORKSPACE'
  Draw-RoundRect $dc 120 850 512 250 $PaperSoft $Ink 6 3
  Text-Block $dc '檢驗重點' 152 878 18 $Muted 180 $TypefaceMono | Out-Null
  Text-Block $dc '輸入、假設、切分、結果' 152 924 30 $Ink 430 $TypefaceSansBold | Out-Null
  Text-Block $dc '同一個工作區，讓策略假設接受可重現的檢查。' 152 982 23 $Ink 420 $TypefaceSans 36 | Out-Null
}

Save-Canvas (Join-Path $ShowcaseRoot '02-holdout-proof.png') {
  param($dc)
  Draw-CertificateBase $dc $Background 'MTR-AFF-002' 'VALIDATION RECORD / HOLDOUT' '02'
  Text-Block $dc '先保留未見資料，' 116 158 82 $Ink 1120 $TypefaceSansBold 102 | Out-Null
  Text-Block $dc '再談好成績' 116 255 82 $Ink 1120 $TypefaceSansBold 102 | Out-Null
  Text-Block $dc 'Holdout 將末段 K 線保留為樣本外；全期、樣本內、樣本外並列檢視。' 120 388 29 $Ink 840 $TypefaceSans 45 | Out-Null
  $dc.DrawEllipse($PaperSoft, [System.Windows.Media.Pen]::new($Teal, 11), [System.Windows.Point]::new(365, 702), 194, 194)
  Text-Block $dc '30%' 238 622 74 $Ink 260 $TypefaceMono | Out-Null
  Text-Block $dc '樣本外保留' 242 718 28 $Teal 260 $TypefaceSansBold | Out-Null
  Draw-Rule $dc 560 704 720 704 $Teal 7
  Text-Block $dc 'NOT USED FOR TUNING' 120 952 20 $Muted 520 $TypefaceMono | Out-Null
  Draw-ImagePlate $dc $Holdout 690 472 1560 640 'EVIDENCE 02 · FULL / IN-SAMPLE / OUT-OF-SAMPLE'
  Text-Block $dc '這是檢查過度擬合的工具，不是消除過度擬合的保證。' 690 1142 21 $Muted 1450 $TypefaceSans | Out-Null
}

Save-Canvas (Join-Path $ShowcaseRoot '03-in-sample-sweep.png') {
  param($dc)
  Draw-CertificateBase $dc $Background 'MTR-AFF-003' 'OPTIMIZATION RECORD / PARAM SWEEP' '03'
  Text-Block $dc '掃描參數，' 116 158 84 $Ink 900 $TypefaceSansBold 104 | Out-Null
  Text-Block $dc '但別偷看樣本外' 116 258 84 $Ink 980 $TypefaceSansBold 104 | Out-Null
  Text-Block $dc '可掃描 1–2 個參數、最多 256 組；開啟 Holdout 時只用樣本內資料找最佳值。' 120 394 28 $Ink 540 $TypefaceSans 44 | Out-Null
  Draw-RoundRect $dc 120 565 520 110 $PaperSoft $Ink 5 3
  $dc.DrawRectangle($Orange, $null, [System.Windows.Rect]::new(144, 596, 326, 24))
  $dc.DrawRectangle($Teal, $null, [System.Windows.Rect]::new(470, 596, 142, 24))
  Text-Block $dc '70% 樣本內' 144 632 19 $Ink 250 $TypefaceMono | Out-Null
  Text-Block $dc '30% 保留' 458 632 19 $Teal 160 $TypefaceMono | Out-Null
  Draw-RoundRect $dc 120 736 310 194 $Ink $null 5 0
  Text-Block $dc '256' 160 750 82 $White 220 $TypefaceMono | Out-Null
  Text-Block $dc 'MAX COMBINATIONS' 160 852 17 $Orange 250 $TypefaceMono | Out-Null
  Draw-ImagePlate $dc $Sweep 720 360 1540 668 'EVIDENCE 03 · 2D HEATMAP / IN-SAMPLE ONLY'
  Text-Block $dc '熱力圖顯示歷史結果；最佳格仍需要樣本外驗證。' 720 1065 23 $Muted 1420 $TypefaceSans | Out-Null
}

Save-Canvas (Join-Path $ShowcaseRoot '04-bar-replay.png') {
  param($dc)
  Draw-CertificateBase $dc $Background 'MTR-AFF-004' 'INSPECTION RECORD / BAR REPLAY' '04'
  Text-Block $dc '逐根回放，' 116 158 86 $Ink 860 $TypefaceSansBold 106 | Out-Null
  Text-Block $dc '重看訊號怎麼發生' 116 260 86 $Ink 1120 $TypefaceSansBold 106 | Out-Null
  Text-Block $dc '滑桿、逐步與 1×–4× 播放，把圖表限制在當時可見的 K 線，並顯示進出場訊號與持倉。' 120 398 27 $Ink 520 $TypefaceSans 43 | Out-Null
  Draw-ImagePlate $dc $ChartCrop 708 438 1540 490 'EVIDENCE 04 · AUTHENTIC CHART CROP'
  Draw-Rule $dc 146 1012 2198 1012 $Ink 4
  $points = @(220, 650, 1080, 1510, 1940)
  foreach ($px in $points) { $dc.DrawEllipse($PaperSoft, [System.Windows.Media.Pen]::new($Ink, 4), [System.Windows.Point]::new($px, 1012), 13, 13) }
  $dc.DrawEllipse($Orange, $null, [System.Windows.Point]::new(1080, 1012), 17, 17)
  Text-Block $dc '301 / 600' 960 1052 27 $Ink 240 $TypefaceMono | Out-Null
  Text-Block $dc '◀  STEP  ·  PLAY 1×–4×  ·  STEP  ▶' 720 1134 24 $Teal 930 $TypefaceMono | Out-Null
  Text-Block $dc '未來 K 線隱藏' 1740 1052 24 $Muted 420 $TypefaceSansBold | Out-Null
}

Save-Canvas (Join-Path $ShowcaseRoot '05-result-context-export.png') {
  param($dc)
  Draw-CertificateBase $dc $Background 'MTR-AFF-005' 'AUDIT RECORD / RESULT CONTEXT' '05'
  Text-Block $dc '每次結果，' 116 154 86 $Ink 800 $TypefaceSansBold 106 | Out-Null
  Text-Block $dc '只屬於當時的輸入' 116 258 68 $Ink 620 $TypefaceSansBold 88 | Out-Null
  Text-Block $dc '資料集、策略或 Holdout 一變，先前結果立即失效；重新回測後才能儲存或匯出。' 120 374 27 $Ink 590 $TypefaceSans 42 | Out-Null
  $chain = @(
    @{Y=570; K='DATASET'; V='K 線與資料身分'},
    @{Y=690; K='STRATEGY'; V='訊號與參數快照'},
    @{Y=810; K='RANGE'; V='全期或 Holdout 切分'},
    @{Y=930; K='RESULT'; V='績效、交易與匯出'}
  )
  foreach ($item in $chain) {
    Draw-RoundRect $dc 120 $item.Y 500 82 $PaperSoft $Ink 4 3
    Text-Block $dc $item.K 148 ($item.Y + 15) 19 $Orange 160 $TypefaceMono | Out-Null
    Text-Block $dc $item.V 300 ($item.Y + 13) 24 $Ink 290 $TypefaceSansBold | Out-Null
    if ($item.Y -lt 930) { Text-Block $dc '↓' 344 ($item.Y + 82) 30 $Teal 40 $TypefaceSansBold | Out-Null }
  }
  Draw-Chip $dc 'EXPORT · JSON' 120 1086 238 $Ink $White
  Draw-Chip $dc 'TRADES · CSV' 380 1086 238 $Teal $White
  Draw-ImagePlate $dc $OverviewInputs 850 326 1370 886 'EVIDENCE 05 · INPUTS AND COMPLETED RUN'
}

$Width = 1200
$Height = 900
Save-Canvas (Join-Path $ShowcaseRoot 'thumbnail.png') {
  param($dc)
  $dc.DrawImage($Background, [System.Windows.Rect]::new(0, 0, $Width, $Height))
  $dc.DrawRectangle((Brush '#D9FFFDF7'), $null, [System.Windows.Rect]::new(0, 0, $Width, $Height))
  $dc.DrawRectangle($Orange, $null, [System.Windows.Rect]::new(1054, 0, 10, $Height))
  Draw-Rule $dc 62 56 1138 56 $Ink 3
  Text-Block $dc 'ALPHAFACTORFORGE' 62 18 20 $Ink 500 $TypefaceMono | Out-Null
  Text-Block $dc '把訊號鍛成' 62 100 59 $Ink 720 $TypefaceSansBold 74 | Out-Null
  Text-Block $dc '可驗證的證據' 62 170 59 $Ink 720 $TypefaceSansBold 74 | Out-Null
  Draw-ImagePlate $dc $Overview 72 346 1022 476 'AUTHENTIC PRODUCT UI'
  $dc.DrawEllipse($PaperSoft, [System.Windows.Media.Pen]::new($Teal, 8), [System.Windows.Point]::new(984, 220), 92, 92)
  Text-Block $dc 'OOS' 942 190 34 $Teal 96 $TypefaceMono | Out-Null
  Text-Block $dc 'PROOF' 936 232 18 $Ink 108 $TypefaceMono | Out-Null
  Text-Block $dc 'HOLDOUT · SWEEP · REPLAY · EXPORT' 62 850 17 $Muted 760 $TypefaceMono | Out-Null
}

Copy-Item -LiteralPath (Join-Path $ShowcaseRoot '01-hero.png') -Destination (Join-Path $ShowcaseRoot 'hero.png') -Force

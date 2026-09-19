# Run only in the disposable Windows QA guest, in tester's interactive session.
$ErrorActionPreference = 'Stop'
$qaRoot = 'C:\Users\tester\PeakRunner-Launcher-QA-20260919'
New-Item -ItemType Directory -Force $qaRoot | Out-Null
Start-Transcript -Path (Join-Path $qaRoot 'qa.log') -Force
try {
    if (-not (Test-Path (Join-Path $qaRoot 'payload'))) {
        Expand-Archive (Join-Path $qaRoot 'payload.zip') (Join-Path $qaRoot 'payload')
    }
    $payload = Join-Path $qaRoot 'payload'
    $env:PEAKRUNNER_LAUNCHER_DATA_DIR = Join-Path $qaRoot 'store'
    $env:QA_CAPTURE_PATH = Join-Path $qaRoot 'launcher.png'
    & (Join-Path $payload 'launcher-release.exe') install-local (Join-Path $payload 'feed\windows-x64\latest.json') (Join-Path $payload 'feed\blobs') $env:PEAKRUNNER_LAUNCHER_DATA_DIR
    if ($LASTEXITCODE -ne 0) { throw "Signed install failed: $LASTEXITCODE" }
    & (Join-Path $payload 'smoke.exe')
    if ($LASTEXITCODE -ne 0) { throw "Launcher render failed: $LASTEXITCODE" }
    $play = Start-Process (Join-Path $payload 'play_smoke.exe') -PassThru -RedirectStandardOutput (Join-Path $qaRoot 'play.log') -RedirectStandardError (Join-Path $qaRoot 'play-error.log')
    # Retain the native process handle before it exits (PowerShell 5.1).
    $playHandle = $play.Handle
    Start-Sleep -Seconds 5
    Add-Type -AssemblyName System.Windows.Forms
    Add-Type -AssemblyName System.Drawing
    $bounds = [System.Windows.Forms.Screen]::PrimaryScreen.Bounds
    $bitmap = New-Object System.Drawing.Bitmap $bounds.Width, $bounds.Height
    $graphics = [System.Drawing.Graphics]::FromImage($bitmap)
    try {
        $graphics.CopyFromScreen($bounds.Location, [System.Drawing.Point]::Empty, $bounds.Size)
        $bitmap.Save((Join-Path $qaRoot 'game.png'), [System.Drawing.Imaging.ImageFormat]::Png)
    } finally { $graphics.Dispose(); $bitmap.Dispose() }
    $play.WaitForExit()
    if ($play.ExitCode -ne 0) { throw "Managed game launch failed: $($play.ExitCode)" }
    'PASS: signed install, launcher render, managed game launch and update lock' | Set-Content (Join-Path $qaRoot 'result.txt')
} catch {
    $_ | Out-String | Set-Content (Join-Path $qaRoot 'result.txt')
    throw
} finally { Stop-Transcript }

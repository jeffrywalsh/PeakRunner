# Run only inside the disposable PeakRunner Windows test guest.
$ErrorActionPreference = 'Stop'
$qaRoot = Join-Path $env:USERPROFILE 'PeakRunner-QA'
New-Item -ItemType Directory -Force $qaRoot | Out-Null
Start-Transcript -Path (Join-Path $qaRoot 'setup.log') -Append
try {
    $release = '0.1.0-raindance.20260919.1'
    $archive = "PeakRunner-$release-windows-x64.zip"
    $zip = Join-Path $qaRoot $archive
    Invoke-WebRequest "https://peakrunner.net/downloads/$archive" -OutFile $zip
    if ((Get-FileHash $zip -Algorithm SHA256).Hash -ne '5B5ECCC1DD5817468CEBAD61A56448CF42E8D1FBD4C4E0EF312C2B094F1E40B4') {
        throw 'Downloaded release checksum mismatch'
    }
    $releaseDir = Join-Path $qaRoot $release
    if (-not (Test-Path $releaseDir)) { Expand-Archive $zip $releaseDir }
    $client = Get-ChildItem $releaseDir -Recurse -Filter peakrunner.exe | Select-Object -First 1
    if (-not $client) { throw 'Client missing from archive' }
    $shortcut = (New-Object -ComObject WScript.Shell).CreateShortcut((Join-Path ([Environment]::GetFolderPath('Desktop')) 'PeakRunner.lnk'))
    $shortcut.TargetPath = $client.FullName
    $shortcut.WorkingDirectory = $client.DirectoryName
    $shortcut.Save()
    Write-Output 'Verified release downloaded. Launch PeakRunner from the desktop.'
} finally {
    Stop-Transcript
}

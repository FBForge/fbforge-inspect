$ErrorActionPreference = 'Stop'
$root = Join-Path $env:RUNNER_TEMP ('fbforge-nvda-' + [Guid]::NewGuid().ToString('N'))
$null = New-Item -ItemType Directory -Path $root
$download = Join-Path $root 'nvda_2026.2.exe'
Invoke-WebRequest 'https://github.com/nvaccess/nvda/releases/download/release-2026.2/nvda_2026.2.exe' -OutFile $download
if ((Get-FileHash -LiteralPath $download -Algorithm SHA256).Hash.ToLowerInvariant() -ne 'f3f8d29974a88d687b3c4809be192219ec579c5bdabcda5aaf53635288bca824') { throw 'nvda_download_integrity_failed' }
$portable = Join-Path $root 'portable'
$setup = Start-Process -FilePath $download -ArgumentList ('--minimal --create-portable-silent --portable-path="' + $portable + '"') -PassThru
if (-not $setup.WaitForExit(180000) -or $setup.ExitCode -ne 0) { throw 'nvda_portable_creation_failed' }
if (-not (Test-Path -LiteralPath (Join-Path $portable 'nvda_noUIAccess.exe'))) { throw 'nvda_portable_missing' }
Add-Content -Path $env:GITHUB_ENV -Value "INSPECT_NVDA_PATH=$(Join-Path $portable 'nvda_noUIAccess.exe')"

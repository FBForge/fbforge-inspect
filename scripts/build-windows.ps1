param(
  [string]$InspectJavaHome = $env:JAVA_HOME,
  [string]$InspectWorkDirectory = (Join-Path ([IO.Path]::GetTempPath()) ('fbforge-inspect-' + [Guid]::NewGuid().ToString('N')))
)
$ErrorActionPreference = 'Stop'
$inspectSource = Split-Path $PSScriptRoot -Parent
if (!$InspectJavaHome) { throw 'OpenJDK 25 mit GPL/Classpath Exception erforderlich. -InspectJavaHome oder JAVA_HOME angeben.' }
$inspectJava = Join-Path $InspectJavaHome 'bin\java.exe'
if (!(Test-Path $inspectJava)) { throw 'Windows JDK 25 fehlt. -InspectJavaHome angeben.' }
$inspectJavaVersion = & $inspectJava --version | Out-String
if ($inspectJavaVersion -notmatch ' 25\.') { throw 'JDK 25 erforderlich.' }
$inspectLicense = Get-Content (Join-Path $InspectJavaHome 'legal\java.base\LICENSE') -Raw
$inspectAdditional = Get-Content (Join-Path $InspectJavaHome 'legal\java.base\ADDITIONAL_LICENSE_INFO') -Raw
if ($inspectLicense -notmatch 'GNU General Public License' -or $inspectAdditional -notmatch 'Classpath') { throw 'OpenJDK mit GPL/Classpath Exception erforderlich, damit Laufzeit und Distributionshinweise übereinstimmen.' }
if (!(Test-Path (Join-Path $inspectSource 'engine\target\inspect-engine-0.1.0.jar'))) { throw 'Zuerst den Java-Kern mit mvn -f engine/pom.xml package bauen und prüfen.' }
New-Item -ItemType Directory -Force $InspectWorkDirectory | Out-Null
$inspectWorkRoot = (Resolve-Path $InspectWorkDirectory).Path
Write-Output "Windows build workspace: $inspectWorkRoot"
foreach ($inspectFolder in @('frontend','desktop','scripts','docs','assets','licenses','tests')) {
  $inspectTarget = Join-Path $inspectWorkRoot $inspectFolder
  # Build trees, local session files and IDE data must never enter the staging directory.
  & robocopy (Join-Path $inspectSource $inspectFolder) $inspectTarget /E /XD target node_modules resources gen .local android ios /NFL /NDL /NJH /NJS /NP | Out-Null
  if ($LASTEXITCODE -ge 8) { throw "Kopieren von $inspectFolder fehlgeschlagen." }
}
foreach ($inspectFile in @('package.json','package-lock.json','tsconfig.json','LICENSE','THIRD_PARTY_NOTICES.md')) { Copy-Item (Join-Path $inspectSource $inspectFile) $inspectWorkRoot }
New-Item -ItemType Directory -Force (Join-Path $inspectWorkRoot 'engine\target') | Out-Null
Copy-Item (Join-Path $inspectSource 'engine\target\inspect-engine-0.1.0.jar') (Join-Path $inspectWorkRoot 'engine\target')
$env:PATH = (Join-Path $InspectJavaHome 'bin') + ';' + $env:PATH
$env:JAVA_HOME = $InspectJavaHome
Push-Location $inspectWorkRoot
try {
  & npm.cmd ci --ignore-scripts --no-audit --no-fund --registry=https://registry.npmjs.org
  if ($LASTEXITCODE) { throw 'npm ci fehlgeschlagen.' }
  & npm.cmd run typecheck
  if ($LASTEXITCODE) { throw 'Typecheck fehlgeschlagen.' }
  & cargo.exe fetch --locked --manifest-path desktop/Cargo.toml --target x86_64-pc-windows-msvc
  if ($LASTEXITCODE) { throw 'Rust-Abhängigkeiten konnten nicht aufgelöst werden.' }
  & npm.cmd run licenses:collect
  if ($LASTEXITCODE) { throw 'Lizenzinventar konnte nicht erstellt werden.' }
  & npm.cmd run stage:engine
  if ($LASTEXITCODE) { throw 'Windows-Laufzeit konnte nicht bereitgestellt werden.' }
  # Verify that the bundled runtime executes our engine without an installed Java requirement.
  $inspectProbe = '{"protocolVersion":1,"operation":"schema","environment":"TEST","apiVersion":"v3","input":{"text":"{\"type\":\"object\"}"}}'
  $inspectProbeResult = $inspectProbe | & '.\desktop\resources\engine\runtime\bin\java.exe' -jar '.\desktop\resources\engine\inspect-engine.jar'
  if ($LASTEXITCODE -or !($inspectProbeResult | ConvertFrom-Json).ok) { throw 'Windows-Laufzeitprobe fehlgeschlagen.' }
  Push-Location desktop
  try { & node.exe '..\node_modules\@tauri-apps\cli\tauri.js' build --ci --no-binary-patching; if ($LASTEXITCODE) { throw 'Tauri-Windows-Build fehlgeschlagen.' } }
  finally { Pop-Location }
  $inspectArtifacts = Join-Path $inspectSource '.local\artifacts\windows-x64'
  New-Item -ItemType Directory -Force $inspectArtifacts | Out-Null
  Copy-Item '.\desktop\target\release\fbforge-inspect.exe' $inspectArtifacts
  $inspectEngineArtifacts = Join-Path $inspectArtifacts 'engine'
  & robocopy '.\desktop\target\release\engine' $inspectEngineArtifacts /MIR /NFL /NDL /NJH /NJS /NP | Out-Null
  if ($LASTEXITCODE -ge 8) { throw 'Kopieren der gebündelten Laufzeit fehlgeschlagen.' }
  Copy-Item '.\desktop\target\release\bundle\nsis\*.exe' $inspectArtifacts
  Copy-Item LICENSE,THIRD_PARTY_NOTICES.md $inspectArtifacts
  $inspectHashes = Get-ChildItem $inspectArtifacts -File -Recurse | Where-Object Name -ne 'SHA256SUMS.txt' | Sort-Object FullName | Get-FileHash -Algorithm SHA256 | ForEach-Object { $_.Hash.ToLower() + '  ' + $_.Path.Substring($inspectArtifacts.Length + 1).Replace('\', '/') }
  [IO.File]::WriteAllLines((Join-Path $inspectArtifacts 'SHA256SUMS.txt'), $inspectHashes, (New-Object Text.UTF8Encoding($false)))
  Write-Output "Unsigned Windows build: $inspectArtifacts"
  Write-Output "Build workspace retained for review: $inspectWorkRoot"
} finally { Pop-Location }
# Robocopy codes 0 through 7 are successful. Do not let a retained code 1
# turn a completed build into a GitHub PowerShell wrapper failure.
$global:LASTEXITCODE = 0

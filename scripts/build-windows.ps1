param(
  [string]$InspectJavaHome = $env:JAVA_HOME,
  [string]$InspectWorkDirectory = (Join-Path ([IO.Path]::GetTempPath()) ('fbforge-inspect-' + [Guid]::NewGuid().ToString('N'))),
  [string]$InspectSigningThumbprint,
  [string]$InspectTimestampUrl = 'http://timestamp.digicert.com',
  [string]$InspectArtifactDirectory
)
$ErrorActionPreference = 'Stop'
$inspectSource = Split-Path $PSScriptRoot -Parent
Import-Module (Join-Path $PSScriptRoot 'windows-signing.psm1') -Force
if ($InspectSigningThumbprint -and (Test-Path -LiteralPath $InspectWorkDirectory)) { throw 'signing_work_directory_must_be_new' }
if (!$InspectJavaHome) { throw 'OpenJDK 25 mit GPL/Classpath Exception erforderlich. -InspectJavaHome oder JAVA_HOME angeben.' }
$inspectJava = Join-Path $InspectJavaHome 'bin\java.exe'
if (!(Test-Path $inspectJava)) { throw 'Windows JDK 25 fehlt. -InspectJavaHome angeben.' }
$inspectJavaVersion = & $inspectJava --version | Out-String
if ($inspectJavaVersion -notmatch ' 25\.') { throw 'JDK 25 erforderlich.' }
$inspectLicense = Get-Content (Join-Path $InspectJavaHome 'legal\java.base\LICENSE') -Raw
$inspectAdditional = Get-Content (Join-Path $InspectJavaHome 'legal\java.base\ADDITIONAL_LICENSE_INFO') -Raw
if ($inspectLicense -notmatch 'GNU General Public License' -or $inspectAdditional -notmatch 'Classpath') { throw 'OpenJDK mit GPL/Classpath Exception erforderlich, damit Laufzeit und Distributionshinweise übereinstimmen.' }
if (!(Test-Path (Join-Path $inspectSource 'engine\target\inspect-engine-0.2.0.jar'))) { throw 'Zuerst den Java-Kern mit mvn -f engine/pom.xml package bauen und prüfen.' }
New-Item -ItemType Directory -Force $InspectWorkDirectory | Out-Null
$inspectWorkRoot = (Resolve-Path $InspectWorkDirectory).Path
Write-Output "Windows build workspace: $inspectWorkRoot"
foreach ($inspectFolder in @('frontend','desktop','scripts','docs','assets','licenses','tests','cli')) {
  $inspectTarget = Join-Path $inspectWorkRoot $inspectFolder
  # Build trees, local session files and IDE data must never enter the staging directory.
  & robocopy (Join-Path $inspectSource $inspectFolder) $inspectTarget /E /XD target node_modules resources gen .local android ios /NFL /NDL /NJH /NJS /NP | Out-Null
  if ($LASTEXITCODE -ge 8) { throw "Kopieren von $inspectFolder fehlgeschlagen." }
}
# The general copy excludes staged resources. Preserve the guard's source module
# named processes/resources as part of its exact public source snapshot.
& robocopy (Join-Path $inspectSource 'desktop/installer/guard-source') (Join-Path $inspectWorkRoot 'desktop/installer/guard-source') /E /XD target /NFL /NDL /NJH /NJS /NP | Out-Null
if ($LASTEXITCODE -ge 8) { throw 'Kopieren der Guardquellen fehlgeschlagen.' }
foreach ($inspectFile in @('package.json','package-lock.json','tsconfig.json','LICENSE','THIRD_PARTY_NOTICES.md')) { Copy-Item (Join-Path $inspectSource $inspectFile) $inspectWorkRoot }
New-Item -ItemType Directory -Force (Join-Path $inspectWorkRoot 'engine\target') | Out-Null
Copy-Item (Join-Path $inspectSource 'engine\target\inspect-engine-0.2.0.jar') (Join-Path $inspectWorkRoot 'engine\target')
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
  try {
    $inspectArguments = @('..\node_modules\@tauri-apps\cli\tauri.js','build','--ci','--no-binary-patching')
    if ($InspectSigningThumbprint) {
      $inspectSnapshot = Join-Path $inspectWorkRoot 'signed-application.exe'
      $inspectConfig = Join-Path $inspectWorkRoot 'signing-config.json'
      $inspectSigning = Get-WindowsSigningConfig -Signer (Join-Path $inspectWorkRoot 'scripts\sign-windows-file.ps1') -Thumbprint $InspectSigningThumbprint -TimestampUrl $InspectTimestampUrl -Snapshot $inspectSnapshot
      & node.exe ../scripts/installer-guard.mjs
      if ($LASTEXITCODE) { throw 'signing_guard_build_failed' }
      $inspectGuard = Join-Path $inspectWorkRoot 'desktop/installer/guard/fbforge_suite_nsis_guard.dll'
      $inspectTool = Get-WindowsSignTool
      Invoke-WindowsFileSigning -Path $inspectGuard -Thumbprint $InspectSigningThumbprint -TimestampUrl $InspectTimestampUrl -SignTool $inspectTool -MainBinaryPath (Join-Path $inspectWorkRoot 'desktop/target/release/fbforge-inspect.exe') -SignedMainBinaryPath $inspectSnapshot
      $inspectGuardSignature = Assert-WindowsSignature -Path $inspectGuard -Thumbprint $InspectSigningThumbprint -SignTool $inspectTool
      # Preserve the signed guard through Tauri's later beforeBundle hook.
      $inspectSigning.build = @{ beforeBundleCommand = ('node scripts/installer-guard.mjs verify ' + $inspectGuardSignature.sha256 + ' && node desktop/installer/nsis/toolchain.mjs prepare') }
      [IO.File]::WriteAllText($inspectConfig, ($inspectSigning | ConvertTo-Json -Depth 8), (New-Object Text.UTF8Encoding($false)))
      $inspectArguments += @('--config',$inspectConfig)
    }
    $inspectArguments += @('--','--locked')
    & node.exe @inspectArguments
    if ($LASTEXITCODE) { throw 'Tauri-Windows-Build fehlgeschlagen.' }
  }
  finally { Pop-Location }
  $inspectArtifacts = if ($InspectArtifactDirectory) { [IO.Path]::GetFullPath($InspectArtifactDirectory) } else { Join-Path $inspectSource '.local\artifacts\windows-x64' }
  if ($InspectSigningThumbprint -and (Test-Path -LiteralPath $inspectArtifacts)) { throw 'signing_output_directory_must_be_new' }
  New-Item -ItemType Directory -Force $inspectArtifacts | Out-Null
  if ($InspectSigningThumbprint) {
    if (!(Test-Path -LiteralPath $inspectSnapshot -PathType Leaf)) { throw 'signing_application_snapshot_missing' }
    $inspectTool = Get-WindowsSignTool
    $null = Assert-WindowsSignature -Path $inspectSnapshot -Thumbprint $InspectSigningThumbprint -SignTool $inspectTool
    Copy-Item -LiteralPath $inspectSnapshot -Destination (Join-Path $inspectArtifacts 'fbforge-inspect.exe')
    $inspectSetups = @(Get-ChildItem -LiteralPath '.\desktop\target\release\bundle\nsis' -Filter '*-setup.exe' -File)
    if ($inspectSetups.Count -ne 1) { throw 'signing_installer_ambiguous' }
    $null = Assert-WindowsSignature -Path $inspectSetups[0].FullName -Thumbprint $InspectSigningThumbprint -SignTool $inspectTool
  } else { Copy-Item '.\desktop\target\release\fbforge-inspect.exe' $inspectArtifacts }
  $inspectEngineArtifacts = Join-Path $inspectArtifacts 'engine'
  & robocopy '.\desktop\target\release\engine' $inspectEngineArtifacts /MIR /NFL /NDL /NJH /NJS /NP | Out-Null
  if ($LASTEXITCODE -ge 8) { throw 'Kopieren der gebündelten Laufzeit fehlgeschlagen.' }
  & node.exe scripts/runtime-metadata.mjs --verify $inspectEngineArtifacts
  if ($LASTEXITCODE) { throw 'Laufzeit-Hinweise passen nicht zum Windows-Paket.' }
  Copy-Item '.\desktop\target\release\bundle\nsis\*.exe' $inspectArtifacts
  Copy-Item LICENSE $inspectArtifacts
  $inspectCliArtifacts = Join-Path $inspectArtifacts 'cli'
  New-Item -ItemType Directory -Force $inspectCliArtifacts | Out-Null
  foreach ($inspectCliFile in @('inspect.ps1','inspect.sh','README.txt')) { Copy-Item -LiteralPath (Join-Path $inspectWorkRoot ('cli\' + $inspectCliFile)) -Destination $inspectCliArtifacts }
  Copy-Item (Join-Path $inspectEngineArtifacts 'THIRD_PARTY_NOTICES.md') $inspectArtifacts
  Write-WindowsPackageHashes -Directory $inspectArtifacts
  Write-Output "Windows build (signed=$([bool]$InspectSigningThumbprint)): $inspectArtifacts"
  Write-Output "Build workspace retained for review: $inspectWorkRoot"
} finally { Pop-Location }
# Robocopy codes 0 through 7 are successful. Do not let a retained code 1
# turn a completed build into a GitHub PowerShell wrapper failure.
$global:LASTEXITCODE = 0

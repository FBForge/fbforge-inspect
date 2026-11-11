[CmdletBinding()]
param(
    [Parameter(Mandatory)][ValidatePattern('^[a-fA-F0-9]{40}$')][string]$Thumbprint,
    [string]$TimestampUrl = 'http://timestamp.digicert.com',
    [string]$InspectJavaHome = $env:JAVA_HOME,
    [string]$InspectToolsDirectory = (Join-Path $env:LOCALAPPDATA 'FBForge\BuildTools'),
    [string]$InspectWorkDirectory = (Join-Path ([IO.Path]::GetTempPath()) ('inspect-signed-build-' + [Guid]::NewGuid().ToString('N'))),
    [string]$InspectOutputDirectory = (Join-Path ([IO.Path]::GetTempPath()) ('inspect-signed-package-' + [Guid]::NewGuid().ToString('N'))),
    [switch]$CI
)
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
if ($env:OS -ne 'Windows_NT' -or -not [Environment]::UserInteractive) { throw 'signing_interactive_windows_required' }
if ($env:PROCESSOR_ARCHITECTURE -ne 'AMD64') { throw 'signing_windows_x64_required' }
if ($CI) {
    if ($env:GITHUB_ACTIONS -ne 'true' -or $env:GITHUB_EVENT_NAME -ne 'workflow_dispatch' -or $env:GITHUB_REPOSITORY -ne 'FBForge/fbforge-inspect' -or $env:GITHUB_REF -ne 'refs/heads/main') { throw 'signing_context_denied' }
    if ($env:GITHUB_SHA -notmatch '^[a-f0-9]{40}$' -or $env:GITHUB_RUN_ID -notmatch '^[1-9][0-9]*$' -or $env:VALIDATION_RUN -notmatch '^[1-9][0-9]{0,18}$') { throw 'signing_provenance_missing' }
}
if ((Test-Path -LiteralPath $InspectWorkDirectory) -or (Test-Path -LiteralPath $InspectOutputDirectory)) { throw 'signing_directories_must_be_new' }
Import-Module (Join-Path $PSScriptRoot 'windows-signing.psm1') -Force
Import-Module (Join-Path $PSScriptRoot 'windows-toolchain.psm1') -Force
$timestamp = $null
if (-not [Uri]::TryCreate($TimestampUrl, [UriKind]::Absolute, [ref]$timestamp) -or $timestamp.Scheme -notin @('http','https') -or -not $timestamp.Host -or $timestamp.UserInfo -or $timestamp.Fragment) { throw 'signing_timestamp_url_invalid' }
$tool = Get-WindowsSignTool
$certificate = Get-Item -LiteralPath ('Cert:\CurrentUser\My\' + $Thumbprint)
Assert-CodeSigningCertificate $certificate
if (-not $PSBoundParameters.ContainsKey('InspectJavaHome') -or -not $InspectJavaHome) { $InspectJavaHome = Find-InspectJdk25 }
if (-not $InspectJavaHome) { throw 'signing_windows_jdk25_missing' }
$root = Split-Path $PSScriptRoot -Parent
$updateKeyPath = $env:TAURI_SIGNING_PRIVATE_KEY_PATH
if (-not $updateKeyPath) { $updateKeyPath = Join-Path $env:LOCALAPPDATA 'FBForge\signing\connect-updater.key' }
$updatePublicKey = Join-Path $root 'desktop/update-public-key.txt'
if (-not (Test-Path -LiteralPath $updateKeyPath -PathType Leaf) -or -not $env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD -or -not ([IO.File]::ReadAllText($updatePublicKey).Trim())) { throw 'update_signing_setup_required' }
$previous = @{}
foreach ($name in @('JAVA_HOME','PATH','GITHUB_SHA','INSPECT_JAVA_SOURCE_URL','RUSTUP_TOOLCHAIN')) { $previous[$name] = [Environment]::GetEnvironmentVariable($name, 'Process') }
Push-Location $root
try {
    $head = (& git.exe rev-parse HEAD | Out-String).Trim()
    if ($LASTEXITCODE -ne 0 -or $head -notmatch '^[a-f0-9]{40}$' -or ($CI -and $head -ne $env:GITHUB_SHA)) { throw 'signing_checkout_mismatch' }
    $dirty = & git.exe status --porcelain --untracked-files=normal
    if ($LASTEXITCODE -ne 0 -or $dirty) { throw 'signing_checkout_modified' }
    $env:GITHUB_SHA = $head
    $env:JAVA_HOME = $InspectJavaHome
    $env:PATH = (Join-Path $InspectJavaHome 'bin') + ';' + $env:PATH
    $env:RUSTUP_TOOLCHAIN = '1.96.0'
    if (-not $env:INSPECT_JAVA_SOURCE_URL) { $env:INSPECT_JAVA_SOURCE_URL = Get-InspectJdkSource -Jdk $InspectJavaHome }
    $maven = Get-InspectMaven -ToolDirectory $InspectToolsDirectory
    & rustup.exe toolchain install 1.96.0 --profile minimal
    if ($LASTEXITCODE -ne 0) { throw 'signing_rust_toolchain_failed' }
    & $maven --batch-mode -f engine/pom.xml package
    if ($LASTEXITCODE -ne 0) { throw 'signing_engine_tests_failed' }
    & (Join-Path $PSScriptRoot 'build-windows.ps1') -InspectJavaHome $InspectJavaHome -InspectWorkDirectory $InspectWorkDirectory -InspectArtifactDirectory $InspectOutputDirectory -InspectSigningThumbprint $Thumbprint -InspectTimestampUrl $TimestampUrl
    if ($LASTEXITCODE -ne 0) { throw 'signing_build_failed' }
    $setups = @(Get-ChildItem -LiteralPath $InspectOutputDirectory -Filter '*-setup.exe' -File)
    if ($setups.Count -ne 1) { throw 'signing_installer_ambiguous' }
    $version = (Get-Content -LiteralPath desktop/tauri.conf.json -Raw | ConvertFrom-Json).version
    $updateInstallerName = 'FBForge-Inspect-' + $version + '-windows-x64-setup.exe'
    if ($setups[0].Name -ne $updateInstallerName) {
        Rename-Item -LiteralPath $setups[0].FullName -NewName $updateInstallerName
        $setups = @(Get-Item -LiteralPath (Join-Path $InspectOutputDirectory $updateInstallerName))
    }
    $signatures = @(
        (Assert-WindowsSignature -Path (Join-Path $InspectOutputDirectory 'fbforge-inspect.exe') -Thumbprint $Thumbprint -SignTool $tool),
        (Assert-WindowsSignature -Path $setups[0].FullName -Thumbprint $Thumbprint -SignTool $tool)
    )
    $dirty = & git.exe status --porcelain --untracked-files=normal
    if ($LASTEXITCODE -ne 0 -or $dirty) { throw 'signing_source_changed_during_build' }
    $version = (Get-Content -LiteralPath desktop/tauri.conf.json -Raw | ConvertFrom-Json).version
    $manifest = [ordered]@{
        application = 'FBForge Inspect'; version = $version; repository = 'FBForge/fbforge-inspect'
        commit = $head; platform = 'windows-x64'; signing = 'authenticode'
        certificateThumbprint = $Thumbprint.ToUpperInvariant(); installer = $setups[0].Name
        bytes = $setups[0].Length; sha256 = $signatures[1].sha256; signatures = $signatures
        runId = $(if ($CI) { $env:GITHUB_RUN_ID } else { $null })
        validationRunId = $(if ($CI) { $env:VALIDATION_RUN } else { $null })
        createdAt = [DateTime]::UtcNow.ToString('o'); published = $false
    }
    [IO.File]::WriteAllText((Join-Path $InspectOutputDirectory 'build-info.json'), ($manifest | ConvertTo-Json -Depth 8), (New-Object Text.UTF8Encoding($false)))
    # Bind the final Authenticode-signed bytes, canonical Inspect product filename and version.
    & node.exe (Join-Path $InspectWorkDirectory 'node_modules/@tauri-apps/cli/tauri.js') signer sign --private-key-path $updateKeyPath --app-version $version $setups[0].FullName | Out-Null
    if ($LASTEXITCODE -ne 0) { throw 'update_signature_failed' }
    & python.exe (Join-Path $root 'scripts/update-release.py') $InspectOutputDirectory --public-key $updatePublicKey
    if ($LASTEXITCODE -ne 0) { throw 'update_manifest_failed' }
    Write-WindowsPackageHashes -Directory $InspectOutputDirectory
    if ($CI) { Add-Content -LiteralPath $env:GITHUB_ENV -Encoding UTF8 -Value ('SIGNED_OUTPUT=' + [IO.Path]::GetFullPath($InspectOutputDirectory)) }
    Write-Host ('Signed application and installer verified: ' + $InspectOutputDirectory)
    Write-Host 'No release was published.'
} finally {
    Pop-Location
    foreach ($name in $previous.Keys) { [Environment]::SetEnvironmentVariable($name, $previous[$name], 'Process') }
}
$global:LASTEXITCODE = 0

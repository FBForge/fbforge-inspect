[CmdletBinding()]
param(
    [Parameter(Mandatory)][string]$Directory,
    [Parameter(Mandatory)][ValidatePattern('^[a-fA-F0-9]{40}$')][string]$Thumbprint
)
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$root = [IO.Path]::GetFullPath($Directory)
if ((Get-Item -LiteralPath $root -Force).Attributes -band [IO.FileAttributes]::ReparsePoint) { throw 'signed_package_link_denied' }
$seen = [Collections.Generic.HashSet[string]]::new([StringComparer]::OrdinalIgnoreCase)
foreach ($line in Get-Content -LiteralPath (Join-Path $root 'SHA256SUMS.txt') -Encoding UTF8) {
    if ($line -notmatch '^([a-f0-9]{64})  (.+)$') { throw 'signed_checksum_format_invalid' }
    $hash = $Matches[1]; $relative = $Matches[2]
    if ($relative -match '[:\\]' -or [IO.Path]::IsPathRooted($relative) -or -not $seen.Add($relative) -or $relative -eq 'SHA256SUMS.txt') { throw 'signed_checksum_path_invalid' }
    $file = $root
    foreach ($part in $relative.Split('/')) {
        if ($part -in @('','.', '..')) { throw 'signed_checksum_path_invalid' }
        $file = Join-Path $file $part
        if ((Get-Item -LiteralPath $file -Force).Attributes -band [IO.FileAttributes]::ReparsePoint) { throw 'signed_package_link_denied' }
    }
    if (-not (Test-Path -LiteralPath $file -PathType Leaf) -or (Get-FileHash -LiteralPath $file -Algorithm SHA256).Hash.ToLowerInvariant() -ne $hash) { throw 'signed_checksum_mismatch' }
}
$checksum = Join-Path $root 'SHA256SUMS.txt'
$files = @(Get-ChildItem -LiteralPath $root -File -Recurse -Force | Where-Object FullName -ne $checksum)
if ($seen.Count -ne $files.Count) { throw 'signed_checksum_inventory_mismatch' }
$manifest = Get-Content -LiteralPath (Join-Path $root 'build-info.json') -Encoding UTF8 -Raw | ConvertFrom-Json
$version = (Get-Content -LiteralPath (Join-Path $PSScriptRoot '../../desktop/tauri.conf.json') -Raw | ConvertFrom-Json).version
if ($manifest.application -ne 'FBForge Inspect' -or $manifest.platform -ne 'windows-x64' -or $manifest.commit -ne $env:GITHUB_SHA -or $manifest.runId -ne $env:GITHUB_RUN_ID -or $manifest.validationRunId -ne $env:VALIDATION_RUN -or $manifest.version -ne $version -or $manifest.repository -ne 'FBForge/fbforge-inspect' -or $manifest.signing -ne 'authenticode' -or $manifest.certificateThumbprint -ne $Thumbprint -or $manifest.published -ne $false) { throw 'signed_manifest_context_mismatch' }
if ([IO.Path]::GetFileName($manifest.installer) -ne $manifest.installer -or $manifest.installer -notlike '*-setup.exe' -or $manifest.installer -match '[:\\/]') { throw 'signed_installer_name_invalid' }
$setup = Join-Path $root $manifest.installer
$file = Get-Item -LiteralPath $setup
if ($manifest.sha256 -ne (Get-FileHash -LiteralPath $setup -Algorithm SHA256).Hash.ToLowerInvariant() -or $manifest.bytes -ne $file.Length) { throw 'signed_installer_integrity_failed' }
Import-Module (Join-Path $PSScriptRoot '../windows-signing.psm1') -Force
$tool = Get-WindowsSignTool
$application = Assert-WindowsSignature -Path (Join-Path $root 'fbforge-inspect.exe') -Thumbprint $Thumbprint -SignTool $tool
$installer = Assert-WindowsSignature -Path $setup -Thumbprint $Thumbprint -SignTool $tool
& node.exe (Join-Path $PSScriptRoot '../runtime-metadata.mjs') --verify (Join-Path $root 'engine')
if ($LASTEXITCODE -ne 0) { throw 'signed_runtime_provenance_invalid' }
& cargo.exe run --locked --manifest-path (Join-Path $PSScriptRoot '../../desktop/native-contracts/Cargo.toml') --example verify-update -- $root (Join-Path $PSScriptRoot '../../desktop/update-public-key.txt')
if ($LASTEXITCODE -ne 0) { throw 'signed_update_signature_invalid' }
$updateKey = [IO.File]::ReadAllText((Join-Path $PSScriptRoot '../../desktop/update-public-key.txt')).Trim()
$updateHash = [Security.Cryptography.SHA256]::Create()
try { $updateKeyHash = ([BitConverter]::ToString($updateHash.ComputeHash([Text.Encoding]::UTF8.GetBytes($updateKey)))).Replace('-', '').ToLowerInvariant() } finally { $updateHash.Dispose() }
if ($manifest.updatePublicKeySha256 -ne $updateKeyHash) { throw 'signed_update_key_provenance_invalid' }
[IO.File]::WriteAllText((Join-Path $root 'signature-info.json'), (@{ application = $application; installer = $installer; update = @{ verified = $true; publicKeySha256 = $updateKeyHash; version = $version; product = 'FBForge Inspect' }; commit = $env:GITHUB_SHA; runId = $env:GITHUB_RUN_ID; validationRunId = $env:VALIDATION_RUN } | ConvertTo-Json -Depth 6), (New-Object Text.UTF8Encoding($false)))
Write-WindowsPackageHashes -Directory $root
Write-Output 'Signed package identity, file inventory, signatures, timestamps and runtime provenance verified.'
$global:LASTEXITCODE = 0

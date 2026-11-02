[CmdletBinding()]
param([string]$TestDirectory = ([IO.Path]::GetTempPath()))
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$root = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../..'))
foreach ($name in @('scripts/windows-signing.psm1','scripts/windows-toolchain.psm1','scripts/sign-windows-file.ps1','scripts/build-signed-windows.ps1','scripts/build-windows.ps1','scripts/ci/check-signed-package.ps1')) {
    $tokens = $null; $errors = $null
    $null = [Management.Automation.Language.Parser]::ParseFile((Join-Path $root $name), [ref]$tokens, [ref]$errors)
    if ($errors.Count) { throw ('signing_script_parse_failed:' + $name + ':' + $errors[0].Message) }
}
Import-Module (Join-Path $root 'scripts/windows-signing.psm1') -Force
function Expect-Rejection([scriptblock]$Action, [string]$Code) {
    try { & $Action } catch { if ($_.Exception.Message -ne $Code) { throw }; return }
    throw ('expected_rejection_missing:' + $Code)
}
$now = [DateTime]::UtcNow
function New-TestCertificate {
    [pscustomobject]@{ HasPrivateKey = $true; NotBefore = $now.AddDays(-1); NotAfter = $now.AddDays(1)
        Extensions = @([pscustomobject]@{ Oid = [pscustomobject]@{ Value = '2.5.29.37' }; EnhancedKeyUsages = @([pscustomobject]@{ Value = '1.3.6.1.5.5.7.3.3' }) }) }
}
Assert-CodeSigningCertificate (New-TestCertificate) -Now $now
$certificate = New-TestCertificate; $certificate.HasPrivateKey = $false
Expect-Rejection { Assert-CodeSigningCertificate $certificate -Now $now } 'signing_private_key_unavailable'
$certificate = New-TestCertificate; $certificate.NotAfter = $now
Expect-Rejection { Assert-CodeSigningCertificate $certificate -Now $now } 'signing_certificate_not_current'
$certificate = New-TestCertificate; $certificate.NotBefore = $now.AddMinutes(1)
Expect-Rejection { Assert-CodeSigningCertificate $certificate -Now $now } 'signing_certificate_not_current'
$certificate = New-TestCertificate; $certificate.Extensions = @()
Expect-Rejection { Assert-CodeSigningCertificate $certificate -Now $now } 'signing_certificate_wrong_usage'
$certificate = New-TestCertificate; $certificate.Extensions[0].EnhancedKeyUsages[0].Value = '1.3.6.1.5.5.7.3.1'
Expect-Rejection { Assert-CodeSigningCertificate $certificate -Now $now } 'signing_certificate_wrong_usage'

# Only the platform providers are replaced. No certificate, private key, network
# endpoint or real executable is used; this tests decision and packaging contracts.
$module = Get-Module windows-signing
$temporary = Join-Path $TestDirectory ('signing-contracts-' + [Guid]::NewGuid().ToString('N'))
$null = New-Item -ItemType Directory -Path $temporary
$file = Join-Path $temporary 'fictional app.exe'
$tool = Join-Path $temporary 'fixture-tool.ps1'
$flag = Join-Path $temporary 'fail'
[IO.File]::WriteAllText($file, 'fictional signed bytes')
[IO.File]::WriteAllText($tool, @'
param([Parameter(ValueFromRemainingArguments)][string[]]$Arguments)
if (Test-Path -LiteralPath (Join-Path $PSScriptRoot 'fail')) { $global:LASTEXITCODE = 1 } else { $global:LASTEXITCODE = 0 }
'@)
$thumbprint = 'a' * 40
try {
    & $module {
        $script:testSignature = $null
        function script:Get-AuthenticodeSignature { param([string]$LiteralPath) return $script:testSignature }
    }
    $signature = [pscustomobject]@{ Status = 'Valid'; SignerCertificate = [pscustomobject]@{ Thumbprint = $thumbprint }; TimeStamperCertificate = [pscustomobject]@{ Thumbprint = ('b' * 40) } }
    & $module { param($Value) $script:testSignature = $Value } $signature
    $verified = Assert-WindowsSignature -Path $file -Thumbprint $thumbprint -SignTool $tool
    if (-not $verified.verified -or $verified.sha256 -ne (Get-FileHash -LiteralPath $file -Algorithm SHA256).Hash.ToLowerInvariant()) { throw 'signing_hash_contract_failed' }
    $snapshot = Join-Path $temporary 'packaged app.exe'
    Invoke-WindowsFileSigning -Path $file -Thumbprint $thumbprint -TimestampUrl 'https://timestamp.example.invalid' -SignTool $tool -MainBinaryPath $file -SignedMainBinaryPath $snapshot
    [IO.File]::WriteAllText($file, 'restored unsigned original')
    if ([IO.File]::ReadAllText($snapshot) -ne 'fictional signed bytes') { throw 'signing_snapshot_changed' }
    $plugin = Join-Path $temporary 'uninstaller.exe'
    [IO.File]::WriteAllText($plugin, 'fictional uninstaller')
    Invoke-WindowsFileSigning -Path $plugin -Thumbprint $thumbprint -TimestampUrl 'https://timestamp.example.invalid' -SignTool $tool -MainBinaryPath $file -SignedMainBinaryPath $snapshot
    if ([IO.File]::ReadAllText($snapshot) -ne 'fictional signed bytes') { throw 'signing_other_file_replaced_application' }
    $signature.Status = 'NotSigned'
    Expect-Rejection { Assert-WindowsSignature -Path $file -Thumbprint $thumbprint -SignTool $tool } 'signing_signature_invalid'
    $signature.Status = 'Valid'; $signature.SignerCertificate.Thumbprint = 'c' * 40
    Expect-Rejection { Assert-WindowsSignature -Path $file -Thumbprint $thumbprint -SignTool $tool } 'signing_certificate_mismatch'
    $signature.SignerCertificate.Thumbprint = $thumbprint; $signature.TimeStamperCertificate = $null
    Expect-Rejection { Assert-WindowsSignature -Path $file -Thumbprint $thumbprint -SignTool $tool } 'signing_timestamp_missing'
    $signature.TimeStamperCertificate = [pscustomobject]@{ Thumbprint = ('b' * 40) }
    [IO.File]::WriteAllText($flag, 'fail')
    Expect-Rejection { Assert-WindowsSignature -Path $file -Thumbprint $thumbprint -SignTool $tool } 'signing_verification_failed'
    Remove-Item -LiteralPath $snapshot
    Expect-Rejection { Invoke-WindowsFileSigning -Path $file -Thumbprint $thumbprint -TimestampUrl 'https://timestamp.example.invalid' -SignTool $tool -MainBinaryPath $file -SignedMainBinaryPath $snapshot } 'signing_file_failed'
    if (Test-Path -LiteralPath $snapshot) { throw 'signing_failed_file_captured' }
    Remove-Item -LiteralPath $flag
    Expect-Rejection { Invoke-WindowsFileSigning -Path $file -Thumbprint $thumbprint -TimestampUrl 'https://user:password@example.invalid' -SignTool $tool -MainBinaryPath $file -SignedMainBinaryPath $snapshot } 'signing_timestamp_url_invalid'
    $config = Get-WindowsSigningConfig -Signer 'C:\folder with spaces\sign.ps1' -Thumbprint $thumbprint -TimestampUrl 'https://timestamp.example.invalid' -Snapshot 'C:\folder with spaces\signed.exe'
    $decoded = $config | ConvertTo-Json -Depth 8 | ConvertFrom-Json
    if ([IO.Path]::GetDirectoryName($decoded.bundle.windows.signCommand.cmd) -ne $PSHOME) { throw 'signing_callback_host_mismatch' }
    if ($decoded.bundle.windows.signCommand.args[4] -ne 'C:\folder with spaces\sign.ps1' -or $decoded.bundle.windows.signCommand.args[-1] -ne '%1') { throw 'signing_config_arguments_changed' }
    Write-WindowsPackageHashes -Directory $temporary
    if (@(Get-Content -LiteralPath (Join-Path $temporary 'SHA256SUMS.txt')).Count -ne 3) { throw 'signing_hash_inventory_failed' }
} finally {
    Remove-Module windows-signing
    Remove-Item -LiteralPath $temporary -Recurse -Force
}
Write-Output 'Signing contract tests passed (synthetic providers; no real signing).'
$global:LASTEXITCODE = 0

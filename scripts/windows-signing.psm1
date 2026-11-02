Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

function Get-WindowsSignTool {
    $root = Join-Path ${env:ProgramFiles(x86)} 'Windows Kits\10\bin'
    $versions = @(Get-ChildItem -LiteralPath $root -Directory | Where-Object { $_.Name -match '^\d+\.\d+\.\d+\.\d+$' } | Sort-Object { [version]$_.Name } -Descending)
    foreach ($version in $versions) {
        $path = Join-Path $version.FullName 'x64\signtool.exe'
        if (Test-Path -LiteralPath $path -PathType Leaf) { return $path }
    }
    throw 'signing_windows_sdk_missing'
}

function Assert-CodeSigningCertificate {
    param([Parameter(Mandatory)]$Certificate, [DateTime]$Now = [DateTime]::UtcNow)
    if (-not $Certificate.HasPrivateKey) { throw 'signing_private_key_unavailable' }
    if ($Certificate.NotBefore.ToUniversalTime() -gt $Now -or $Certificate.NotAfter.ToUniversalTime() -le $Now) { throw 'signing_certificate_not_current' }
    $usage = @($Certificate.Extensions | Where-Object { $_.Oid.Value -eq '2.5.29.37' })
    if ($usage.Count -ne 1 -or @($usage[0].EnhancedKeyUsages | Where-Object { $_.Value -eq '1.3.6.1.5.5.7.3.3' }).Count -ne 1) { throw 'signing_certificate_wrong_usage' }
}

function Assert-WindowsSignature {
    param(
        [Parameter(Mandatory)][string]$Path,
        [Parameter(Mandatory)][ValidatePattern('^[a-fA-F0-9]{40}$')][string]$Thumbprint,
        [Parameter(Mandatory)][string]$SignTool
    )
    $file = [IO.Path]::GetFullPath($Path)
    $signature = Get-AuthenticodeSignature -LiteralPath $file
    Write-Host ('Authenticode: ' + [IO.Path]::GetFileName($file) + ' [' + $signature.Status + ']')
    if ($signature.Status -ne 'Valid') { throw 'signing_signature_invalid' }
    if (-not $signature.SignerCertificate -or $signature.SignerCertificate.Thumbprint -ne $Thumbprint) { throw 'signing_certificate_mismatch' }
    if (-not $signature.TimeStamperCertificate) { throw 'signing_timestamp_missing' }
    & $SignTool verify /pa /all /tw /v $file | Out-Host
    if ($LASTEXITCODE -ne 0) { throw 'signing_verification_failed' }
    return [pscustomobject]@{
        file = [IO.Path]::GetFileName($file)
        sha256 = (Get-FileHash -LiteralPath $file -Algorithm SHA256).Hash.ToLowerInvariant()
        certificateThumbprint = $signature.SignerCertificate.Thumbprint
        timestampCertificateThumbprint = $signature.TimeStamperCertificate.Thumbprint
        verified = $true
    }
}

function Invoke-WindowsFileSigning {
    param(
        [Parameter(Mandatory)][string]$Path,
        [Parameter(Mandatory)][ValidatePattern('^[a-fA-F0-9]{40}$')][string]$Thumbprint,
        [Parameter(Mandatory)][string]$TimestampUrl,
        [Parameter(Mandatory)][string]$SignTool,
        [Parameter(Mandatory)][string]$MainBinaryPath,
        [Parameter(Mandatory)][string]$SignedMainBinaryPath
    )
    $file = [IO.Path]::GetFullPath($Path)
    $timestamp = $null
    if (-not [Uri]::TryCreate($TimestampUrl, [UriKind]::Absolute, [ref]$timestamp) -or $timestamp.Scheme -notin @('http','https') -or -not $timestamp.Host -or $timestamp.UserInfo -or $timestamp.Fragment) { throw 'signing_timestamp_url_invalid' }
    & $SignTool sign /fd SHA256 /sha1 $Thumbprint /d 'FBForge Inspect' /tr $TimestampUrl /td SHA256 $file | Out-Host
    if ($LASTEXITCODE -ne 0) { throw 'signing_file_failed' }
    $null = Assert-WindowsSignature -Path $file -Thumbprint $Thumbprint -SignTool $SignTool
    if ($file -eq [IO.Path]::GetFullPath($MainBinaryPath)) {
        # Tauri restores the unsigned original after bundling. Save the exact signed
        # bytes now, while NSIS will still read this same executable into the setup.
        Copy-Item -LiteralPath $file -Destination $SignedMainBinaryPath
    }
}

function Select-WindowsSigningCertificate {
    param([string]$RequestedThumbprint)
    if ($RequestedThumbprint) {
        $RequestedThumbprint = $RequestedThumbprint -replace '\s', ''
        if ($RequestedThumbprint -notmatch '^[a-fA-F0-9]{40}$') { throw 'Ungueltiger Zertifikat-Thumbprint.' }
        $certificate = Get-Item -LiteralPath ('Cert:\CurrentUser\My\' + $RequestedThumbprint)
        Assert-CodeSigningCertificate $certificate
        return $certificate
    }
    $certificates = @(Get-ChildItem -Path Cert:\CurrentUser\My -CodeSigningCert |
        Where-Object { $_.HasPrivateKey -and $_.NotBefore.ToUniversalTime() -le [DateTime]::UtcNow -and $_.NotAfter.ToUniversalTime() -gt [DateTime]::UtcNow } |
        Sort-Object Subject, Thumbprint)
    if (!$certificates.Count) {
        throw 'Kein gueltiges Code-Signing-Zertifikat mit privatem Schluessel in CurrentUser\My gefunden. Token anschliessen und dieselbe Middleware wie fuer Connect verwenden.'
    }
    $index = 0
    if ($certificates.Count -gt 1) {
        Write-Host 'Mehrere Signierzertifikate gefunden. Das fuer FBForge Connect verwendete waehlen:'
        for ($i = 0; $i -lt $certificates.Count; $i++) {
            Write-Host ('[{0}] {1} | bis {2:yyyy-MM-dd} | {3}' -f ($i + 1), $certificates[$i].Subject, $certificates[$i].NotAfter, $certificates[$i].Thumbprint)
        }
        $selection = 0
        if (![int]::TryParse((Read-Host 'Nummer'), [ref]$selection) -or $selection -lt 1 -or $selection -gt $certificates.Count) {
            throw 'Keine eindeutige Zertifikatsauswahl.'
        }
        $index = $selection - 1
    }
    Assert-CodeSigningCertificate $certificates[$index]
    return $certificates[$index]
}


function Get-WindowsSigningConfig {
    param([string]$Signer, [string]$Thumbprint, [string]$TimestampUrl, [string]$Snapshot)
    # Keep the callback in the builder's PowerShell edition. A PS7 -> native tool ->
    # PS5 chain otherwise inherits PS7's module path and can lose Get-FileHash.
    $shell = Join-Path $PSHOME $(if ($PSVersionTable.PSVersion.Major -ge 6) { 'pwsh.exe' } else { 'powershell.exe' })
    return @{ bundle = @{ windows = @{
        certificateThumbprint = $Thumbprint; digestAlgorithm = 'sha256'; timestampUrl = $TimestampUrl; tsp = $true
        signCommand = @{ cmd = $shell; args = @('-NoProfile','-ExecutionPolicy','Bypass','-File',$Signer,'-Thumbprint',$Thumbprint,'-TimestampUrl',$TimestampUrl,'-SignedMainBinaryPath',$Snapshot,'-Path','%1') }
    } } }
}

function Write-WindowsPackageHashes {
    param([Parameter(Mandatory)][string]$Directory)
    $root = [IO.Path]::GetFullPath($Directory).TrimEnd([IO.Path]::DirectorySeparatorChar)
    $checksum = Join-Path $root 'SHA256SUMS.txt'
    $hashes = @(Get-ChildItem -LiteralPath $root -File -Recurse -Force | Where-Object FullName -ne $checksum | Sort-Object FullName | Get-FileHash -Algorithm SHA256 | ForEach-Object { $_.Hash.ToLowerInvariant() + '  ' + $_.Path.Substring($root.Length + 1).Replace('\', '/') })
    [IO.File]::WriteAllLines((Join-Path $root 'SHA256SUMS.txt'), [string[]]$hashes, (New-Object Text.UTF8Encoding($false)))
}

Export-ModuleMember -Function Get-WindowsSignTool, Assert-CodeSigningCertificate, Assert-WindowsSignature, Invoke-WindowsFileSigning, Get-WindowsSigningConfig, Write-WindowsPackageHashes, Select-WindowsSigningCertificate

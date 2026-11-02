[CmdletBinding()]
param(
    [Parameter(Mandatory)][string]$Path,
    [Parameter(Mandatory)][ValidatePattern('^[a-fA-F0-9]{40}$')][string]$Thumbprint,
    [Parameter(Mandatory)][string]$TimestampUrl,
    [Parameter(Mandatory)][string]$SignedMainBinaryPath
)
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
Import-Module (Join-Path $PSScriptRoot 'windows-signing.psm1') -Force
$main = Join-Path $PSScriptRoot '../desktop/target/release/fbforge-inspect.exe'
Invoke-WindowsFileSigning -Path $Path -Thumbprint $Thumbprint -TimestampUrl $TimestampUrl -SignTool (Get-WindowsSignTool) -MainBinaryPath $main -SignedMainBinaryPath $SignedMainBinaryPath
exit 0

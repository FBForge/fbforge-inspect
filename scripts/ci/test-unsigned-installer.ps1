[CmdletBinding()]
param(
    [Parameter(Mandatory)][string]$InstallerPath,
    [Parameter(Mandatory)][string]$ToolId
)
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$catalog = Get-Content -LiteralPath (Join-Path $PSScriptRoot '../../desktop/installer/guard-source/catalog/tools.v1.json') -Raw | ConvertFrom-Json
$tool = @($catalog.tools | Where-Object { $_.id -ceq $ToolId })
if ($catalog.schemaVersion -ne 1 -or $tool.Count -ne 1) { throw 'unsigned_fixture_tool_unknown' }
$tool = $tool[0]
$localAppData = [Environment]::GetFolderPath([Environment+SpecialFolder]::LocalApplicationData, [Environment+SpecialFolderOption]::DoNotVerify)
if (-not $localAppData) { throw 'unsigned_fixture_local_app_data_unavailable' }
$defaultTarget = Join-Path $localAppData $tool.productName
function Assert-NativeFixtureAbsent([string]$Code) {
    if (Test-Path -LiteralPath $defaultTarget) { throw $Code }
    foreach ($view in @([Microsoft.Win32.RegistryView]::Registry64, [Microsoft.Win32.RegistryView]::Registry32)) {
        $registry = [Microsoft.Win32.RegistryKey]::OpenBaseKey([Microsoft.Win32.RegistryHive]::CurrentUser, $view)
        try {
            foreach ($name in $tool.registryNames) {
                $entry = $registry.OpenSubKey('Software\Microsoft\Windows\CurrentVersion\Uninstall\' + $name, $false)
                if ($null -ne $entry) { $entry.Dispose(); throw $Code }
            }
        } finally { $registry.Dispose() }
    }
}
# V2 chooses its native destination, independently of NSIS's requested /D path.
# Refuse an existing installation without modifying or deleting it.
Assert-NativeFixtureAbsent 'unsigned_fixture_not_clean'
$installer = (Resolve-Path -LiteralPath $InstallerPath).Path
if ((Get-AuthenticodeSignature -LiteralPath $installer).Status -ne 'NotSigned') { throw 'unsigned_fixture_required' }
$root = Join-Path ([IO.Path]::GetTempPath()) ('fbforge-unsigned-rejection-' + [Guid]::NewGuid().ToString('N'))
$target = Join-Path $root 'application files'
$null = New-Item -ItemType Directory -Path $root
$sentinel = Join-Path $root 'sentinel.txt'
[IO.File]::WriteAllText($sentinel, 'unchanged')
try {
    # NSIS /D is deliberately last and unquoted, including its possible spaces.
    $process = Start-Process -FilePath $installer -ArgumentList ('/S /D=' + $target) -PassThru
    try {
        if (-not $process.WaitForExit(120000)) { throw 'unsigned_rejection_timeout' }
        if ($process.ExitCode -ne 70) { throw 'unsigned_installer_not_rejected' }
    } finally { $process.Dispose() }
    Assert-NativeFixtureAbsent 'unsigned_installer_mutated_native_target'
    if (Test-Path -LiteralPath $target) { throw 'unsigned_installer_mutated_target' }
    if ([IO.File]::ReadAllText($sentinel) -ne 'unchanged') { throw 'unsigned_installer_changed_sentinel' }
    Write-Output 'Unsigned installer rejected with exit 70; native default target, uninstall keys and requested target absent; sentinel unchanged.'
} finally {
    Remove-Item -LiteralPath $root -Recurse -Force
}

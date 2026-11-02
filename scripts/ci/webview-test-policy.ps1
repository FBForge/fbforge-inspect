[CmdletBinding()]
param([Parameter(Mandatory)][ValidateSet('enable','disable')][string]$Mode, [Parameter(Mandatory)][string]$Directory, [Parameter(Mandatory)][int]$Port)
# Adapted from FBForge Connect's EUPL-1.2 hosted native test helper.
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
# Elevated WebView2 hosts ignore environment/HKCU overrides. Use only these two
# application identifiers on the disposable hosted CI machine, never a wildcard.
# https://learn.microsoft.com/microsoft-edge/webview2/concepts/security#for-an-elevated-host-app-use-appropriate-override-flags
if ($env:GITHUB_REPOSITORY -ne 'FBForge/fbforge-inspect' -or $env:GITHUB_EVENT_NAME -notin @('push','pull_request','workflow_dispatch') -or $env:RUNNER_ENVIRONMENT -ne 'github-hosted') { throw 'ui_policy_context_denied' }
$root = [IO.Path]::GetFullPath($env:RUNNER_TEMP).TrimEnd('\') + '\'
$owned = [IO.Path]::GetFullPath($Directory)
if (-not $owned.StartsWith($root, [StringComparison]::OrdinalIgnoreCase) -or $Port -lt 1024 -or $Port -gt 65535) { throw 'ui_policy_arguments_denied' }
$receipt = Join-Path $owned 'webview-test-policy.json'
$identity = [Security.Principal.WindowsIdentity]::GetCurrent()
$admin = [Security.Principal.WindowsPrincipal]::new($identity).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
if (-not $admin) { Write-Output 'WebView test uses normal per-process environment flags'; exit 0 }
$base = 'SOFTWARE\Policies\Microsoft\Edge\WebView2'
$applications = @('fbforge-inspect.exe','de.fbforge.inspect')
$expected = @{
    AdditionalBrowserArguments = "--remote-debugging-port=$Port --remote-debugging-address=127.0.0.1"
    UserDataFolder = (Join-Path $owned 'webview-cache')
}
if ($Mode -eq 'enable') {
    if (Test-Path -LiteralPath $receipt) { throw 'ui_policy_receipt_conflict' }
    $created = @()
    foreach ($kind in $expected.Keys) {
        $key = [Microsoft.Win32.Registry]::LocalMachine.OpenSubKey("$base\$kind")
        try {
            if (-not $key) { $created += $kind; continue }
            foreach ($name in $applications) { if ($key.GetValueNames() -contains $name) { throw 'ui_policy_existing_application_override' } }
        } finally { if ($key) { $key.Dispose() } }
    }
    # The receipt is written first so partially completed setup is also reversible.
    [IO.File]::WriteAllText($receipt, (@{ port = $Port; directory = $owned; created = @($created) } | ConvertTo-Json), [Text.UTF8Encoding]::new($false))
    foreach ($kind in $expected.Keys) {
        $key = [Microsoft.Win32.Registry]::LocalMachine.CreateSubKey("$base\$kind")
        try { foreach ($name in $applications) { $key.SetValue($name, $expected[$kind], [Microsoft.Win32.RegistryValueKind]::String) } }
        finally { $key.Dispose() }
    }
    Write-Output 'Temporary executable-specific WebView test overrides enabled for elevated hosted CI'
} else {
    if (-not (Test-Path -LiteralPath $receipt)) { exit 0 }
    $record = Get-Content -LiteralPath $receipt -Raw | ConvertFrom-Json
    if ($record.port -ne $Port -or $record.directory -ne $owned) { throw 'ui_policy_receipt_mismatch' }
    foreach ($kind in $expected.Keys) {
        $key = [Microsoft.Win32.Registry]::LocalMachine.OpenSubKey("$base\$kind", $true)
        if (-not $key) { continue }
        try {
            foreach ($name in $applications) {
                $current = $key.GetValue($name)
                if ($null -eq $current) { continue }
                if ($current -ne $expected[$kind]) { throw 'ui_policy_changed_by_another_process' }
                $key.DeleteValue($name, $false)
            }
            $empty = $key.ValueCount -eq 0 -and $key.SubKeyCount -eq 0
        } finally { $key.Dispose() }
        if ($empty -and @($record.created) -contains $kind) { [Microsoft.Win32.Registry]::LocalMachine.DeleteSubKey("$base\$kind", $false) }
    }
    Remove-Item -LiteralPath $receipt
    Write-Output 'Temporary executable-specific WebView test overrides removed'
}

# Test helper for the Windows common Save dialog owned by the supplied Inspect process.
[CmdletBinding()]
param(
    [Parameter(Mandatory)][int]$ApplicationProcessId,
    [Parameter(Mandatory)][ValidateSet('cancel','save')][string]$Mode,
    [Parameter(Mandatory)][string]$TestDirectory,
    [string]$FileName
)
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$application = Get-Process -Id $ApplicationProcessId
if ($application.ProcessName -ne 'fbforge-inspect') { throw 'save_dialog_wrong_process' }
$directory = [IO.Path]::GetFullPath($TestDirectory)
$temporaryRoot = [IO.Path]::GetFullPath($env:RUNNER_TEMP).TrimEnd('\') + '\'
if (-not $directory.StartsWith($temporaryRoot, [StringComparison]::OrdinalIgnoreCase) -or -not (Test-Path -LiteralPath $directory -PathType Container)) { throw 'save_dialog_directory_not_owned' }
if ($Mode -eq 'save' -and $FileName -notmatch '^native-report-(default|raw)\.(json|html)$') { throw 'save_dialog_invalid_filename' }
$destination = if ($Mode -eq 'save') { Join-Path $directory $FileName } else { $null }
if ($destination -and (Test-Path -LiteralPath $destination)) { throw 'save_dialog_refuses_overwrite' }
Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName UIAutomationTypes
$processCondition = [System.Windows.Automation.PropertyCondition]::new([System.Windows.Automation.AutomationElement]::ProcessIdProperty, $ApplicationProcessId)
$dialogCondition = [System.Windows.Automation.PropertyCondition]::new([System.Windows.Automation.AutomationElement]::ClassNameProperty, '#32770')
$dialog = $null
$deadline = [DateTime]::UtcNow.AddSeconds(25)
do {
    $windows = [System.Windows.Automation.AutomationElement]::RootElement.FindAll([System.Windows.Automation.TreeScope]::Children, $processCondition)
    foreach ($window in $windows) {
        if ($window.Current.ClassName -eq '#32770') { $dialog = $window; break }
        $dialog = $window.FindFirst([System.Windows.Automation.TreeScope]::Descendants, $dialogCondition)
        if ($dialog) { break }
    }
    if ($dialog) { break }
    Start-Sleep -Milliseconds 200
} while ([DateTime]::UtcNow -lt $deadline)
if (-not $dialog) { throw 'save_dialog_not_found' }
if ($dialog.Current.ProcessId -ne $ApplicationProcessId) { throw 'save_dialog_wrong_owner' }
function Find-DialogControl([string]$Id, $Type) {
    $idCondition = [System.Windows.Automation.PropertyCondition]::new([System.Windows.Automation.AutomationElement]::AutomationIdProperty, $Id)
    $typeCondition = [System.Windows.Automation.PropertyCondition]::new([System.Windows.Automation.AutomationElement]::ControlTypeProperty, $Type)
    $condition = [System.Windows.Automation.AndCondition]::new($idCondition, $typeCondition)
    $controlDeadline = [DateTime]::UtcNow.AddSeconds(15)
    do {
        $control = $dialog.FindFirst([System.Windows.Automation.TreeScope]::Descendants, $condition)
        if ($control -and $control.Current.IsEnabled) { return $control }
        Start-Sleep -Milliseconds 200
    } while ([DateTime]::UtcNow -lt $controlDeadline)
    return $null
}
if ($Mode -eq 'save') {
    $filenameInput = Find-DialogControl '1001' ([System.Windows.Automation.ControlType]::Edit)
    if (-not $filenameInput) { throw 'save_dialog_filename_edit_missing' }
    $valuePattern = $null
    if (-not $filenameInput.TryGetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern, [ref]$valuePattern)) { throw 'save_dialog_filename_not_editable' }
    $valuePattern.SetValue($destination)
}
$buttonId = if ($Mode -eq 'save') { '1' } else { '2' }
$button = Find-DialogControl $buttonId ([System.Windows.Automation.ControlType]::Button)
if (-not $button) {
    $buttonCondition = [System.Windows.Automation.PropertyCondition]::new([System.Windows.Automation.AutomationElement]::ControlTypeProperty, [System.Windows.Automation.ControlType]::Button)
    $observed = @($dialog.FindAll([System.Windows.Automation.TreeScope]::Descendants, $buttonCondition) | ForEach-Object { $_.Current.AutomationId + ':' + $_.Current.ClassName })
    throw ('save_dialog_action_missing buttonIdsAndClasses=' + ($observed -join ','))
}
$invokePattern = $null
if (-not $button.TryGetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern, [ref]$invokePattern)) { throw 'save_dialog_action_unavailable' }
$invokePattern.Invoke()
Write-Output ('Native owned Save dialog action: ' + $Mode)

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
Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
using System.Text;
using System.Collections.Generic;
public static class InspectSaveWindow {
    [DllImport("user32.dll")] public static extern IntPtr GetAncestor(IntPtr window, uint flags);
    [DllImport("user32.dll")] private static extern uint GetWindowThreadProcessId(IntPtr window, out uint process);
    public static uint ProcessId(IntPtr window) { uint process; GetWindowThreadProcessId(window, out process); return process; }
    private delegate bool VisitWindow(IntPtr window, IntPtr parameter);
    [DllImport("user32.dll")] private static extern bool EnumChildWindows(IntPtr window, VisitWindow visit, IntPtr parameter);
    [DllImport("user32.dll")] private static extern int GetDlgCtrlID(IntPtr window);
    [DllImport("user32.dll", CharSet = CharSet.Unicode)] private static extern int GetClassName(IntPtr window, StringBuilder name, int size);
    [DllImport("user32.dll")] private static extern bool IsWindowEnabled(IntPtr window);
    [DllImport("user32.dll", CharSet = CharSet.Unicode)] private static extern IntPtr SendMessage(IntPtr window, uint message, IntPtr parameter, string text);
    private static string ClassName(IntPtr window) { var name = new StringBuilder(256); GetClassName(window, name, name.Capacity); return name.ToString(); }
    public static IntPtr FindChild(IntPtr root, int id, string expectedClass) {
        IntPtr found = IntPtr.Zero;
        EnumChildWindows(root, (window, parameter) => {
            if (GetDlgCtrlID(window) == id && ClassName(window).Equals(expectedClass, StringComparison.OrdinalIgnoreCase) && IsWindowEnabled(window)) { found = window; return false; }
            return true;
        }, IntPtr.Zero);
        return found;
    }
    [DllImport("user32.dll", EntryPoint = "SendMessageW", CharSet = CharSet.Unicode)] private static extern IntPtr ReadMessage(IntPtr window, uint message, IntPtr parameter, StringBuilder text);
    [DllImport("user32.dll", EntryPoint = "SendMessageW")] private static extern IntPtr NativeMessage(IntPtr window, uint message, UIntPtr parameter, IntPtr value);
    public static string ReadText(IntPtr window) {
        var text = new StringBuilder(1024);
        ReadMessage(window, 0x000D, (IntPtr)text.Capacity, text);
        return text.ToString();
    }
    public static IntPtr FindFilenameInput(IntPtr root, string expectedDefault) {
        var matches = new List<IntPtr>();
        EnumChildWindows(root, (window, parameter) => {
            if (ClassName(window) != "Edit" || !IsWindowEnabled(window)) return true;
            // Identify the field by the exact default filename supplied by Inspect,
            // rather than shell-dependent IDs reused by search and address controls.
            var value = ReadText(window);
            if (value == expectedDefault || value == "fbforge-inspect") matches.Add(window);
            return true;
        }, IntPtr.Zero);
        return matches.Count == 1 ? matches[0] : IntPtr.Zero;
    }
    public static bool TypeFilename(IntPtr window, string text) {
        NativeMessage(window, 0x00B1, UIntPtr.Zero, (IntPtr)(-1)); // Select the default filename.
        foreach (char character in text) NativeMessage(window, 0x0102, (UIntPtr)character, (IntPtr)1);
        return ReadText(window).Equals(text, StringComparison.OrdinalIgnoreCase);
    }
    public static string DescribeChildren(IntPtr root) {
        var controls = new List<string>();
        EnumChildWindows(root, (window, parameter) => {
            var name = ClassName(window);
            if (name == "Button" || name == "Edit" || name.StartsWith("ComboBox")) controls.Add(GetDlgCtrlID(window) + ":" + name);
            return controls.Count < 50;
        }, IntPtr.Zero);
        return string.Join(",", controls);
    }
    public static void Click(IntPtr window) { SendMessage(window, 0x00F5, IntPtr.Zero, null); }
}
'@
$processCondition = [System.Windows.Automation.PropertyCondition]::new([System.Windows.Automation.AutomationElement]::ProcessIdProperty, $ApplicationProcessId)
$dialogCondition = [System.Windows.Automation.PropertyCondition]::new([System.Windows.Automation.AutomationElement]::ClassNameProperty, '#32770')
$dialog = $null
$deadline = [DateTime]::UtcNow.AddSeconds(25)
do {
    $windows = [System.Windows.Automation.AutomationElement]::RootElement.FindAll([System.Windows.Automation.TreeScope]::Children, $processCondition)
    foreach ($window in $windows) {
        if ($window.Current.ClassName -eq '#32770') { $dialog = $window; break }
        $nestedDialog = $window.FindFirst([System.Windows.Automation.TreeScope]::Descendants, $dialogCondition)
        # The shell folder pane can itself be a nested #32770. Search the owning
        # top-level window so the filename field and action buttons remain in scope.
        if ($nestedDialog) { $dialog = $window; break }
    }
    if ($dialog) { break }
    Start-Sleep -Milliseconds 200
} while ([DateTime]::UtcNow -lt $deadline)
if (-not $dialog) { throw 'save_dialog_not_found' }
# Resolve the real HWND root, rather than a UIA fragment rooted at the shell pane.
$dialogHandle = [InspectSaveWindow]::GetAncestor([IntPtr]$dialog.Current.NativeWindowHandle, 2)
if ($dialogHandle -eq [IntPtr]::Zero -or [InspectSaveWindow]::ProcessId($dialogHandle) -ne $ApplicationProcessId) { throw 'save_dialog_wrong_native_owner' }
$dialog = [System.Windows.Automation.AutomationElement]::FromHandle($dialogHandle)
if ($dialog.Current.ProcessId -ne $ApplicationProcessId) { throw 'save_dialog_wrong_owner' }
function Wait-NativeControl([int]$Id, [string]$Class) {
    $controlDeadline = [DateTime]::UtcNow.AddSeconds(10)
    do {
        $control = [InspectSaveWindow]::FindChild($dialogHandle, $Id, $Class)
        if ($control -ne [IntPtr]::Zero) {
            if ([InspectSaveWindow]::ProcessId($control) -ne $ApplicationProcessId) { throw 'save_dialog_wrong_control_owner' }
            return $control
        }
        Start-Sleep -Milliseconds 200
    } while ([DateTime]::UtcNow -lt $controlDeadline)
    throw ('save_dialog_native_control_missing id=' + $Id + ' class=' + $Class + ' observedIdsAndClasses=' + [InspectSaveWindow]::DescribeChildren($dialogHandle))
}
if ($Mode -eq 'save') {
    $filenameDeadline = [DateTime]::UtcNow.AddSeconds(10)
    $filenameInput = [IntPtr]::Zero
    do {
        $filenameInput = [InspectSaveWindow]::FindFilenameInput($dialogHandle, ('fbforge-inspect' + [IO.Path]::GetExtension($FileName)))
        if ($filenameInput -ne [IntPtr]::Zero) { break }
        Start-Sleep -Milliseconds 200
    } while ([DateTime]::UtcNow -lt $filenameDeadline)
    if ($filenameInput -eq [IntPtr]::Zero) { throw ('save_dialog_filename_missing observedIdsAndClasses=' + [InspectSaveWindow]::DescribeChildren($dialogHandle)) }
    if ([InspectSaveWindow]::ProcessId($filenameInput) -ne $ApplicationProcessId) { throw 'save_dialog_wrong_filename_owner' }
    if (-not [InspectSaveWindow]::TypeFilename($filenameInput, $destination)) { throw 'save_dialog_filename_not_set' }
}
$buttonId = if ($Mode -eq 'save') { 1 } else { 2 }
$button = Wait-NativeControl $buttonId 'Button'
[InspectSaveWindow]::Click($button)
Write-Output ('Native owned Save dialog action: ' + $Mode)

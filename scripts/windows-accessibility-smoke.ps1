[CmdletBinding()]
param(
    [Parameter(Mandatory)][string]$InstallerPath,
    [string]$ExpectedBinaryPath,
    [string]$CandidateSourceCommit = $env:GITHUB_SHA,
    [ValidatePattern('^[a-f0-9]{64}$')][string]$ExpectedInstallerSha256,
    [Parameter(Mandatory)][string]$NvdaPath
)
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
if ([bool]$ExpectedBinaryPath -eq [bool]$ExpectedInstallerSha256) { throw 'a11y_expected_artifact_required' }
if ($ExpectedInstallerSha256 -and (Get-FileHash -LiteralPath $InstallerPath -Algorithm SHA256).Hash.ToLowerInvariant() -ne $ExpectedInstallerSha256) { throw 'a11y_installer_integrity_failed' }
Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName UIAutomationTypes
Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
public static class AccessibilityKeys {
    [DllImport("user32.dll")] private static extern bool SetForegroundWindow(IntPtr window);
    [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
    [DllImport("user32.dll")] private static extern uint GetWindowThreadProcessId(IntPtr window, out uint process);
    [DllImport("kernel32.dll")] private static extern uint GetCurrentThreadId();
    [DllImport("user32.dll")] private static extern bool AttachThreadInput(uint first, uint second, bool attach);
    [DllImport("user32.dll")] private static extern bool BringWindowToTop(IntPtr window);
    [DllImport("user32.dll")] private static extern bool ShowWindow(IntPtr window, int command);
    public static bool FocusWindow(IntPtr window) {
        if (GetForegroundWindow() == window) return true;
        uint process;
        uint current = GetCurrentThreadId();
        uint foreground = GetWindowThreadProcessId(GetForegroundWindow(), out process);
        uint target = GetWindowThreadProcessId(window, out process);
        bool attachedForeground = false, attachedTarget = false;
        try {
            if (foreground != 0 && foreground != current)
                attachedForeground = AttachThreadInput(current, foreground, true);
            if (target != current && target != foreground)
                attachedTarget = AttachThreadInput(current, target, true);
            ShowWindow(window, 9);
            BringWindowToTop(window);
            SetForegroundWindow(window);
        } finally {
            if (attachedTarget) AttachThreadInput(current, target, false);
            if (attachedForeground) AttachThreadInput(current, foreground, false);
        }
        return GetForegroundWindow() == window;
    }
    [DllImport("user32.dll")] private static extern bool SetCursorPos(int x, int y);
    [DllImport("user32.dll")] private static extern void mouse_event(uint flags, uint x, uint y, uint data, UIntPtr extra);
    public static void ActivateInput(int x, int y) {
        if (!SetCursorPos(x, y)) throw new InvalidOperationException("a11y_pointer_failed");
        mouse_event(2, 0, 0, 0, UIntPtr.Zero);
        System.Threading.Thread.Sleep(60);
        mouse_event(4, 0, 0, 0, UIntPtr.Zero);
        System.Threading.Thread.Sleep(300);
    }
    [DllImport("user32.dll")] private static extern void keybd_event(byte key, byte scan, uint flags, UIntPtr extra);
    [DllImport("user32.dll")] private static extern uint MapVirtualKey(uint key, uint mode);
    private static void Key(byte key, bool released) {
        uint flags = (key == 0x2d ? 1u : 0u) | (released ? 2u : 0u);
        keybd_event(key, (byte)MapVirtualKey(key, 0), flags, UIntPtr.Zero);
    }
    public static void Press(byte key) {
        Key(key, false);
        System.Threading.Thread.Sleep(60);
        Key(key, true);
        System.Threading.Thread.Sleep(250);
    }
    public static void ReportFocus() {
        Key(0x2d, false); // NVDA's standard Insert+Tab command.
        System.Threading.Thread.Sleep(40);
        Press(0x09);
        Key(0x2d, true);
        System.Threading.Thread.Sleep(250);
    }
}
'@
$taskRoot = Join-Path $env:RUNNER_TEMP ('fbforge-accessibility-' + [Guid]::NewGuid().ToString('N'))
$taskInstall = Join-Path $taskRoot 'application files'
$taskConfig = Join-Path $taskRoot 'nvda-config'
$taskCapture = Join-Path $taskRoot 'speech.jsonl'
$taskApp = $null
$taskNvda = $null
$taskWindow = $null
$taskInputActive = $false
$checks = [Collections.Generic.List[object]]::new()

function Wait-Check([scriptblock]$Check, [string]$Code, [int]$Seconds = 30) {
    $until = [DateTime]::UtcNow.AddSeconds($Seconds)
    do {
        $value = & $Check
        if ($value) { return $value }
        Start-Sleep -Milliseconds 250
    } while ([DateTime]::UtcNow -lt $until)
    throw $Code
}
function Find-Control([string]$Name, $Type) {
    $nameCondition = [System.Windows.Automation.PropertyCondition]::new([System.Windows.Automation.AutomationElement]::NameProperty, $Name)
    $typeCondition = [System.Windows.Automation.PropertyCondition]::new([System.Windows.Automation.AutomationElement]::ControlTypeProperty, $Type)
    $condition = [System.Windows.Automation.AndCondition]::new($nameCondition, $typeCondition)
    $taskWindow.FindFirst([System.Windows.Automation.TreeScope]::Descendants, $condition)
}
function Read-Capture {
    if (-not (Test-Path -LiteralPath $taskCapture)) { return @() }
    try { @(Get-Content -LiteralPath $taskCapture -Encoding UTF8 | ForEach-Object { $_ | ConvertFrom-Json }) }
    catch { @() } # A concurrent append is retried by the bounded wait.
}
function Check-Spoken([string]$Name, $Type) {
    $element = Wait-Check { Find-Control $Name $Type } ('a11y_control_missing:' + $Name)
    if (-not $element.Current.IsEnabled -or -not $element.Current.IsKeyboardFocusable) { throw ('a11y_not_focusable:' + $Name) }
    $before = @(Read-Capture).Count
    if (-not [AccessibilityKeys]::FocusWindow([IntPtr]$taskWindow.Current.NativeWindowHandle)) { throw 'a11y_window_not_foreground' }
    if ($taskInputActive) {
        # Reach controls using actual keyboard navigation rather than UIA's DOM-only focus.
        $reached = $false
        for ($step = 0; $step -lt 60; $step++) {
            $focused = [System.Windows.Automation.AutomationElement]::FocusedElement
            if ($focused -and $focused.Current.Name -eq $Name -and $focused.Current.ControlType -eq $Type) { $reached = $true; break }
            if ([AccessibilityKeys]::GetForegroundWindow() -ne [IntPtr]$taskWindow.Current.NativeWindowHandle) { throw 'a11y_keyboard_left_owned_window' }
            [AccessibilityKeys]::Press(0x09)
        }
        if (-not $reached) { throw ('a11y_native_focus_unreachable:' + $Name) }
    } else {
        $element.SetFocus()
    }
    Start-Sleep -Milliseconds 300
    [AccessibilityKeys]::ReportFocus()
    $spoken = Wait-Check {
        $records = @(Read-Capture)
        if (@($records | Where-Object { $_.PSObject.Properties.Name -contains 'captureLimitReached' }).Count) { throw 'nvda_capture_limit_reached' }
        if ($records.Count -le $before) { return $false }
        foreach ($record in $records[$before..($records.Count - 1)]) {
            if ($record.PSObject.Properties.Name -contains 'text' -and $record.PSObject.Properties.Name -contains 'focus' -and $record.focus -eq $Name -and $record.text.IndexOf($Name, [StringComparison]::OrdinalIgnoreCase) -ge 0) { return $record.text }
        }
        return $false
    } ('nvda_label_not_spoken:' + $Name) 15
    $checks.Add([ordered]@{ name = $Name; role = $Type.ProgrammaticName; speech = $spoken })
    Write-Host ('NVDA generated the focused control label: ' + $Name)
    return $element
}
function Invoke-Control($Element) {
    $pattern = $null
    if (-not $Element.TryGetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern, [ref]$pattern)) { throw 'a11y_invoke_missing' }
    $pattern.Invoke()
}
function Start-Clean([string]$Executable, [string]$Arguments = '') {
    $info = [Diagnostics.ProcessStartInfo]::new([IO.Path]::GetFullPath($Executable))
    $info.UseShellExecute = $false; $info.Arguments = $Arguments
    foreach ($key in @('JAVA_HOME','JDK_HOME','JAVA_TOOL_OPTIONS','_JAVA_OPTIONS','JDK_JAVA_OPTIONS','CLASSPATH','GH_TOKEN','GITHUB_TOKEN','ACTIONS_RUNTIME_TOKEN','ACTIONS_ID_TOKEN_REQUEST_TOKEN','SENDERID','SENDERSECRET','SENDERDEST','SUBSCRIBERID','SUBSCRIBERSECRET')) { $null = $info.EnvironmentVariables.Remove($key) }
    $info.EnvironmentVariables['PATH'] = "$env:SystemRoot\System32;$env:SystemRoot"
    return $info
}
try {
    if (@(Get-Process -Name 'nvda','nvda_noUIAccess' -ErrorAction SilentlyContinue).Count) { throw 'existing_screen_reader_conflict' }
    $null = New-Item -ItemType Directory -Path (Join-Path $taskConfig 'scratchpad\globalPlugins') -Force
    Copy-Item -LiteralPath (Join-Path $PSScriptRoot 'ci\nvda-capture.py') -Destination (Join-Path $taskConfig 'scratchpad\globalPlugins\fbforgeCapture.py')
    $config = @'
[general]
language = en
saveConfigurationOnExit = False
askToExit = False
playStartAndExitSounds = False
showWelcomeDialogAtStartup = False
[speech]
synth = silence
[keyboard]
handleInjectedKeys = True
[development]
enableScratchpadDir = True
[update]
autoCheck = False
startupNotification = False
allowUsageStats = False
askedAllowUsageStats = True
[addonStore]
automaticUpdates = disabled
'@
    [IO.File]::WriteAllText((Join-Path $taskConfig 'nvda.ini'), $config, [Text.UTF8Encoding]::new($false))
    $setup = Start-Process -FilePath ([IO.Path]::GetFullPath($InstallerPath)) -ArgumentList ('/S /D=' + $taskInstall) -PassThru
    if (-not $setup.WaitForExit(120000) -or $setup.ExitCode -ne 0) { throw 'a11y_install_failed' }
    $binary = Join-Path $taskInstall 'fbforge-inspect.exe'
    if ($ExpectedBinaryPath) {
        $installedHash = (Get-FileHash -LiteralPath $binary -Algorithm SHA256).Hash
        $expectedHash = (Get-FileHash -LiteralPath $ExpectedBinaryPath -Algorithm SHA256).Hash
        if ($installedHash -ne $expectedHash) {
            $installedInfo = Get-Item -LiteralPath $binary
            $expectedInfo = Get-Item -LiteralPath $ExpectedBinaryPath
            $installedBytes = [IO.File]::ReadAllBytes($binary)
            $expectedBytes = [IO.File]::ReadAllBytes($ExpectedBinaryPath)
            $differenceCount = 0
            $firstDifferences = [Collections.Generic.List[string]]::new()
            for ($offset = 0; $offset -lt [Math]::Min($installedBytes.Length, $expectedBytes.Length); $offset++) {
                if ($installedBytes[$offset] -ne $expectedBytes[$offset]) {
                    $differenceCount++
                    if ($firstDifferences.Count -lt 32) { $firstDifferences.Add(('{0:X}:{1:X2}/{2:X2}' -f $offset, $installedBytes[$offset], $expectedBytes[$offset])) }
                }
            }
            throw ('a11y_binary_revision_mismatch differences=' + $differenceCount + ' firstOffsetsInstalledExpected=' + ($firstDifferences -join ',') + ' installed=' + $installedHash + ' expected=' + $expectedHash + ' installedBytes=' + $installedInfo.Length + ' expectedBytes=' + $expectedInfo.Length + ' installedVersion=' + $installedInfo.VersionInfo.ProductVersion + ' expectedVersion=' + $expectedInfo.VersionInfo.ProductVersion)
        }
    }
    $taskApp = [Diagnostics.Process]::Start((Start-Clean $binary))
    $taskWindow = Wait-Check {
        if ($taskApp.HasExited) { throw 'a11y_app_exited' }
        $condition = [System.Windows.Automation.PropertyCondition]::new([System.Windows.Automation.AutomationElement]::ProcessIdProperty, $taskApp.Id)
        [System.Windows.Automation.AutomationElement]::RootElement.FindFirst([System.Windows.Automation.TreeScope]::Children, $condition)
    } 'a11y_window_missing'
    if ($taskWindow.Current.Name -ne 'FBForge Inspect') { throw 'a11y_title_mismatch' }
    $info = Start-Clean $NvdaPath ('--minimal --config-path="' + $taskConfig + '" --log-file="' + (Join-Path $taskRoot 'nvda.log') + '"')
    $info.EnvironmentVariables['INSPECT_A11Y_CAPTURE'] = $taskCapture
    $info.EnvironmentVariables['INSPECT_A11Y_WINDOW'] = [string]$taskWindow.Current.NativeWindowHandle
    $taskNvda = [Diagnostics.Process]::Start($info)
    $ready = Wait-Check {
        if ($taskNvda.HasExited) { throw 'nvda_exited' }
        @(Read-Capture) | Where-Object { $_.PSObject.Properties.Name -contains 'ready' -and $_.ready } | Select-Object -First 1
    } 'nvda_observer_not_ready' 60
    if ($ready.nvdaVersion -ne '2026.2') { throw 'nvda_version_mismatch' }
    $buttonType = [System.Windows.Automation.ControlType]::Button
    $editType = [System.Windows.Automation.ControlType]::Edit
    $destination = Check-Spoken 'Destination prüfen' $buttonType
    Invoke-Control $destination
    $field = Wait-Check { Find-Control 'Destination-ID' $editType } 'a11y_destination_field_missing'
    $point = $field.GetClickablePoint()
    [AccessibilityKeys]::ActivateInput([int]$point.X, [int]$point.Y)
    $taskInputActive = $true
    $null = Check-Spoken 'Destination-ID' $editType
    $null = Check-Spoken 'Umgebung' ([System.Windows.Automation.ControlType]::ComboBox)
    $null = Check-Spoken 'API' ([System.Windows.Automation.ControlType]::ComboBox)
    $null = Check-Spoken 'Darstellung wählen' ([System.Windows.Automation.ControlType]::Button)
    foreach ($name in @('Routing prüfen','Prozessnachricht prüfen','Schlüssel prüfen','Fachdatenschemata')) {
        $control = Check-Spoken $name $buttonType
        Invoke-Control $control
    }
    $null = Check-Spoken 'JSON Schema / XSD' $editType
    $help = Check-Spoken 'Hilfe' $buttonType
    Invoke-Control $help
    # Repeat the proven native engine/export flow against the installed resources,
    # after stopping the screen reader and its app so they cannot intercept input.
    if ($taskNvda -and -not $taskNvda.HasExited) { $taskNvda.Kill(); $taskNvda.WaitForExit() }
    if ($taskNvda) { $taskNvda.Dispose(); $taskNvda = $null }
    $null = $taskApp.CloseMainWindow()
    if (-not $taskApp.WaitForExit(10000)) { $taskApp.Kill(); $taskApp.WaitForExit() }
    $taskApp.Dispose(); $taskApp = $null; $taskWindow = $null
    $repository = Split-Path $PSScriptRoot -Parent
    $probeScript = Join-Path $repository 'tests\native-windows.mjs'
    $probeOutput = Join-Path $repository '.local\installed-native-evidence'
    $node = (Get-Command node.exe -CommandType Application | Select-Object -First 1).Source
    $probeInfo = Start-Clean $node ('"' + $probeScript + '" "' + $binary + '" "' + $probeOutput + '"')
    $probeInfo.EnvironmentVariables['CANDIDATE_SOURCE_COMMIT'] = $CandidateSourceCommit
    $probe = [Diagnostics.Process]::Start($probeInfo)
    try {
        if (-not $probe.WaitForExit(240000)) { $probe.Kill(); $probe.WaitForExit(); throw 'a11y_installed_native_probe_timeout' }
        if ($probe.ExitCode -ne 0) { throw 'a11y_installed_native_probe_failed' }
    } finally { $probe.Dispose() }
    Write-Output ('Actual NVDA 2026.2 speech and native Tab navigation: ' + $checks.Count + ' labels passed')
    $report = [ordered]@{ application = 'FBForge Inspect'; nvda = $ready.nvdaVersion; installerSha256 = (Get-FileHash -LiteralPath $InstallerPath -Algorithm SHA256).Hash.ToLowerInvariant(); harnessCommit = $env:GITHUB_SHA; candidateSourceCommit = $CandidateSourceCommit; runId = $env:GITHUB_RUN_ID; installedBinarySha256 = (Get-FileHash -LiteralPath $binary -Algorithm SHA256).Hash.ToLowerInvariant(); candidate = $(if ($ExpectedBinaryPath) { 'installed EXE matched expected SHA-256' } else { 'installer SHA-256 checked' }); scope = 'Installed Windows WebView; generated speech sequences with silent synthesizer, not an auditory usability assessment'; checks = @($checks.ToArray()); initialWebViewActivation = 'one pointer click in empty Destination-ID field'; keyboardTab = 'passed'; installedNativeEngineAndExports = 'passed'; modalFocusRestoration = 'verified by native export probe' }
    [IO.File]::WriteAllText((Join-Path $env:RUNNER_TEMP 'fbforge-accessibility-result.json'), ($report | ConvertTo-Json -Depth 5), [Text.UTF8Encoding]::new($false))
} catch {
    # The test never enters credentials. Log only known synthetic control names.
    $knownNames = @('Umgebung','API','Darstellung wählen','Destination prüfen','Destination-ID','Routing prüfen','Prozessnachricht prüfen','Schlüssel prüfen','Fachdatenschemata','JSON Schema / XSD','Hilfe')
    $focused = [System.Windows.Automation.AutomationElement]::FocusedElement
    if ($focused) {
        $label = if ($knownNames -contains $focused.Current.Name) { $focused.Current.Name } else { '(other control)' }
        $foreground = $taskWindow -and [AccessibilityKeys]::GetForegroundWindow() -eq [IntPtr]$taskWindow.Current.NativeWindowHandle
        Write-Host ('Accessibility failure focus: ' + $label + '; role=' + $focused.Current.ControlType.ProgrammaticName + '; foreground=' + $foreground)
    }
    Write-Host ('Observed capture records: ' + @(Read-Capture).Count)
    # This flow enters no values and reads only our own credential-free UI.
    foreach ($record in @(Read-Capture | Where-Object { $_.PSObject.Properties.Name -contains 'text' } | Select-Object -Last 8)) {
        Write-Host ('Observed NVDA speech: ' + ($record | ConvertTo-Json -Compress))
    }
    foreach ($record in @(Read-Capture | Where-Object { $_.PSObject.Properties.Name -contains 'key' } | Select-Object -Last 12)) { Write-Host ('Observed NVDA key: ' + ($record | ConvertTo-Json -Compress)) }
    throw
} finally {
    if ($taskNvda) { if (-not $taskNvda.HasExited) { $taskNvda.Kill(); $taskNvda.WaitForExit() }; $taskNvda.Dispose() }
    if ($taskApp) {
        $java = @(Get-CimInstance Win32_Process -Filter ("ParentProcessId = " + $taskApp.Id + " AND Name = 'java.exe'"))
        if (-not $taskApp.HasExited) {
            $null = $taskApp.CloseMainWindow()
            if (-not $taskApp.WaitForExit(10000)) { $taskApp.Kill(); $taskApp.WaitForExit() }
        }
        foreach ($child in $java) { Stop-Process -Id $child.ProcessId -Force -ErrorAction SilentlyContinue }
        $taskApp.Dispose()
    }
    $uninstaller = Join-Path $taskInstall 'uninstall.exe'
    if (Test-Path -LiteralPath $uninstaller) {
        $uninstall = Start-Process -FilePath $uninstaller -ArgumentList ('/S _?=' + $taskInstall) -PassThru
        if (-not $uninstall.WaitForExit(120000)) { $uninstall.Kill(); throw 'a11y_uninstall_timeout' }
        if ($uninstall.ExitCode -ne 0) { throw 'a11y_uninstall_failed' }
        $uninstall.Dispose()
    }
    if (Test-Path -LiteralPath $taskRoot) { Remove-Item -LiteralPath $taskRoot -Recurse -Force }
}

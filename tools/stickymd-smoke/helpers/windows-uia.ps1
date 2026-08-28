[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [ValidateSet('export', 'tray-exit', 'tray-menu', 'tray-show')]
    [string]$Action,
    [Parameter(Mandatory = $true)]
    [int]$ProcessId,
    [string]$Path,
    [int]$TimeoutSeconds = 10
)

$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName UIAutomationTypes
Add-Type @'
using System;
using System.Runtime.InteropServices;
public static class StickyMdSmokeMouse {
    [DllImport("user32.dll", SetLastError=true)]
    public static extern bool SetCursorPos(int x, int y);
    [DllImport("user32.dll")]
    public static extern void mouse_event(uint flags, uint dx, uint dy, uint data, UIntPtr extra);
    [DllImport("user32.dll")]
    public static extern void keybd_event(byte virtualKey, byte scanCode, uint flags, UIntPtr extra);
}
'@

function Wait-Until([scriptblock]$Probe, [string]$Label) {
    $deadline = [DateTime]::UtcNow.AddSeconds($TimeoutSeconds)
    do {
        try {
            $value = & $Probe
            if ($null -ne $value) { return $value }
        } catch {
            # Native shell UI can be rebuilt while it is being enumerated.
        }
        Start-Sleep -Milliseconds 50
    } while ([DateTime]::UtcNow -lt $deadline)
    throw "Timed out waiting for $Label"
}

function Find-ProcessTopLevel([int]$OwnerPid, [string]$ExpectedName) {
    $root = [System.Windows.Automation.AutomationElement]::RootElement
    $condition = New-Object System.Windows.Automation.PropertyCondition(
        [System.Windows.Automation.AutomationElement]::ProcessIdProperty,
        $OwnerPid
    )
    $windows = $root.FindAll([System.Windows.Automation.TreeScope]::Children, $condition)
    foreach ($window in $windows) {
        if (-not $ExpectedName -or $window.Current.Name -eq $ExpectedName) { return $window }
    }
    return $null
}

function Invoke-ExportDialog {
    if (-not $Path) { throw 'export action requires -Path' }
    $exportLabel = ([string][char]0x5bfc) + ([string][char]0x51fa)
    $dialog = Wait-Until { Find-ProcessTopLevel $ProcessId $exportLabel } 'native export dialog'
    $editCondition = New-Object System.Windows.Automation.PropertyCondition(
        [System.Windows.Automation.AutomationElement]::ControlTypeProperty,
        [System.Windows.Automation.ControlType]::Edit
    )
    $edits = $dialog.FindAll([System.Windows.Automation.TreeScope]::Descendants, $editCondition)
    $filename = $null
    foreach ($edit in $edits) {
        if (-not $edit.Current.IsEnabled) { continue }
        $pattern = $null
        if ($edit.TryGetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern, [ref]$pattern)) {
            if ($edit.Current.AutomationId -eq '1001') { $filename = @($edit, $pattern); break }
            if ($null -eq $filename) { $filename = @($edit, $pattern) }
        }
    }
    if ($null -eq $filename) { throw 'Native export dialog has no writable filename control' }
    $filename[1].SetValue([IO.Path]::GetFullPath($Path))

    $buttonCondition = New-Object System.Windows.Automation.AndCondition(
        (New-Object System.Windows.Automation.PropertyCondition(
            [System.Windows.Automation.AutomationElement]::ControlTypeProperty,
            [System.Windows.Automation.ControlType]::Button
        )),
        (New-Object System.Windows.Automation.PropertyCondition(
            [System.Windows.Automation.AutomationElement]::NameProperty,
            $exportLabel
        ))
    )
    $button = $dialog.FindFirst([System.Windows.Automation.TreeScope]::Descendants, $buttonCondition)
    if ($null -eq $button) { throw 'Native export dialog has no export button' }
    $invoke = $button.GetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern)
    $invoke.Invoke()
    Write-Output 'UIA_EXPORT_SUBMITTED'
}

function Find-TrayIcon {
    $root = [System.Windows.Automation.AutomationElement]::RootElement
    $name = New-Object System.Windows.Automation.PropertyCondition(
        [System.Windows.Automation.AutomationElement]::NameProperty,
        'StickyMD'
    )
    foreach ($top in $root.FindAll([System.Windows.Automation.TreeScope]::Children, [System.Windows.Automation.Condition]::TrueCondition)) {
        try {
            $matches = $top.FindAll([System.Windows.Automation.TreeScope]::Descendants, $name)
            foreach ($match in $matches) {
                if ($match.Current.AutomationId -eq 'NotifyItemIcon') { return $match }
            }
        } catch { }
    }
    return $null
}

function Open-TrayOverflow {
    $root = [System.Windows.Automation.AutomationElement]::RootElement
    $class = New-Object System.Windows.Automation.PropertyCondition(
        [System.Windows.Automation.AutomationElement]::ClassNameProperty,
        'Shell_TrayWnd'
    )
    $taskbar = $root.FindFirst([System.Windows.Automation.TreeScope]::Children, $class)
    if ($null -eq $taskbar) { throw 'Windows taskbar is unavailable' }
    $id = New-Object System.Windows.Automation.PropertyCondition(
        [System.Windows.Automation.AutomationElement]::AutomationIdProperty,
        'SystemTrayIcon'
    )
    $button = $taskbar.FindFirst([System.Windows.Automation.TreeScope]::Descendants, $id)
    if ($null -eq $button) { throw 'Taskbar overflow button is unavailable' }
    $invoke = $button.GetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern)
    $invoke.Invoke()
}

function Open-StickyMdTrayMenu {
    $icon = Find-TrayIcon
    if ($null -eq $icon) {
        Open-TrayOverflow
        $null = Wait-Until { Find-TrayIcon } 'StickyMD tray icon'
    }
    $null = Wait-Until {
        $current = Find-TrayIcon
        if ($null -eq $current) { return $null }
        $rect = $current.Current.BoundingRectangle
        if ($rect.Width -le 0 -or $rect.Height -le 0) { return $null }
        $x = [int]($rect.X + $rect.Width / 2)
        $y = [int]($rect.Y + $rect.Height / 2)
        if ([StickyMdSmokeMouse]::SetCursorPos($x, $y)) { return $current }
        return $null
    } 'movable StickyMD tray icon'
    [StickyMdSmokeMouse]::mouse_event(0x0008, 0, 0, 0, [UIntPtr]::Zero)
    [StickyMdSmokeMouse]::mouse_event(0x0010, 0, 0, 0, [UIntPtr]::Zero)
}

function Find-ProcessMenuItems {
    $root = [System.Windows.Automation.AutomationElement]::RootElement
    $process = New-Object System.Windows.Automation.PropertyCondition(
        [System.Windows.Automation.AutomationElement]::ProcessIdProperty,
        $ProcessId
    )
    $type = New-Object System.Windows.Automation.PropertyCondition(
        [System.Windows.Automation.AutomationElement]::ControlTypeProperty,
        [System.Windows.Automation.ControlType]::MenuItem
    )
    $items = New-Object System.Collections.Generic.List[object]
    foreach ($top in $root.FindAll([System.Windows.Automation.TreeScope]::Children, $process)) {
        try {
            foreach ($item in $top.FindAll([System.Windows.Automation.TreeScope]::Descendants, $type)) {
                $parent = [System.Windows.Automation.TreeWalker]::ControlViewWalker.GetParent($item)
                # Every native top-level window exposes a synthetic "System"
                # item beneath SystemMenuBar. It is unrelated to the tray
                # popup and must not become product-menu evidence.
                if ($null -ne $parent -and $parent.Current.AutomationId -eq 'SystemMenuBar') { continue }
                if ($item.Current.IsEnabled -and $item.Current.Name) { $items.Add($item) }
            }
        } catch { }
    }
    if ($items.Count -eq 0) { return $null }
    return $items.ToArray()
}

function Convert-NameToHex([string]$Name) {
    return (($Name.ToCharArray() | ForEach-Object { '{0:X4}' -f [int]$_ }) -join '-')
}

function Invoke-TrayMenuItem([string]$ExpectedName, [string]$Label) {
    Open-StickyMdTrayMenu
    $item = Wait-Until {
        foreach ($candidate in @(Find-ProcessMenuItems)) {
            if ($candidate.Current.Name -eq $ExpectedName) { return $candidate }
        }
        return $null
    } $Label
    $invoke = $item.GetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern)
    $invoke.Invoke()
}

function Inspect-TrayMenu {
    Open-StickyMdTrayMenu
    $items = Wait-Until { Find-ProcessMenuItems } 'StickyMD tray menu items'
    foreach ($item in @($items)) {
        Write-Output ('UIA_TRAY_ITEM_HEX=' + (Convert-NameToHex $item.Current.Name))
    }
    [StickyMdSmokeMouse]::keybd_event(0x1b, 0x01, 0, [UIntPtr]::Zero)
    [StickyMdSmokeMouse]::keybd_event(0x1b, 0x01, 0x0002, [UIntPtr]::Zero)
}

function Invoke-TrayExit {
    $exitLabel = ([string][char]0x9000) + ([string][char]0x51fa)
    Invoke-TrayMenuItem $exitLabel 'StickyMD Exit tray item'
    Write-Output 'UIA_TRAY_EXIT_SUBMITTED'
}

function Invoke-TrayShow {
    $showLabel = ([string][char]0x663e) + ([string][char]0x793a)
    Invoke-TrayMenuItem $showLabel 'StickyMD Show tray item'
    Write-Output 'UIA_TRAY_SHOW_SUBMITTED'
}

switch ($Action) {
    'export' { Invoke-ExportDialog }
    'tray-exit' { Invoke-TrayExit }
    'tray-menu' { Inspect-TrayMenu }
    'tray-show' { Invoke-TrayShow }
}

[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [ValidateSet('capture-window', 'export', 'tray-exit', 'tray-menu', 'tray-show')]
    [string]$Action,
    [Parameter(Mandatory = $true)]
    [int]$ProcessId,
    [string]$Path,
    [int]$TimeoutSeconds = 10
)

$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName UIAutomationTypes
Add-Type -AssemblyName System.Drawing
Add-Type @'
using System;
using System.Runtime.InteropServices;
public struct StickyMdSmokePoint {
    public int X;
    public int Y;
}
public static class StickyMdSmokeMouse {
    [DllImport("user32.dll", SetLastError=true)]
    public static extern bool SetCursorPos(int x, int y);
    [DllImport("user32.dll", SetLastError=true)]
    public static extern bool GetCursorPos(out StickyMdSmokePoint point);
    [DllImport("user32.dll")]
    public static extern void mouse_event(uint flags, uint dx, uint dy, uint data, UIntPtr extra);
    [DllImport("user32.dll")]
    public static extern void keybd_event(byte virtualKey, byte scanCode, uint flags, UIntPtr extra);
    [DllImport("user32.dll")]
    public static extern int GetSystemMetrics(int index);
}
'@

$TrayMenuOpenAttempts = 2
$TrayMenuOpenTimeoutMilliseconds = 1500
$TrayMenuCloseTimeoutMilliseconds = 1500
$TrayContainerClasses = @('Shell_TrayWnd', 'TopLevelWindowForOverflowXamlIsland')

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

function Find-StickyMdPaperWindow([int]$OwnerPid) {
    $root = [System.Windows.Automation.AutomationElement]::RootElement
    $condition = New-Object System.Windows.Automation.PropertyCondition(
        [System.Windows.Automation.AutomationElement]::ProcessIdProperty,
        $OwnerPid
    )
    foreach ($window in $root.FindAll([System.Windows.Automation.TreeScope]::Children, $condition)) {
        if ($window.Current.Name.StartsWith('StickyMD')) { return $window }
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

function Save-WindowCapture {
    if (-not $Path) { throw 'capture-window action requires -Path' }
    $window = Wait-Until { Find-StickyMdPaperWindow $ProcessId } 'StickyMD paper window'
    $rect = $window.Current.BoundingRectangle
    $width = [int][Math]::Ceiling($rect.Width)
    $height = [int][Math]::Ceiling($rect.Height)
    if ($width -le 0 -or $height -le 0) { throw 'StickyMD window has invalid capture geometry' }
    $target = [IO.Path]::GetFullPath($Path)
    $parent = [IO.Path]::GetDirectoryName($target)
    if ($parent) { [IO.Directory]::CreateDirectory($parent) | Out-Null }
    $bitmap = New-Object System.Drawing.Bitmap($width, $height)
    try {
        $graphics = [System.Drawing.Graphics]::FromImage($bitmap)
        try {
            $graphics.CopyFromScreen(
                [int][Math]::Floor($rect.X),
                [int][Math]::Floor($rect.Y),
                0,
                0,
                (New-Object System.Drawing.Size($width, $height)),
                [System.Drawing.CopyPixelOperation]::SourceCopy
            )
        } finally {
            $graphics.Dispose()
        }
        $bitmap.Save($target, [System.Drawing.Imaging.ImageFormat]::Png)
    } finally {
        $bitmap.Dispose()
    }
    Write-Output ('UIA_WINDOW_CAPTURE=' + $target)
}

function Find-StickyMdTrayTarget {
    $root = [System.Windows.Automation.AutomationElement]::RootElement
    $name = New-Object System.Windows.Automation.PropertyCondition(
        [System.Windows.Automation.AutomationElement]::NameProperty,
        'StickyMD'
    )
    $id = New-Object System.Windows.Automation.PropertyCondition(
        [System.Windows.Automation.AutomationElement]::AutomationIdProperty,
        'NotifyItemIcon'
    )
    $iconCondition = New-Object System.Windows.Automation.AndCondition($name, $id)
    $virtualLeft = [StickyMdSmokeMouse]::GetSystemMetrics(76)
    $virtualTop = [StickyMdSmokeMouse]::GetSystemMetrics(77)
    $virtualRight = $virtualLeft + [StickyMdSmokeMouse]::GetSystemMetrics(78)
    $virtualBottom = $virtualTop + [StickyMdSmokeMouse]::GetSystemMetrics(79)

    # Restrict enumeration to Explorer's two known tray containers. Walking every
    # top-level provider can block indefinitely and can surface stale same-name
    # nodes left behind by a display-topology rebuild.
    foreach ($className in $TrayContainerClasses) {
        $class = New-Object System.Windows.Automation.PropertyCondition(
            [System.Windows.Automation.AutomationElement]::ClassNameProperty,
            $className
        )
        foreach ($container in $root.FindAll([System.Windows.Automation.TreeScope]::Children, $class)) {
            try {
                foreach ($match in $container.FindAll([System.Windows.Automation.TreeScope]::Descendants, $iconCondition)) {
                    $current = $match.Current
                    $rect = $current.BoundingRectangle
                    if (-not $current.IsEnabled -or $current.IsOffscreen) { continue }
                    if ($rect.Width -le 0 -or $rect.Height -le 0) { continue }
                    $x = [int]($rect.X + $rect.Width / 2)
                    $y = [int]($rect.Y + $rect.Height / 2)
                    if ($x -lt $virtualLeft -or $x -ge $virtualRight -or
                        $y -lt $virtualTop -or $y -ge $virtualBottom) { continue }
                    return [PSCustomObject]@{
                        X = $x
                        Y = $y
                        Geometry = ('container={0};x={1};y={2};width={3};height={4}' -f
                            $className, [int]$rect.X, [int]$rect.Y,
                            [int]$rect.Width, [int]$rect.Height)
                    }
                }
            } catch { }
        }
    }
    return $null
}

function Test-TrayOverflowVisible {
    $root = [System.Windows.Automation.AutomationElement]::RootElement
    $class = New-Object System.Windows.Automation.PropertyCondition(
        [System.Windows.Automation.AutomationElement]::ClassNameProperty,
        'TopLevelWindowForOverflowXamlIsland'
    )
    foreach ($window in $root.FindAll([System.Windows.Automation.TreeScope]::Children, $class)) {
        try {
            $current = $window.Current
            $rect = $current.BoundingRectangle
            if (-not $current.IsOffscreen -and $rect.Width -gt 0 -and $rect.Height -gt 0) {
                return $true
            }
        } catch { }
    }
    return $false
}

function Invoke-TrayOverflowToggle {
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

function Open-TrayOverflow {
    if (-not (Test-TrayOverflowVisible)) { Invoke-TrayOverflowToggle }
}

function Close-TrayOverflow {
    if (Test-TrayOverflowVisible) { Invoke-TrayOverflowToggle }
}

function Resolve-StickyMdTrayTarget {
    $target = Find-StickyMdTrayTarget
    $openedOverflow = $false
    if ($null -eq $target) {
        $openedOverflow = -not (Test-TrayOverflowVisible)
        Open-TrayOverflow
        try {
            $target = Wait-Until { Find-StickyMdTrayTarget } 'usable StickyMD tray icon'
        } catch {
            if ($openedOverflow) {
                try { Close-TrayOverflow } catch { }
            }
            throw
        }
    }
    $deadline = [DateTime]::UtcNow.AddSeconds($TimeoutSeconds)
    $lastGeometry = $target.Geometry
    $lastCursor = '<unavailable>'
    do {
        $current = Find-StickyMdTrayTarget
        if ($null -eq $current) {
            $lastGeometry = '<none>'
            Start-Sleep -Milliseconds 50
            continue
        }
        $lastGeometry = $current.Geometry
        if (-not [StickyMdSmokeMouse]::SetCursorPos($current.X, $current.Y)) {
            Start-Sleep -Milliseconds 50
            continue
        }
        $actual = New-Object StickyMdSmokePoint
        if (-not [StickyMdSmokeMouse]::GetCursorPos([ref]$actual)) {
            Start-Sleep -Milliseconds 50
            continue
        }
        $lastCursor = ('x={0};y={1}' -f $actual.X, $actual.Y)
        if ($actual.X -eq $current.X -and $actual.Y -eq $current.Y) { return $current }
        Start-Sleep -Milliseconds 50
    } while ([DateTime]::UtcNow -lt $deadline)
    if ($openedOverflow) {
        try { Close-TrayOverflow } catch { }
    }
    throw ('Timed out waiting for movable StickyMD tray icon; candidate={0}; cursor={1}' -f
        $lastGeometry, $lastCursor)
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
                $current = $item.Current
                $rect = $current.BoundingRectangle
                if (-not $current.IsEnabled -or $current.IsOffscreen -or -not $current.Name) { continue }
                if ($rect.Width -le 0 -or $rect.Height -le 0) { continue }
                # Snapshot volatile UIA properties while the native popup is
                # live. The provider can invalidate Current immediately after
                # Escape/Invoke, so diagnostics must not re-read stale nodes.
                $items.Add([PSCustomObject]@{
                    Element = $item
                    Name = $current.Name
                    Geometry = ('x={0};y={1};width={2};height={3}' -f
                        [int]$rect.X, [int]$rect.Y, [int]$rect.Width, [int]$rect.Height)
                })
            }
        } catch { }
    }
    if ($items.Count -eq 0) { return @() }
    return $items.ToArray()
}

function Convert-NameToHex([string]$Name) {
    return (($Name.ToCharArray() | ForEach-Object { '{0:X4}' -f [int]$_ }) -join '-')
}

function Format-MenuItems([object[]]$Items) {
    if ($null -eq $Items -or $Items.Count -eq 0) { return '<none>' }
    return (($Items | ForEach-Object {
        '{0}@{1}' -f (Convert-NameToHex $_.Name), $_.Geometry
    }) -join ',')
}

function Wait-ForProcessMenuItems([int]$TimeoutMilliseconds) {
    $deadline = [DateTime]::UtcNow.AddMilliseconds($TimeoutMilliseconds)
    do {
        $items = @(Find-ProcessMenuItems)
        if ($items.Count -gt 0) { return $items }
        Start-Sleep -Milliseconds 25
    } while ([DateTime]::UtcNow -lt $deadline)
    return @()
}

function Wait-ForProcessMenuClosed([int]$TimeoutMilliseconds) {
    $deadline = [DateTime]::UtcNow.AddMilliseconds($TimeoutMilliseconds)
    do {
        if (@(Find-ProcessMenuItems).Count -eq 0) { return $true }
        Start-Sleep -Milliseconds 25
    } while ([DateTime]::UtcNow -lt $deadline)
    return $false
}

function Close-StickyMdTrayMenu {
    [StickyMdSmokeMouse]::keybd_event(0x1b, 0x01, 0, [UIntPtr]::Zero)
    [StickyMdSmokeMouse]::keybd_event(0x1b, 0x01, 0x0002, [UIntPtr]::Zero)
    if (-not (Wait-ForProcessMenuClosed $TrayMenuCloseTimeoutMilliseconds)) {
        $observed = @(Find-ProcessMenuItems)
        throw ('StickyMD tray menu did not close after Escape; observed_items={0}' -f
            (Format-MenuItems $observed))
    }
}

function Open-StickyMdTrayMenu {
    $existing = @(Find-ProcessMenuItems)
    if ($existing.Count -gt 0) { return $existing }

    $lastGeometry = '<unavailable>'
    $lastCursor = '<unavailable>'
    for ($attempt = 1; $attempt -le $TrayMenuOpenAttempts; $attempt++) {
        $target = Resolve-StickyMdTrayTarget
        $lastGeometry = $target.Geometry
        $lastCursor = ('x={0};y={1}' -f $target.X, $target.Y)
        [StickyMdSmokeMouse]::mouse_event(0x0008, 0, 0, 0, [UIntPtr]::Zero)
        [StickyMdSmokeMouse]::mouse_event(0x0010, 0, 0, 0, [UIntPtr]::Zero)

        $items = @(Wait-ForProcessMenuItems $TrayMenuOpenTimeoutMilliseconds)
        if ($items.Count -gt 0) { return $items }

        # A desktop notification or USER input may have taken the physical
        # click after SetCursorPos. Close any unrelated popup before the one
        # permitted retry; this harness requires an otherwise exclusive desktop.
        [StickyMdSmokeMouse]::keybd_event(0x1b, 0x01, 0, [UIntPtr]::Zero)
        [StickyMdSmokeMouse]::keybd_event(0x1b, 0x01, 0x0002, [UIntPtr]::Zero)
        $null = Wait-ForProcessMenuClosed $TrayMenuCloseTimeoutMilliseconds
    }

    $observed = @(Find-ProcessMenuItems)
    throw ('StickyMD tray menu did not open after {0} attempts; icon={1}; cursor={2}; observed_items={3}' -f
        $TrayMenuOpenAttempts, $lastGeometry, $lastCursor, (Format-MenuItems $observed))
}

function Invoke-TrayMenuItem([string]$ExpectedName, [string]$Label) {
    $items = @(Open-StickyMdTrayMenu)
    $item = $items | Where-Object { $_.Name -eq $ExpectedName } | Select-Object -First 1
    if ($null -eq $item) {
        $observed = Format-MenuItems $items
        Close-StickyMdTrayMenu
        throw ('{0} is absent from the opened StickyMD tray menu; observed_items={1}' -f
            $Label, $observed)
    }
    $invoke = $item.Element.GetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern)
    $invoke.Invoke()
    if (-not (Wait-ForProcessMenuClosed $TrayMenuCloseTimeoutMilliseconds)) {
        throw "$Label did not close the StickyMD tray menu"
    }
}

function Inspect-TrayMenu {
    $items = @(Open-StickyMdTrayMenu)
    foreach ($item in @($items)) {
        Write-Output ('UIA_TRAY_ITEM_HEX=' + (Convert-NameToHex $item.Name))
    }
    Close-StickyMdTrayMenu
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
    'capture-window' { Save-WindowCapture }
    'export' { Invoke-ExportDialog }
    'tray-exit' { Invoke-TrayExit }
    'tray-menu' { Inspect-TrayMenu }
    'tray-show' { Invoke-TrayShow }
}

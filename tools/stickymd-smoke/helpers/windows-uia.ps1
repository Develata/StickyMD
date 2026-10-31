[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [ValidateSet('capture-window', 'export', 'tray-target', 'tray-targets', 'tray-menu-items', 'tray-menu-closed', 'tray-invoke', 'tray-overflow-close')]
    [string]$Action,
    [Parameter(Mandatory = $true)]
    [int]$ProcessId,
    [string]$Path,
    [string]$Item,
    [int]$TimeoutSeconds = 10
)

$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName UIAutomationTypes
Add-Type -AssemblyName System.Drawing
Add-Type @'
using System;
using System.Runtime.InteropServices;
public static class StickyMdSmokeScreen {
    [DllImport("user32.dll")]
    public static extern int GetSystemMetrics(int index);
}
'@

# The Rust caller places the cursor, opens, retries and closes the tray menu: it owns all input
# injection and cursor placement (plan 11 physical input injection). This helper only locates,
# reads and invokes through UI Automation, and says when it could not observe.
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

function Find-StickyMdTrayTargets {
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
    $virtualLeft = [StickyMdSmokeScreen]::GetSystemMetrics(76)
    $virtualTop = [StickyMdSmokeScreen]::GetSystemMetrics(77)
    $virtualRight = $virtualLeft + [StickyMdSmokeScreen]::GetSystemMetrics(78)
    $virtualBottom = $virtualTop + [StickyMdSmokeScreen]::GetSystemMetrics(79)

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
                    [PSCustomObject]@{
                        X = $x
                        Y = $y
                        Geometry = ('container={0};x={1};y={2};width={3};height={4}' -f
                            $className, [int]$rect.X, [int]$rect.Y,
                            [int]$rect.Width, [int]$rect.Height)
                    }
                }
            } catch { throw "Tray icon enumeration was incomplete: $_" }
        }
    }
}

function Find-StickyMdTrayTarget {
    Find-StickyMdTrayTargets | Select-Object -First 1
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

# The overflow flyout's state as observed: 'visible', 'hidden' (every flyout window read and none
# shown), or 'unobservable' (any read failed). Unlike Test-TrayOverflowVisible, a failed read is
# never taken for closed.
function Get-TrayOverflowState {
    $root = [System.Windows.Automation.AutomationElement]::RootElement
    $class = New-Object System.Windows.Automation.PropertyCondition(
        [System.Windows.Automation.AutomationElement]::ClassNameProperty,
        'TopLevelWindowForOverflowXamlIsland'
    )
    try {
        foreach ($window in $root.FindAll([System.Windows.Automation.TreeScope]::Children, $class)) {
            $current = $window.Current
            $rect = $current.BoundingRectangle
            if (-not $current.IsOffscreen -and $rect.Width -gt 0 -and $rect.Height -gt 0) {
                return 'visible'
            }
        }
    } catch {
        return 'unobservable'
    }
    return 'hidden'
}

# Closes the flyout through UI Automation (no input injection) and waits, bounded, until it is
# observed hidden; an unobservable state is never success. Within the deadline it toggles once,
# on the first state observed visible (an unobservable read is retried, not skipped past), and
# never on an unobservable one. Throws with the last state otherwise.
function Close-TrayOverflowObserved {
    $toggled = $false
    $deadline = [DateTime]::UtcNow.AddSeconds($TimeoutSeconds)
    do {
        $state = Get-TrayOverflowState
        if ($state -eq 'hidden') { return }
        if ($state -eq 'visible' -and -not $toggled) {
            $toggled = $true
            Invoke-TrayOverflowToggle
        }
        Start-Sleep -Milliseconds 50
    } while ([DateTime]::UtcNow -lt $deadline)
    throw "the tray overflow flyout was not observed closed (last state: $state; toggled: $toggled)"
}

# Opens the flyout on one observed state, which decides both the toggle and the answer:
# 'opened' (observed hidden, toggled here) or 'already-open' (observed visible, untouched). A
# state that stays unobservable is retried briefly and then an error; nothing is toggled on a
# guess.
function Open-TrayOverflowObserved {
    $deadline = [DateTime]::UtcNow.AddMilliseconds(2000)
    do {
        $state = Get-TrayOverflowState
        if ($state -ne 'unobservable') { break }
        Start-Sleep -Milliseconds 50
    } while ([DateTime]::UtcNow -lt $deadline)
    if ($state -eq 'visible') { return 'already-open' }
    if ($state -eq 'hidden') {
        # Set before the toggle: one that takes effect and then throws still leaves a flyout to
        # close (Open-TrayOverflowUntil).
        $script:overflowToggleAttempted = $true
        Invoke-TrayOverflowToggle
        return 'opened'
    }
    throw 'the tray overflow flyout could not be observed before opening it; it was not toggled'
}

# Opens the flyout as Open-TrayOverflowObserved does and waits for $Ready (a Wait-Until probe).
# Returns whether this opened it, and the probe's result. Any failure after a toggle was
# attempted, including a toggle that took effect and then threw, closes the flyout (observed)
# before the error is rethrown with that outcome.
function Open-TrayOverflowUntil([scriptblock]$Ready, [string]$Label) {
    $script:overflowToggleAttempted = $false
    try {
        $opened = (Open-TrayOverflowObserved) -eq 'opened'
        $result = @(Wait-Until $Ready $Label)
        return [PSCustomObject]@{ Opened = $opened; Result = $result }
    } catch {
        if ($script:overflowToggleAttempted) { throw "$_; $(Close-OpenedTrayOverflow)" }
        throw
    }
}

# After a failed tray step that opened the flyout (`UIA_TRAY_OVERFLOW_OPENED`), the Rust caller
# asks for this; `UIA_TRAY_OVERFLOW_CLOSED` only when it is observed hidden.
function Close-TrayOverflowAndReport {
    Close-TrayOverflowObserved
    Write-Output 'UIA_TRAY_OVERFLOW_CLOSED'
}

# This enumeration's own cleanup before it fails: what happened to the flyout it opened, for the
# error it rethrows (the opening marker is never printed on that path).
function Close-OpenedTrayOverflow {
    try {
        Close-TrayOverflowObserved
        return 'the overflow flyout this enumeration opened is closed'
    } catch {
        return "the overflow flyout this enumeration opened was not closed: $_"
    }
}

function Resolve-StickyMdTrayTarget {
    $target = Find-StickyMdTrayTarget
    if ($null -ne $target) { return $target }
    $openedOverflow = -not (Test-TrayOverflowVisible)
    Open-TrayOverflow
    try {
        return Wait-Until { Find-StickyMdTrayTarget } 'usable StickyMD tray icon'
    } catch {
        if ($openedOverflow) {
            try { Close-TrayOverflow } catch { }
        }
        throw
    }
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
    # Any provider error leaves the snapshot unobserved: an empty list then means nothing.
    $observed = $true
    try {
        $tops = $root.FindAll([System.Windows.Automation.TreeScope]::Children, $process)
    } catch {
        return [PSCustomObject]@{ Items = @(); Observed = $false }
    }
    foreach ($top in $tops) {
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
        } catch {
            $observed = $false
        }
    }
    return [PSCustomObject]@{ Items = $items.ToArray(); Observed = $observed }
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

# Waits for menu items: the first observed snapshot with items decides, or else the latest
# snapshot (observed with no items, or unobserved). Whichever decides, the items the latest
# unobserved poll before it read until its provider failed come along as Partial, for
# diagnostics only.
function Wait-ForProcessMenuItems([int]$TimeoutMilliseconds) {
    $deadline = [DateTime]::UtcNow.AddMilliseconds($TimeoutMilliseconds)
    $partial = @()
    do {
        $snapshot = Find-ProcessMenuItems
        if ($snapshot.Observed -and @($snapshot.Items).Count -gt 0) {
            return [PSCustomObject]@{ Items = @($snapshot.Items); Observed = $true; Partial = $partial }
        }
        if (@($snapshot.Items).Count -gt 0) { $partial = @($snapshot.Items) }
        Start-Sleep -Milliseconds 25
    } while ([DateTime]::UtcNow -lt $deadline)
    return [PSCustomObject]@{ Items = @(); Observed = $snapshot.Observed; Partial = $partial }
}

# Only an observed snapshot with no items shows the menu closed.
function Wait-ForProcessMenuClosed([int]$TimeoutMilliseconds) {
    $deadline = [DateTime]::UtcNow.AddMilliseconds($TimeoutMilliseconds)
    do {
        $snapshot = Find-ProcessMenuItems
        if ($snapshot.Observed -and @($snapshot.Items).Count -eq 0) { return $true }
        Start-Sleep -Milliseconds 25
    } while ([DateTime]::UtcNow -lt $deadline)
    return $false
}

# A report line followed by the items read before a provider failed, if any; those are
# diagnostics only, never a menu.
function Write-WithPartial([string]$Line, [object[]]$Partial) {
    $partial = @($Partial)
    if ($partial.Count -gt 0) {
        Write-Output ($Line + '; partial_items=' + (Format-MenuItems $partial))
    } else {
        Write-Output $Line
    }
}

function Write-TrayTarget {
    # The menu must be observed closed before the right-click so that its opening is observed.
    $before = Find-ProcessMenuItems
    if (-not $before.Observed) {
        Write-WithPartial 'UIA_TRAY_MENU_UNOBSERVABLE' $before.Items
        return
    }
    $items = @($before.Items)
    Write-Output ('UIA_TRAY_MENU_ITEMS_BEFORE=' + $items.Count)
    if ($items.Count -gt 0) {
        Write-Output ('UIA_TRAY_MENU_OPEN; observed_items=' + (Format-MenuItems $items))
        return
    }
    $target = Resolve-StickyMdTrayTarget
    Write-Output ('UIA_TRAY_TARGET={0},{1}' -f $target.X, $target.Y)
    Write-Output ('UIA_TRAY_GEOMETRY=' + $target.Geometry)
}

# Enumerate every visible same-name icon. The Rust caller identifies a candidate
# only from the native popup owner PID, never from its Explorer UIA process ID.
function Write-TrayTargets {
    $before = Find-ProcessMenuItems
    if (-not $before.Observed) {
        Write-WithPartial 'UIA_TRAY_MENU_UNOBSERVABLE' $before.Items
        return
    }
    $items = @($before.Items)
    Write-Output ('UIA_TRAY_MENU_ITEMS_BEFORE=' + $items.Count)
    if ($items.Count -gt 0) {
        Write-Output ('UIA_TRAY_MENU_OPEN; observed_items=' + (Format-MenuItems $items))
        return
    }
    if ($Item -notmatch '^[12]$') { throw 'tray-targets requires the owned instance count (1 or 2)' }
    $minimum = [int]$Item
    $targets = @(Find-StickyMdTrayTargets)
    if ($targets.Count -lt $minimum) {
        # The Rust caller closes a flyout this opened if its tray step then fails; a failure here
        # closes it before the error (Open-TrayOverflowUntil). Only a flyout observed hidden and
        # toggled here counts as opened; the Rust caller treats a failed call as unknown.
        $outcome = Open-TrayOverflowUntil {
            $found = @(Find-StickyMdTrayTargets)
            if ($found.Count -ge $minimum) { return $found }
            return $null
        } 'all owned StickyMD tray icons'
        $openedOverflow = $outcome.Opened
        $targets = @($outcome.Result)
        if ($targets.Count -gt 16 -and $openedOverflow) {
            throw "Too many same-name tray icons for a bounded search; $(Close-OpenedTrayOverflow)"
        }
        if ($openedOverflow) { Write-Output 'UIA_TRAY_OVERFLOW_OPENED' }
    }
    if ($targets.Count -gt 16) { throw 'Too many same-name tray icons for a bounded search' }
    foreach ($target in $targets) {
        Write-Output ('UIA_TRAY_TARGET={0},{1}' -f $target.X, $target.Y)
        Write-Output ('UIA_TRAY_GEOMETRY=' + $target.Geometry)
    }
}

function Write-TrayMenuItems {
    $snapshot = Wait-ForProcessMenuItems $TrayMenuOpenTimeoutMilliseconds
    if (-not $snapshot.Observed) {
        Write-WithPartial 'UIA_TRAY_MENU_UNOBSERVABLE' $snapshot.Partial
        return
    }
    $items = @($snapshot.Items)
    if ($items.Count -eq 0) {
        Write-WithPartial 'UIA_TRAY_MENU_NONE' $snapshot.Partial
        return
    }
    foreach ($menuItem in $items) {
        Write-Output ('UIA_TRAY_ITEM={0}@{1}' -f (Convert-NameToHex $menuItem.Name), $menuItem.Geometry)
    }
    if (@($snapshot.Partial).Count -gt 0) { Write-WithPartial 'UIA_TRAY_MENU_PARTIAL' $snapshot.Partial }
}

function Wait-TrayMenuClosed {
    if (-not (Wait-ForProcessMenuClosed $TrayMenuCloseTimeoutMilliseconds)) {
        $last = Find-ProcessMenuItems
        throw ('StickyMD tray menu was not observed closed; observed={0}; observed_items={1}' -f
            $last.Observed, (Format-MenuItems @($last.Items)))
    }
    Write-Output 'UIA_TRAY_MENU_CLOSED'
}

function Invoke-TrayMenuItem {
    if (-not $Item) { throw 'tray-invoke action requires -Item' }
    $snapshot = Find-ProcessMenuItems
    if (-not $snapshot.Observed) { throw 'the StickyMD tray menu could not be observed' }
    $items = @($snapshot.Items)
    $match = $items | Where-Object { (Convert-NameToHex $_.Name) -eq $Item } | Select-Object -First 1
    if ($null -eq $match) {
        throw ('tray item {0} is absent from the open StickyMD tray menu; observed_items={1}' -f
            $Item, (Format-MenuItems $items))
    }
    $invoke = $match.Element.GetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern)
    $invoke.Invoke()
    if (-not (Wait-ForProcessMenuClosed $TrayMenuCloseTimeoutMilliseconds)) {
        throw "tray item $Item did not close the StickyMD tray menu"
    }
    Write-Output ('UIA_TRAY_INVOKED=' + $Item)
}

# Read a native menu item's check state; absence of a supported pattern is an error,
# never an unchecked observation. Rust owns the expected state and its verdict.
switch ($Action) {
    'capture-window' { Save-WindowCapture }
    'export' { Invoke-ExportDialog }
    'tray-target' { Write-TrayTarget }
    'tray-targets' { Write-TrayTargets }
    'tray-menu-items' { Write-TrayMenuItems }
    'tray-menu-closed' { Wait-TrayMenuClosed }
    'tray-invoke' { Invoke-TrayMenuItem }
    'tray-overflow-close' { Close-TrayOverflowAndReport }
}

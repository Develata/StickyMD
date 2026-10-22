//! The StickyMD tray menu in exact qualification. The UIA helper locates the icon and reads or
//! invokes menu items, and says when it could not observe; the cursor placement and right-click
//! that open the menu and the Esc that closes it are injected here, after their targets are
//! verified (plan 11 physical input injection). The menu must be observed closed before the
//! right-click and open after it, so its opening is observed. Every helper report is parsed
//! strictly, and every failure reports what each attempt saw and how its cleanup went.
//!
//! Exact qualification runs one StickyMD process, so the first usable icon is its icon; picking
//! one copy's icon among two (USER decision D2, P08-A11) is C2d work.
//!
//! plan_ref: docs/plan/11_testing_and_release.md#physical-input-injection

use std::path::Path;

use super::run_uia;

/// A right-click on the icon is only a request; one retry is allowed (plan 11 G4-01).
pub(super) const TRAY_MENU_OPEN_ATTEMPTS: usize = 2;

/// Menu item names as the helper reports them: UTF-16 code units in hex.
const SHOW: &str = "663E-793A";
const HIDE: &str = "9690-85CF";
const TOPMOST: &str = "7F6E-9876";
const EXIT: &str = "9000-51FA";

fn helper(
    repository: &Path,
    action: &str,
    process_id: u32,
    item: Option<&str>,
) -> Result<String, String> {
    run_uia(repository, action, process_id, None, item)
}

/// The helper's report, one `(key, rest)` per non-blank line: the key runs up to the first `=`
/// or `;`, and the rest keeps that separator, so every parser matches the exact form of each
/// line. A report with no lines, or with a key not in `allowed`, is malformed.
fn report<'a>(
    output: &'a str,
    action: &str,
    allowed: &[&str],
) -> Result<Vec<(&'a str, &'a str)>, String> {
    let malformed = || format!("malformed {action} report: {output:?}");
    let lines = output
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| {
            let split = line.find(['=', ';']).unwrap_or(line.len());
            let (key, rest) = line.split_at(split);
            if allowed.contains(&key) {
                Ok((key, rest))
            } else {
                Err(malformed())
            }
        })
        .collect::<Result<Vec<_>, _>>()?;
    if lines.is_empty() {
        return Err(malformed());
    }
    Ok(lines)
}

/// The value of the one `key=value` line; `None` when the key is missing, repeated or has no
/// `=`.
fn one<'a>(lines: &[(&str, &'a str)], key: &str) -> Option<&'a str> {
    let mut found = lines.iter().filter(|(found, _)| *found == key);
    match (found.next(), found.next()) {
        (Some((_, rest)), None) => rest.strip_prefix('='),
        _ => None,
    }
}

/// A report of the single line `key`, alone or followed by the items read before a provider
/// failed (`; partial_items=…`). Returns that tail, for errors; those items are diagnostics
/// only, never a menu.
fn alone_with_partial<'a>(lines: &[(&str, &'a str)], key: &str) -> Option<&'a str> {
    match lines {
        [(found, rest)]
            if *found == key && (rest.is_empty() || rest.starts_with("; partial_items=")) =>
        {
            Some(rest)
        }
        _ => None,
    }
}

/// What `tray-menu-items` observed: an open menu's item names in menu order, or none, and the
/// items an earlier poll read before its provider failed (diagnostics only, also when the menu
/// then opened).
#[derive(Debug)]
struct MenuRead {
    items: Vec<String>,
    partial: String,
}

/// What `tray-target` saw before the right-click.
#[derive(Debug, PartialEq, Eq)]
enum Located {
    /// No menu open; the icon's centre and its geometry.
    Closed { x: i32, y: i32, geometry: String },
    /// A menu is already open (its report line).
    Open(String),
}

fn located(output: &str) -> Result<Located, String> {
    let malformed = || format!("malformed tray-target report: {output:?}");
    let lines = report(
        output,
        "tray-target",
        &[
            "UIA_TRAY_MENU_UNOBSERVABLE",
            "UIA_TRAY_MENU_ITEMS_BEFORE",
            "UIA_TRAY_MENU_OPEN",
            "UIA_TRAY_TARGET",
            "UIA_TRAY_GEOMETRY",
        ],
    )?;
    if let Some(partial) = alone_with_partial(&lines, "UIA_TRAY_MENU_UNOBSERVABLE") {
        return Err(format!(
            "the StickyMD tray menu could not be observed before the right-click{partial}"
        ));
    }
    let before = one(&lines, "UIA_TRAY_MENU_ITEMS_BEFORE")
        .filter(|count| !count.is_empty() && count.bytes().all(|byte| byte.is_ascii_digit()))
        .and_then(|count| count.parse::<usize>().ok())
        .ok_or_else(malformed)?;
    match (before, &lines[..]) {
        (0, [_, _, _]) => {
            let target = one(&lines, "UIA_TRAY_TARGET").ok_or_else(malformed)?;
            let geometry = one(&lines, "UIA_TRAY_GEOMETRY")
                .filter(|geometry| !geometry.is_empty())
                .ok_or_else(malformed)?;
            let (x, y) = target
                .split_once(',')
                .and_then(|(x, y)| Some((x.parse::<i32>().ok()?, y.parse::<i32>().ok()?)))
                .ok_or_else(malformed)?;
            Ok(Located::Closed {
                x,
                y,
                geometry: geometry.to_owned(),
            })
        }
        (1.., [_, ("UIA_TRAY_MENU_OPEN", rest)]) if rest.starts_with("; observed_items=") => {
            Ok(Located::Open(format!("UIA_TRAY_MENU_OPEN{rest}")))
        }
        _ => Err(malformed()),
    }
}

/// An item name: UTF-16 code units as four upper-case hex digits joined by `-`.
fn is_item_name(name: &str) -> bool {
    name.split('-').all(|unit| {
        unit.len() == 4
            && unit
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'A'..=b'F').contains(&byte))
    })
}

/// The item names of an open menu, in menu order; none when the helper observed no menu.
fn items(output: &str) -> Result<MenuRead, String> {
    let malformed = || format!("malformed tray-menu-items report: {output:?}");
    let lines = report(
        output,
        "tray-menu-items",
        &[
            "UIA_TRAY_MENU_UNOBSERVABLE",
            "UIA_TRAY_MENU_NONE",
            "UIA_TRAY_ITEM",
            "UIA_TRAY_MENU_PARTIAL",
        ],
    )?;
    match &lines[..] {
        _ if let Some(partial) = alone_with_partial(&lines, "UIA_TRAY_MENU_UNOBSERVABLE") => Err(
            format!("the StickyMD tray menu could not be observed{partial}"),
        ),
        _ if let Some(partial) = alone_with_partial(&lines, "UIA_TRAY_MENU_NONE") => Ok(MenuRead {
            items: Vec::new(),
            partial: partial.to_owned(),
        }),
        _ => {
            // An open menu's items may be followed by one line of earlier partial items.
            let (lines, partial) = match lines.split_last() {
                Some((("UIA_TRAY_MENU_PARTIAL", rest), items))
                    if rest.starts_with("; partial_items=") && !items.is_empty() =>
                {
                    (items, *rest)
                }
                _ => (&lines[..], ""),
            };
            let mut names: Vec<String> = Vec::with_capacity(lines.len());
            for &(key, rest) in lines {
                let (name, geometry) = rest
                    .strip_prefix('=')
                    .filter(|_| key == "UIA_TRAY_ITEM")
                    .and_then(|item| item.split_once('@'))
                    .ok_or_else(malformed)?;
                if !is_item_name(name)
                    || geometry.is_empty()
                    || names.iter().any(|seen| seen == name)
                {
                    return Err(malformed());
                }
                names.push(name.to_owned());
            }
            Ok(MenuRead {
                items: names,
                partial: partial.to_owned(),
            })
        }
    }
}

/// Whether the helper observed the menu closed.
fn closed(output: &str) -> Result<(), String> {
    match &report(output, "tray-menu-closed", &["UIA_TRAY_MENU_CLOSED"])?[..] {
        [("UIA_TRAY_MENU_CLOSED", "")] => Ok(()),
        _ => Err(format!("malformed tray-menu-closed report: {output:?}")),
    }
}

/// Whether the helper invoked exactly `item`.
fn invoked(output: &str, item: &str) -> Result<(), String> {
    let lines = report(output, "tray-invoke", &["UIA_TRAY_INVOKED"])?;
    match one(&lines, "UIA_TRAY_INVOKED") {
        Some(name) if name == item && lines.len() == 1 => Ok(()),
        _ => Err(format!(
            "malformed tray-invoke report for {item}: {output:?}"
        )),
    }
}

/// Closes the open menu: Esc reaches only StickyMD's active popup menu, which must then be
/// observed closed.
fn close(repository: &Path, process_id: u32) -> Result<(), String> {
    if !crate::window_control::escape_tray_popup(process_id)? {
        return Err("refusing Esc for the tray menu: no StickyMD popup menu is active".to_owned());
    }
    closed(&helper(repository, "tray-menu-closed", process_id, None)?)
}

/// How closing a menu that may be open after a failed step went: whether it is observed closed,
/// and what happened, for the error. Esc goes only to an active StickyMD popup; either way the
/// menu counts as closed only once the helper observes it closed.
fn cleanup(repository: &Path, process_id: u32) -> (bool, String) {
    let escaped = match crate::window_control::escape_tray_popup(process_id) {
        Ok(true) => "Esc pressed",
        Ok(false) => "no StickyMD popup was active",
        Err(error) => return (false, format!("cleanup not done: {error}")),
    };
    match helper(repository, "tray-menu-closed", process_id, None)
        .and_then(|output| closed(&output))
    {
        Ok(()) => (
            true,
            format!("cleanup: {escaped}; the menu is observed closed"),
        ),
        Err(error) => (
            false,
            format!("cleanup: {escaped}; the menu was not observed closed: {error}"),
        ),
    }
}

/// What one attempt to open the menu saw.
#[derive(Default)]
struct Attempt {
    geometry: Option<String>,
    cursor: Option<(i32, i32)>,
    outcome: String,
}

/// The attempts so far, for an error.
fn history(attempts: &[Attempt]) -> String {
    attempts
        .iter()
        .enumerate()
        .map(|(index, attempt)| {
            format!(
                "attempt {}: icon {}, cursor read back at {}: {}",
                index + 1,
                attempt.geometry.as_deref().unwrap_or("not located"),
                attempt
                    .cursor
                    .map_or("none".to_owned(), |(x, y)| format!("({x},{y})")),
                attempt.outcome
            )
        })
        .collect::<Vec<_>>()
        .join("; ")
}

/// An opened menu: its items and how it was opened.
struct Opened {
    items: Vec<String>,
    attempts: Vec<Attempt>,
}

impl Opened {
    fn observed(menu: MenuRead, mut attempt: Attempt, mut attempts: Vec<Attempt>) -> Self {
        attempt.outcome = format!("opened{}", menu.partial);
        attempts.push(attempt);
        Self {
            items: menu.items,
            attempts,
        }
    }

    /// An error for a step after the opening, with its cleanup and the opening's history.
    fn failed(&self, step: &str, cleanup: &str) -> String {
        format!(
            "{step}; {cleanup}; the menu had opened with {:?} after: {}",
            self.items,
            history(&self.attempts)
        )
    }
}

/// Opens the menu from observed-closed. Every failure records each attempt and how its cleanup
/// went.
fn open(repository: &Path, process_id: u32) -> Result<Opened, String> {
    let mut attempts: Vec<Attempt> = Vec::new();
    let failed =
        |attempts: &[Attempt]| format!("StickyMD tray menu not opened: {}", history(attempts));
    for _ in 0..TRAY_MENU_OPEN_ATTEMPTS {
        let mut attempt = Attempt::default();
        let report =
            helper(repository, "tray-target", process_id, None).and_then(|output| located(&output));
        let (x, y) = match report {
            Ok(Located::Closed { x, y, geometry }) => {
                attempt.geometry = Some(geometry);
                (x, y)
            }
            Ok(Located::Open(items)) => {
                // Its opening cannot be observed; close it and count the attempt.
                let closing = close(repository, process_id);
                attempt.outcome = match &closing {
                    Ok(()) => format!("a menu was already open ({items}); closed"),
                    Err(error) => format!("a menu was already open ({items}): {error}"),
                };
                attempts.push(attempt);
                if closing.is_err() {
                    return Err(failed(&attempts));
                }
                continue;
            }
            Err(error) => {
                attempt.outcome = format!("{error}; no cleanup needed, nothing was clicked");
                attempts.push(attempt);
                return Err(failed(&attempts));
            }
        };
        let clicked = crate::window_control::place_cursor_on_tray_icon(x, y, &mut attempt.cursor)
            .and_then(|()| crate::window_control::right_click_tray_icon(x, y));
        if let Err(error) = clicked {
            let (_, cleanup) = cleanup(repository, process_id);
            attempt.outcome = format!("{error}; {cleanup}");
            attempts.push(attempt);
            return Err(failed(&attempts));
        }
        let opened = helper(repository, "tray-menu-items", process_id, None)
            .and_then(|output| items(&output));
        match opened {
            Ok(menu) if !menu.items.is_empty() => {
                return Ok(Opened::observed(menu, attempt, attempts));
            }
            Ok(menu) => {
                // A notification or other input may have taken the click. A StickyMD popup that
                // opened late is closed first; unless the menu is then observed closed, nothing
                // more is clicked.
                let (known_closed, cleanup) = cleanup(repository, process_id);
                attempt.outcome =
                    format!("no menu after the right-click{}; {cleanup}", menu.partial);
                attempts.push(attempt);
                if !known_closed {
                    return Err(failed(&attempts));
                }
            }
            Err(error) => {
                let (_, cleanup) = cleanup(repository, process_id);
                attempt.outcome = format!("{error}; {cleanup}");
                attempts.push(attempt);
                return Err(failed(&attempts));
            }
        }
    }
    Err(format!(
        "StickyMD tray menu did not open after {TRAY_MENU_OPEN_ATTEMPTS} attempts: {}",
        history(&attempts)
    ))
}

/// Opens the menu, reads its items in order and closes it.
pub(in crate::qualification) fn menu_items(
    repository: &Path,
    process_id: u32,
) -> Result<Vec<String>, String> {
    let opened = open(repository, process_id)?;
    match close(repository, process_id) {
        Ok(()) => Ok(opened.items),
        Err(error) => Err(opened.failed(
            "the menu items were read but the menu was not closed",
            &format!("closing: {error}"),
        )),
    }
}

fn invoke(repository: &Path, process_id: u32, item: &str, label: &str) -> Result<(), String> {
    let opened = open(repository, process_id)?;
    if !opened.items.iter().any(|name| name == item) {
        let (_, cleanup) = cleanup(repository, process_id);
        return Err(opened.failed(
            &format!("{label} is absent from the opened StickyMD tray menu"),
            &cleanup,
        ));
    }
    helper(repository, "tray-invoke", process_id, Some(item))
        .and_then(|output| invoked(&output, item))
        .map_err(|error| {
            // Leave no menu open behind a failed invocation.
            let (_, cleanup) = cleanup(repository, process_id);
            opened.failed(&format!("{label}: {error}"), &cleanup)
        })
}

pub(in crate::qualification) fn show(repository: &Path, process_id: u32) -> Result<(), String> {
    invoke(repository, process_id, SHOW, "the tray Show item")
}

pub(in crate::qualification) fn hide(repository: &Path, process_id: u32) -> Result<(), String> {
    invoke(repository, process_id, HIDE, "the tray Hide item")
}

pub(in crate::qualification) fn toggle_topmost(
    repository: &Path,
    process_id: u32,
) -> Result<(), String> {
    invoke(repository, process_id, TOPMOST, "the tray Topmost item")
}

fn checked_state(output: &str, item: &str) -> Result<bool, String> {
    let lines = report(output, "tray-item-checked", &["UIA_TRAY_CHECKED"])?;
    let value = one(&lines, "UIA_TRAY_CHECKED").filter(|_| lines.len() == 1);
    match value.and_then(|value| value.strip_prefix(item)) {
        Some(";value=true") => Ok(true),
        Some(";value=false") => Ok(false),
        _ => Err(format!("malformed tray-item-checked report: {output:?}")),
    }
}

pub(in crate::qualification) fn topmost_checked(
    repository: &Path,
    process_id: u32,
) -> Result<bool, String> {
    let opened = open(repository, process_id)?;
    let observed = helper(repository, "tray-item-checked", process_id, Some(TOPMOST))
        .and_then(|output| checked_state(&output, TOPMOST));
    let closing = close(repository, process_id);
    match (observed, closing) {
        (Ok(value), Ok(())) => Ok(value),
        (observed, closing) => Err(opened.failed(
            &format!("topmost check state: {observed:?}"),
            &format!("closing: {closing:?}"),
        )),
    }
}

pub(in crate::qualification) fn exit(repository: &Path, process_id: u32) -> Result<(), String> {
    invoke(repository, process_id, EXIT, "the tray Exit item")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn check_state_requires_the_named_item_and_a_boolean() {
        assert_eq!(
            checked_state("UIA_TRAY_CHECKED=7F6E-9876;value=true", TOPMOST),
            Ok(true)
        );
        assert_eq!(
            checked_state("UIA_TRAY_CHECKED=7F6E-9876;value=false", TOPMOST),
            Ok(false)
        );
        for report in [
            "",
            "UIA_TRAY_CHECKED=7F6E-9876;value=0",
            "UIA_TRAY_CHECKED=9000-51FA;value=true",
            "UIA_TRAY_CHECKED=7F6E-9876;value=true\nUIA_TRAY_CHECKED=7F6E-9876;value=false",
        ] {
            assert!(checked_state(report, TOPMOST).is_err(), "{report}");
        }
    }

    #[test]
    fn successful_open_preserves_partial_items_in_later_failures() {
        let menu = items("UIA_TRAY_ITEM=663E-793A@g\nUIA_TRAY_MENU_PARTIAL; partial_items=earlier")
            .unwrap();
        let opened = Opened::observed(menu, Attempt::default(), Vec::new());
        let error = opened.failed("invoke failed", "closed");
        assert!(error.contains("opened; partial_items=earlier"), "{error}");
        assert!(error.contains("663E-793A"), "{error}");
    }

    #[test]
    fn the_target_report_is_read_strictly() {
        assert_eq!(
            located("UIA_TRAY_MENU_ITEMS_BEFORE=0\nUIA_TRAY_TARGET=1830,1062\nUIA_TRAY_GEOMETRY=g"),
            Ok(Located::Closed {
                x: 1830,
                y: 1062,
                geometry: "g".to_owned()
            })
        );
        assert_eq!(
            located("UIA_TRAY_MENU_ITEMS_BEFORE=3\nUIA_TRAY_MENU_OPEN; observed_items=a,b,c"),
            Ok(Located::Open(
                "UIA_TRAY_MENU_OPEN; observed_items=a,b,c".to_owned()
            ))
        );
        assert!(
            located("UIA_TRAY_MENU_UNOBSERVABLE")
                .unwrap_err()
                .contains("observed")
        );
        for malformed in [
            "",
            "noise",
            "UIA_TRAY_MENU_ITEMS_BEFORE=0",
            "UIA_TRAY_MENU_ITEMS_BEFORE=0\nUIA_TRAY_TARGET=1830,1062",
            "UIA_TRAY_MENU_ITEMS_BEFORE=0\nUIA_TRAY_TARGET=1830\nUIA_TRAY_GEOMETRY=g",
            "UIA_TRAY_MENU_ITEMS_BEFORE=0\nUIA_TRAY_TARGET=1,2,3\nUIA_TRAY_GEOMETRY=g",
            "UIA_TRAY_MENU_ITEMS_BEFORE=0\nUIA_TRAY_TARGET=a,b\nUIA_TRAY_GEOMETRY=g",
            "UIA_TRAY_MENU_ITEMS_BEFORE=0\nUIA_TRAY_TARGET=1,2\nUIA_TRAY_GEOMETRY=",
            "UIA_TRAY_MENU_ITEMS_BEFORE=0\nUIA_TRAY_TARGET=1,2\nUIA_TRAY_GEOMETRY=g\nnoise",
            // Repeated, contradicting or extra reports.
            "UIA_TRAY_MENU_ITEMS_BEFORE=0\nUIA_TRAY_MENU_ITEMS_BEFORE=0\nUIA_TRAY_TARGET=1,2\nUIA_TRAY_GEOMETRY=g",
            "UIA_TRAY_MENU_ITEMS_BEFORE=0\nUIA_TRAY_TARGET=1,2\nUIA_TRAY_TARGET=3,4\nUIA_TRAY_GEOMETRY=g",
            "UIA_TRAY_MENU_ITEMS_BEFORE=0\nUIA_TRAY_TARGET=1,2\nUIA_TRAY_GEOMETRY=g\nUIA_TRAY_MENU_OPEN",
            "UIA_TRAY_MENU_ITEMS_BEFORE=2\nUIA_TRAY_TARGET=1,2\nUIA_TRAY_GEOMETRY=g",
            "UIA_TRAY_MENU_ITEMS_BEFORE=2",
            "UIA_TRAY_MENU_ITEMS_BEFORE=2\nUIA_TRAY_MENU_OPEN\nUIA_TRAY_MENU_OPEN",
            "UIA_TRAY_MENU_ITEMS_BEFORE=x\nUIA_TRAY_TARGET=1,2\nUIA_TRAY_GEOMETRY=g",
            "UIA_TRAY_MENU_UNOBSERVABLE\nUIA_TRAY_MENU_ITEMS_BEFORE=0\nUIA_TRAY_TARGET=1,2\nUIA_TRAY_GEOMETRY=g",
            "UIA_TRAY_MENU_UNOBSERVABLE=x",
            "UIA_TRAY_MENU_ITEMS_BEFORE=+0\nUIA_TRAY_TARGET=1,2\nUIA_TRAY_GEOMETRY=g",
            "UIA_TRAY_MENU_ITEMS_BEFORE;0\nUIA_TRAY_TARGET=1,2\nUIA_TRAY_GEOMETRY=g",
            "UIA_TRAY_MENU_ITEMS_BEFORE=0\nUIA_TRAY_TARGET;1,2\nUIA_TRAY_GEOMETRY=g",
            "UIA_TRAY_MENU_ITEMS_BEFORE=2\nUIA_TRAY_MENU_OPEN",
            "UIA_TRAY_MENU_ITEMS_BEFORE=2\nUIA_TRAY_MENU_OPEN=x",
            "UIA_TRAY_MENU_OPEN; observed_items=a\nUIA_TRAY_MENU_ITEMS_BEFORE=2",
        ] {
            assert!(located(malformed).is_err(), "{malformed:?}");
        }
    }

    /// The helper's menu wait keeps the items an earlier unobserved poll read, through later polls
    /// that read none, and reports them only as diagnostics. The helper's own functions are
    /// loaded from its source; the UIA enumeration is replaced by a scripted sequence.
    #[cfg(windows)]
    #[test]
    fn the_helper_keeps_items_an_earlier_unobserved_poll_read() {
        const SCRIPT: &str = r#"
$ErrorActionPreference = 'Stop'
$ast = [System.Management.Automation.Language.Parser]::ParseFile($env:SMOKE_UIA_HELPER, [ref]$null, [ref]$null)
$wanted = 'Convert-NameToHex', 'Format-MenuItems', 'Write-WithPartial', 'Wait-ForProcessMenuItems', 'Write-TrayMenuItems'
foreach ($function in $ast.FindAll({ param($node) $node -is [System.Management.Automation.Language.FunctionDefinitionAst] }, $true)) {
    if ($wanted -contains $function.Name) { . ([scriptblock]::Create($function.Extent.Text)) }
}
# Wide enough for PowerShell's first-call compilation while other tests run.
$TrayMenuOpenTimeoutMilliseconds = 1000
$Ending = $env:SMOKE_ENDING
function Find-ProcessMenuItems {
    $global:polls++
    if ($Ending -eq 'warm') {
        return [PSCustomObject]@{ Items = @([PSCustomObject]@{ Name = 'W'; Geometry = 'W' }); Observed = $true }
    }
    if ($global:polls -eq 1) {
        return [PSCustomObject]@{ Items = @([PSCustomObject]@{ Name = 'AB'; Geometry = 'G' }); Observed = $false }
    }
    if ($Ending -eq 'opens') {
        return [PSCustomObject]@{ Items = @([PSCustomObject]@{ Name = 'CD'; Geometry = 'G2' }); Observed = $true }
    }
    return [PSCustomObject]@{ Items = @(); Observed = $Ending -eq 'none' }
}
# A first run compiles the functions, so the measured run's polls do not race that compilation.
$real = $Ending; $Ending = 'warm'; $global:polls = 0
Write-TrayMenuItems | Out-Null
$Ending = $real; $global:polls = 0
Write-TrayMenuItems
if ($global:polls -lt 2) { throw "only $global:polls poll(s)" }
"#;
        let helper = Path::new(env!("CARGO_MANIFEST_DIR")).join("helpers/windows-uia.ps1");
        let partial = "; partial_items=0041-0042@G";
        for ending in ["unobservable", "none", "opens"] {
            let mut command = std::process::Command::new("powershell");
            command
                .args(["-NoProfile", "-NonInteractive", "-Command", SCRIPT])
                .env("SMOKE_UIA_HELPER", &helper)
                .env("SMOKE_ENDING", ending);
            let output = crate::managed_process::run_bounded_output(
                &mut command,
                std::time::Duration::from_secs(60),
                "the UIA helper probe",
                1 << 20,
            )
            .unwrap();
            assert!(
                output.status.success(),
                "{ending}: {}",
                String::from_utf8_lossy(&output.stderr)
            );
            let report = String::from_utf8(output.stdout).unwrap();
            let report = report.trim().replace("\r\n", "\n");
            match ending {
                "unobservable" => {
                    assert_eq!(report, format!("UIA_TRAY_MENU_UNOBSERVABLE{partial}"));
                    assert!(items(&report).unwrap_err().ends_with(partial));
                }
                "none" => {
                    assert_eq!(report, format!("UIA_TRAY_MENU_NONE{partial}"));
                    let menu = items(&report).unwrap();
                    assert!(menu.items.is_empty());
                    assert_eq!(menu.partial, partial);
                }
                _ => {
                    assert_eq!(
                        report,
                        format!("UIA_TRAY_ITEM=0043-0044@G2\nUIA_TRAY_MENU_PARTIAL{partial}")
                    );
                    let menu = items(&report).unwrap();
                    assert_eq!(menu.items, ["0043-0044"]);
                    assert_eq!(menu.partial, partial);
                }
            }
        }
    }

    #[test]
    fn the_other_reports_are_read_strictly() {
        assert_eq!(
            items("UIA_TRAY_ITEM=9690-85CF@x=1;y=2\nUIA_TRAY_ITEM=9000-51FA@x=1;y=30")
                .unwrap()
                .items,
            ["9690-85CF", "9000-51FA"]
        );
        assert!(items("UIA_TRAY_MENU_NONE").unwrap().items.is_empty());
        let none = items("UIA_TRAY_MENU_NONE; partial_items=9690-85CF@g").unwrap();
        assert!(none.items.is_empty());
        assert_eq!(none.partial, "; partial_items=9690-85CF@g");
        let opened =
            items("UIA_TRAY_ITEM=9000-51FA@g\nUIA_TRAY_MENU_PARTIAL; partial_items=x").unwrap();
        assert_eq!(opened.items, ["9000-51FA"]);
        assert_eq!(opened.partial, "; partial_items=x");
        assert!(items("UIA_TRAY_MENU_UNOBSERVABLE").is_err());
        // Items read before the provider failed are reported, never taken as a menu.
        for error in [
            items("UIA_TRAY_MENU_UNOBSERVABLE; partial_items=9690-85CF@g").unwrap_err(),
            located("UIA_TRAY_MENU_UNOBSERVABLE; partial_items=9690-85CF@g").unwrap_err(),
        ] {
            assert!(
                error.contains("could not be observed")
                    && error.ends_with("; partial_items=9690-85CF@g"),
                "{error}"
            );
        }
        for malformed in [
            "",
            "UIA_TRAY_ITEM=",
            "UIA_TRAY_ITEM=9690-85CF",
            "UIA_TRAY_ITEM=9690-85CF@",
            "UIA_TRAY_ITEM=@g",
            "UIA_TRAY_ITEM=9690-85cf@g",
            "UIA_TRAY_ITEM=969@g",
            "UIA_TRAY_ITEM=9690-85CF@g\nUIA_TRAY_ITEM=9690-85CF@h",
            "UIA_TRAY_MENU_NONE\nUIA_TRAY_MENU_NONE",
            "UIA_TRAY_MENU_NONE\nUIA_TRAY_ITEM=9690-85CF@g",
            "UIA_TRAY_MENU_UNOBSERVABLE\nUIA_TRAY_ITEM=9690-85CF@g",
            "UIA_TRAY_MENU_UNOBSERVABLE\nUIA_TRAY_MENU_NONE",
            "UIA_TRAY_ITEM=9690-85CF@g\nnoise",
            "UIA_TRAY_ITEM;9690-85CF@g",
            "UIA_TRAY_MENU_NONE=x",
            "UIA_TRAY_MENU_UNOBSERVABLE;x",
            "UIA_TRAY_MENU_NONE;x",
            "UIA_TRAY_MENU_NONE; partial_items=a\nUIA_TRAY_ITEM=9690-85CF@g",
            // Partial items only ever follow an open menu's items, once, at the end.
            "UIA_TRAY_MENU_PARTIAL; partial_items=a",
            "UIA_TRAY_MENU_PARTIAL; partial_items=a\nUIA_TRAY_ITEM=9690-85CF@g",
            "UIA_TRAY_ITEM=9690-85CF@g\nUIA_TRAY_MENU_PARTIAL",
            "UIA_TRAY_ITEM=9690-85CF@g\nUIA_TRAY_MENU_PARTIAL=a",
            "UIA_TRAY_ITEM=9690-85CF@g\nUIA_TRAY_MENU_PARTIAL; partial_items=a\nUIA_TRAY_MENU_PARTIAL; partial_items=b",
            "UIA_TRAY_MENU_NONE\nUIA_TRAY_MENU_PARTIAL; partial_items=a",
            "UIA_TRAY_MENU_UNOBSERVABLE\nUIA_TRAY_MENU_PARTIAL; partial_items=a",
            "UIA_TRAY_ITEM=9690--85CF@g",
        ] {
            assert!(items(malformed).is_err(), "{malformed:?}");
        }
        assert!(closed("UIA_TRAY_MENU_CLOSED").is_ok());
        for malformed in [
            "",
            "UIA_TRAY_MENU_CLOSED\nUIA_TRAY_MENU_CLOSED",
            "UIA_TRAY_MENU_CLOSED\nUIA_TRAY_MENU_UNOBSERVABLE",
            "UIA_TRAY_MENU_CLOSED=x",
            "UIA_TRAY_MENU_CLOSED;x",
            "not UIA_TRAY_MENU_CLOSED",
        ] {
            assert!(closed(malformed).is_err(), "{malformed:?}");
        }
        assert!(invoked("UIA_TRAY_INVOKED=9000-51FA", EXIT).is_ok());
        for malformed in [
            "",
            "UIA_TRAY_INVOKED=663E-793A",
            "UIA_TRAY_INVOKED=9000-51FA\nUIA_TRAY_INVOKED=9000-51FA",
            "UIA_TRAY_INVOKED=9000-51FA\nnoise",
            "UIA_TRAY_INVOKED;9000-51FA",
            "UIA_TRAY_INVOKED=9000-51FA;x",
        ] {
            assert!(invoked(malformed, EXIT).is_err(), "{malformed:?}");
        }
    }
}

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

mod multiple;

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

/// What a tray step did to the overflow flyout, as far as it knows.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
enum Overflow {
    /// No enumeration reported opening it.
    #[default]
    Untouched,
    /// An enumeration reported opening it (`UIA_TRAY_OVERFLOW_OPENED`).
    Opened,
    /// An enumeration did not complete (failed, or was ended at its time limit), so whether it
    /// opened the flyout is unknown; why.
    Unknown(String),
}

impl Overflow {
    /// A known opening is never downgraded; an unknown outcome outranks "untouched".
    fn note(&mut self, next: Overflow) {
        match (&*self, &next) {
            (Overflow::Opened, _) | (_, Overflow::Untouched) => {}
            (Overflow::Unknown(_), Overflow::Unknown(_)) => {}
            _ => *self = next,
        }
    }
}

/// What opening the menu changed on the desktop besides the menu, to put back if the step
/// fails: where the cursor was, and what the step did to the overflow flyout.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct Restore {
    cursor: (i32, i32),
    overflow: Overflow,
}

/// What a failed step does about the overflow flyout: close it only when the step is known to
/// have opened it and its menu is observed closed (nothing is toggled under an open menu, or on
/// a guess); otherwise just say why not.
fn overflow_plan(overflow: &Overflow, menu_closed: &Result<(), String>) -> Result<(), String> {
    match (overflow, menu_closed) {
        (Overflow::Untouched, _) => Err("this step opened no overflow flyout".to_owned()),
        (Overflow::Unknown(why), _) => Err(format!(
            "whether this step opened the overflow flyout is unknown ({why}), so it is not toggled"
        )),
        (Overflow::Opened, Err(why)) => Err(format!(
            "the overflow flyout this step opened is left open: its menu was not observed closed ({why})"
        )),
        (Overflow::Opened, Ok(())) => Ok(()),
    }
}

/// `cleanup`'s verdict as a menu-closed observation for `restore`.
fn observed_closed(closed: bool) -> Result<(), String> {
    if closed {
        Ok(())
    } else {
        Err("the cleanup above did not observe it closed".to_owned())
    }
}

/// Puts back what a failed tray step changed, after its menu cleanup: closes an overflow flyout
/// as `overflow_plan` allows, then puts the cursor back through the injection module. A success
/// path keeps both, as before. The outcome, for the step's error; the step has failed either way.
fn restore(
    repository: &Path,
    process_id: u32,
    restore: &Restore,
    menu_closed: &Result<(), String>,
) -> String {
    let overflow = match overflow_plan(&restore.overflow, menu_closed) {
        Err(outcome) => outcome,
        Ok(()) => match helper(repository, "tray-overflow-close", process_id, None) {
            Ok(output) if output.trim() == "UIA_TRAY_OVERFLOW_CLOSED" => {
                "the overflow flyout this step opened is closed".to_owned()
            }
            Ok(output) => format!("malformed tray-overflow-close report: {output:?}"),
            Err(error) => {
                format!("the overflow flyout this step opened was not closed: {error}")
            }
        },
    };
    let (x, y) = restore.cursor;
    let cursor = match crate::window_control::restore_tray_cursor(x, y) {
        Ok(()) => format!("the cursor is back at ({x},{y})"),
        Err(error) => format!("the cursor was not put back: {error}"),
    };
    format!("restore: {overflow}; {cursor}")
}

/// An opened menu: its items, how it was opened, and what to put back if a later step fails.
struct Opened {
    items: Vec<String>,
    attempts: Vec<Attempt>,
    restore: Restore,
}

impl Opened {
    fn observed(
        menu: MenuRead,
        mut attempt: Attempt,
        mut attempts: Vec<Attempt>,
        restore: Restore,
    ) -> Self {
        attempt.outcome = format!("opened{}", menu.partial);
        attempts.push(attempt);
        Self {
            items: menu.items,
            attempts,
            restore,
        }
    }

    /// `failed`, after putting back what opening the menu changed.
    fn failed_restoring(
        &self,
        repository: &Path,
        process_id: u32,
        step: &str,
        cleanup: &str,
        menu_closed: Result<(), String>,
    ) -> String {
        let restored = restore(repository, process_id, &self.restore, &menu_closed);
        self.failed(step, &format!("{cleanup}; {restored}"))
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
    multiple::open(
        repository,
        process_id,
        &[process_id],
        multiple::Foreign::Excluded,
    )
}

/// Opens the menu, reads its items in order and closes it.
pub(in crate::qualification) fn menu_items(
    repository: &Path,
    process_id: u32,
) -> Result<Vec<String>, String> {
    let opened = open(repository, process_id)?;
    match close(repository, process_id) {
        Ok(()) => Ok(opened.items),
        Err(error) => Err(opened.failed_restoring(
            repository,
            process_id,
            "the menu items were read but the menu was not closed",
            &format!("closing: {error}"),
            Err(error),
        )),
    }
}

fn invoke(repository: &Path, process_id: u32, item: &str, label: &str) -> Result<(), String> {
    let opened = open(repository, process_id)?;
    invoke_opened(repository, process_id, item, label, opened)
}

fn invoke_opened(
    repository: &Path,
    process_id: u32,
    item: &str,
    label: &str,
    opened: Opened,
) -> Result<(), String> {
    if !opened.items.iter().any(|name| name == item) {
        let (closed, cleanup) = cleanup(repository, process_id);
        return Err(opened.failed_restoring(
            repository,
            process_id,
            &format!("{label} is absent from the opened StickyMD tray menu"),
            &cleanup,
            observed_closed(closed),
        ));
    }
    helper(repository, "tray-invoke", process_id, Some(item))
        .and_then(|output| invoked(&output, item))
        .map_err(|error| {
            // Leave no menu open behind a failed invocation.
            let (closed, cleanup) = cleanup(repository, process_id);
            opened.failed_restoring(
                repository,
                process_id,
                &format!("{label}: {error}"),
                &cleanup,
                observed_closed(closed),
            )
        })
}

pub(crate) fn exit_owned_copy(
    repository: &Path,
    process_id: u32,
    known: &[u32],
) -> Result<(), String> {
    // The only caller is the WindowShell runtime, which has no exclusive-desktop preflight.
    let opened = multiple::open(
        repository,
        process_id,
        known,
        multiple::Foreign::CheckBeforeEachClick,
    )?;
    invoke_opened(
        repository,
        process_id,
        EXIT,
        "the owned copy's tray Exit item",
        opened,
    )
}

pub(in crate::qualification) fn show(repository: &Path, process_id: u32) -> Result<(), String> {
    invoke(repository, process_id, SHOW, "the tray Show item")
}

pub(in crate::qualification) fn set_owned_visibility(
    repository: &Path,
    process_id: u32,
    known: &[u32],
    visible: bool,
) -> Result<(), String> {
    let opened = multiple::open(repository, process_id, known, multiple::Foreign::Excluded)?;
    let item = if visible { SHOW } else { HIDE };
    invoke_opened(
        repository,
        process_id,
        item,
        "owned copy visibility",
        opened,
    )
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

pub(in crate::qualification) fn topmost_checked(
    repository: &Path,
    process_id: u32,
) -> Result<bool, String> {
    let opened = open(repository, process_id)?;
    let observed = crate::window_control::tray_item_checked(process_id, "置顶");
    let closing = close(repository, process_id);
    match (observed, closing) {
        (Ok(value), Ok(())) => Ok(value),
        (observed, closing) => Err(opened.failed_restoring(
            repository,
            process_id,
            &format!("topmost check state: {observed:?}"),
            &format!("closing: {closing:?}"),
            closing.clone(),
        )),
    }
}

/// Opens the menu and asks for an item no StickyMD menu has, so the step fails after opening:
/// the local preflight's way to reach a failure's cleanup and restore on a real desktop.
#[cfg(test)]
pub(in crate::qualification) fn invoke_absent_item(
    repository: &Path,
    process_id: u32,
) -> Result<(), String> {
    invoke(
        repository,
        process_id,
        "0021",
        "an item no StickyMD menu has",
    )
}

pub(in crate::qualification) fn exit(repository: &Path, process_id: u32) -> Result<(), String> {
    invoke(repository, process_id, EXIT, "the tray Exit item")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A known opening wins, an unknown outcome outranks "untouched", and the flyout is toggled
    /// only when the step is known to have opened it and its menu is observed closed.
    #[test]
    fn the_overflow_is_closed_only_when_known_opened_and_the_menu_observed_closed() {
        let unknown = || Overflow::Unknown("helper ended".to_owned());
        let noted = |steps: &[Overflow]| {
            let mut overflow = Overflow::Untouched;
            for step in steps {
                overflow.note(step.clone());
            }
            overflow
        };
        assert_eq!(noted(&[]), Overflow::Untouched);
        assert_eq!(noted(&[unknown()]), unknown());
        assert_eq!(noted(&[unknown(), Overflow::Opened]), Overflow::Opened);
        assert_eq!(noted(&[Overflow::Opened, unknown()]), Overflow::Opened);
        assert_eq!(
            noted(&[Overflow::Opened, Overflow::Untouched]),
            Overflow::Opened
        );
        let closed = Ok(());
        let open = Err("PID 2's menu was not observed closed".to_owned());
        assert_eq!(overflow_plan(&Overflow::Opened, &closed), Ok(()));
        assert!(
            overflow_plan(&Overflow::Opened, &open)
                .unwrap_err()
                .contains("PID 2")
        );
        assert!(
            overflow_plan(&unknown(), &closed)
                .unwrap_err()
                .contains("not toggled")
        );
        assert!(overflow_plan(&Overflow::Untouched, &closed).is_err());
    }

    #[test]
    fn successful_open_preserves_partial_items_in_later_failures() {
        let menu = items("UIA_TRAY_ITEM=663E-793A@g\nUIA_TRAY_MENU_PARTIAL; partial_items=earlier")
            .unwrap();
        let opened = Opened::observed(menu, Attempt::default(), Vec::new(), Restore::default());
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

    /// The helper opens the flyout on one observed state: a visible flyout read after an
    /// unobservable read is "already open" and untouched, a hidden one is toggled once and
    /// "opened", and one that stays unobservable toggles nothing and is an error. The helper's
    /// function is loaded from its source; the UIA reads and the toggle are scripted.
    #[cfg(windows)]
    #[test]
    fn the_helper_opens_the_overflow_only_on_an_observed_hidden_state() {
        const SCRIPT: &str = r#"
$ErrorActionPreference = 'Stop'
$ast = [System.Management.Automation.Language.Parser]::ParseFile($env:SMOKE_UIA_HELPER, [ref]$null, [ref]$null)
foreach ($function in $ast.FindAll({ param($node) $node -is [System.Management.Automation.Language.FunctionDefinitionAst] }, $true)) {
    if ($function.Name -eq 'Open-TrayOverflowObserved') { . ([scriptblock]::Create($function.Extent.Text)) }
}
$global:reads = @($env:SMOKE_STATES -split ',')
$global:read = 0
$global:toggles = 0
function Get-TrayOverflowState {
    $state = $global:reads[[Math]::Min($global:read, $global:reads.Count - 1)]
    $global:read++
    return $state
}
function Invoke-TrayOverflowToggle { $global:toggles++ }
try { $result = Open-TrayOverflowObserved } catch { $result = 'error' }
Write-Output ('{0} toggles={1}' -f $result, $global:toggles)
"#;
        let helper = Path::new(env!("CARGO_MANIFEST_DIR")).join("helpers/windows-uia.ps1");
        for (states, expected) in [
            ("unobservable,visible", "already-open toggles=0"),
            ("visible", "already-open toggles=0"),
            ("unobservable,hidden", "opened toggles=1"),
            ("hidden", "opened toggles=1"),
            ("unobservable", "error toggles=0"),
        ] {
            let mut command = std::process::Command::new("powershell");
            command
                .args(["-NoProfile", "-NonInteractive", "-Command", SCRIPT])
                .env("SMOKE_UIA_HELPER", &helper)
                .env("SMOKE_STATES", states);
            let output = crate::managed_process::run_bounded_output(
                &mut command,
                std::time::Duration::from_secs(60),
                "the UIA helper overflow probe",
                1 << 20,
            )
            .unwrap();
            assert!(
                output.status.success(),
                "{states}: {}",
                String::from_utf8_lossy(&output.stderr)
            );
            assert_eq!(
                String::from_utf8(output.stdout).unwrap().trim(),
                expected,
                "{states}"
            );
        }
    }

    /// Closing retries an unobservable read and toggles once on the first visible one, never on an
    /// unobservable one; opening-then-waiting closes a flyout it may have opened on any failure
    /// after the toggle was attempted, including a toggle that took effect and then threw, and
    /// toggles nothing when the flyout was never observed. The helper's functions are loaded from
    /// its source; the UIA reads and the toggle are scripted.
    #[cfg(windows)]
    #[test]
    fn the_helper_closes_and_compensates_the_overflow_on_observed_states() {
        const SCRIPT: &str = r#"
$ErrorActionPreference = 'Stop'
$ast = [System.Management.Automation.Language.Parser]::ParseFile($env:SMOKE_UIA_HELPER, [ref]$null, [ref]$null)
$wanted = 'Wait-Until', 'Open-TrayOverflowObserved', 'Close-TrayOverflowObserved', 'Close-OpenedTrayOverflow', 'Open-TrayOverflowUntil'
foreach ($function in $ast.FindAll({ param($node) $node -is [System.Management.Automation.Language.FunctionDefinitionAst] }, $true)) {
    if ($wanted -contains $function.Name) { . ([scriptblock]::Create($function.Extent.Text)) }
}
$TimeoutSeconds = 1
$global:reads = @($env:SMOKE_STATES -split ',')
$global:read = 0
$global:toggles = 0
function Get-TrayOverflowState {
    $state = $global:reads[[Math]::Min($global:read, $global:reads.Count - 1)]
    $global:read++
    return $state
}
function Invoke-TrayOverflowToggle {
    $global:toggles++
    if ($env:SMOKE_TOGGLE -eq 'throw-first' -and $global:toggles -eq 1) { throw 'toggle threw after taking effect' }
}
try {
    if ($env:SMOKE_ACTION -eq 'close') {
        Close-TrayOverflowObserved
        $result = 'closed'
    } else {
        $outcome = Open-TrayOverflowUntil { if ($env:SMOKE_READY -eq 'yes') { return 'ready' } else { return $null } } 'candidates'
        $result = 'opened=' + $outcome.Opened
    }
} catch { $result = "error: $_" }
Write-Output ('{0} | toggles={1}' -f $result, $global:toggles)
"#;
        let helper = Path::new(env!("CARGO_MANIFEST_DIR")).join("helpers/windows-uia.ps1");
        // (action, reads, toggle, ready, expected fragments)
        let cases: &[(&str, &str, &str, &str, &[&str])] = &[
            (
                "close",
                "unobservable,visible,hidden",
                "count",
                "",
                &["closed |", "toggles=1"],
            ),
            (
                "close",
                "visible,visible,hidden",
                "count",
                "",
                &["closed |", "toggles=1"],
            ),
            (
                "close",
                "unobservable",
                "count",
                "",
                &["not observed closed", "toggles=0"],
            ),
            (
                "until",
                "hidden",
                "count",
                "yes",
                &["opened=True |", "toggles=1"],
            ),
            (
                "until",
                "visible",
                "count",
                "yes",
                &["opened=False |", "toggles=0"],
            ),
            (
                "until",
                "hidden,visible,hidden",
                "throw-first",
                "yes",
                &["toggle threw", "opened is closed", "toggles=2"],
            ),
            (
                "until",
                "hidden,visible,hidden",
                "count",
                "no",
                &[
                    "Timed out waiting for candidates",
                    "opened is closed",
                    "toggles=2",
                ],
            ),
            (
                "until",
                "unobservable",
                "count",
                "yes",
                &["could not be observed", "toggles=0"],
            ),
        ];
        for (action, states, toggle, ready, expected) in cases {
            let mut command = std::process::Command::new("powershell");
            command
                .args(["-NoProfile", "-NonInteractive", "-Command", SCRIPT])
                .env("SMOKE_UIA_HELPER", &helper)
                .env("SMOKE_ACTION", action)
                .env("SMOKE_STATES", states)
                .env("SMOKE_TOGGLE", toggle)
                .env("SMOKE_READY", ready);
            let output = crate::managed_process::run_bounded_output(
                &mut command,
                std::time::Duration::from_secs(60),
                "the UIA helper overflow cleanup probe",
                1 << 20,
            )
            .unwrap();
            let stdout = String::from_utf8(output.stdout).unwrap();
            assert!(
                output.status.success(),
                "{action} {states}: {}",
                String::from_utf8_lossy(&output.stderr)
            );
            for fragment in *expected {
                assert!(
                    stdout.contains(fragment),
                    "{action} {states} {toggle}: {stdout}"
                );
            }
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

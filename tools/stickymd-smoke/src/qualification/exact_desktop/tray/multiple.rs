//! Same-name tray icons are candidates; only a native popup owner identifies one.
//!
//! plan_ref: docs/plan/11_testing_and_release.md#physical-input-injection

use std::cell::RefCell;
use std::path::Path;
use std::time::{Duration, Instant};

use super::{Attempt, Located, Opened, Overflow, Restore, TRAY_MENU_OPEN_ATTEMPTS};
use crate::managed_process::Product;

#[derive(Debug, PartialEq, Eq)]
struct Icon {
    x: i32,
    y: i32,
    geometry: String,
}

enum Targets {
    Icons(Vec<Icon>),
    Open(String),
}

/// Whether the helper says this enumeration opened the overflow flyout: its
/// `UIA_TRAY_OVERFLOW_OPENED`, right after the closed-menu line. Read apart from the candidates,
/// so a report whose candidates are then rejected still carries the desktop change to undo.
fn opened_overflow(output: &str) -> bool {
    let mut lines = output.lines().map(str::trim).filter(|s| !s.is_empty());
    lines.next() == Some("UIA_TRAY_MENU_ITEMS_BEFORE=0")
        && lines.next() == Some("UIA_TRAY_OVERFLOW_OPENED")
}

/// The candidates, and whether this enumeration opened the overflow flyout.
fn parse(output: &str) -> Result<(Targets, bool), String> {
    let lines: Vec<_> = output
        .lines()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .collect();
    if lines.first() != Some(&"UIA_TRAY_MENU_ITEMS_BEFORE=0") {
        return match super::located(output)? {
            Located::Open(items) => Ok((Targets::Open(items), false)),
            _ => Err(format!("malformed tray-targets report: {output:?}")),
        };
    }
    let overflow_opened = opened_overflow(output);
    let pairs = &lines[if overflow_opened { 2 } else { 1 }..];
    if pairs.is_empty() || pairs.len() > 32 || !pairs.len().is_multiple_of(2) {
        return Err(format!("invalid tray candidate count: {output:?}"));
    }
    let mut icons = Vec::new();
    for pair in pairs.chunks_exact(2) {
        let one = format!("{}\n{}\n{}", lines[0], pair[0], pair[1]);
        let Located::Closed { x, y, geometry } = super::located(&one)? else {
            return Err(format!("malformed tray candidate: {one:?}"));
        };
        if icons.iter().any(|icon: &Icon| (icon.x, icon.y) == (x, y)) {
            return Err(format!("duplicate tray candidate at {x},{y}"));
        }
        icons.push(Icon { x, y, geometry });
    }
    Ok((Targets::Icons(icons), overflow_opened))
}

/// Enumerates the candidates, remembering in `search` whether any enumeration opened the
/// overflow flyout, so a failed step can close it again.
fn targets(search: &Search<'_>) -> Result<Targets, String> {
    let output = super::helper(
        search.repository,
        "tray-targets",
        search.target,
        Some(&search.known.len().to_string()),
    )
    .inspect_err(|_| {
        // A helper that failed or was ended at its time limit printed no marker; it may have
        // opened the flyout first, so the change is unknown rather than absent.
        search.overflow.borrow_mut().note(Overflow::Unknown(
            "the tray-targets helper did not complete".to_owned(),
        ));
    })?;
    // Before the candidates are validated, so a rejected report still records the opening.
    if opened_overflow(&output) {
        search.overflow.borrow_mut().note(Overflow::Opened);
    }
    parse(&output).map(|(targets, _)| targets)
}

/// Whether every owned copy's menu is observed closed, by the helper, for a failure where no
/// popup owner was active: the foreground thread's popup state says nothing about the other
/// copy's menu. The first copy that is not observed closed, and why.
fn owned_menus_closed(repository: &Path, known: &[u32]) -> Result<(), String> {
    for &pid in known {
        super::helper(repository, "tray-menu-closed", pid, None)
            .and_then(|output| super::closed(&output))
            .map_err(|error| format!("PID {pid}'s menu was not observed closed: {error}"))?;
    }
    Ok(())
}

fn owned(owner: u32, known: &[u32]) -> Result<(), String> {
    if known.contains(&owner) {
        Ok(())
    } else {
        Err(format!(
            "popup belongs to PID {owner}, outside owned test instances {known:?}; no further input sent"
        ))
    }
}

fn close_current(repository: &Path, known: &[u32]) -> Result<Option<u32>, String> {
    let owner = crate::window_control::active_tray_popup_owner()?;
    if let Some(owner) = owner {
        owned(owner, known)?;
        // Rechecks the native owner immediately before Esc, then observes closed.
        super::close(repository, owner)?;
    }
    Ok(owner)
}

fn wait_owner() -> Result<Option<u32>, String> {
    let until = Instant::now() + Duration::from_millis(1500);
    loop {
        let owner = crate::window_control::active_tray_popup_owner()?;
        if owner.is_some() || Instant::now() >= until {
            return Ok(owner);
        }
        std::thread::sleep(Duration::from_millis(25));
    }
}

/// Whether the desktop may hold StickyMD processes outside `known` when an icon is clicked.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Foreign {
    /// The caller's preflight already made the desktop exclusive (G4).
    Excluded,
    /// No exclusive-desktop preflight ran (the WindowShell runtime): before the tray is touched and
    /// again before every click, `exclusivity` must hold. A copy started between the last check and
    /// the click is a residual race; its popup is then refused by `owned` before any item runs.
    CheckBeforeEachClick,
}

/// What a foreign tray owner is, from its image and from the running process.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Identity {
    /// The ProductName of the file at its image path.
    pub(super) product: Product,
    /// Whether the process has a StickyMD-titled window, or why that is unknown.
    pub(super) stickymd_window: Result<bool, String>,
}

impl Identity {
    /// Either piece of evidence suffices to refuse: the image's ProductName can be read from a
    /// file that replaced the running image, and a title is checked only in case it was.
    fn may_be_stickymd(&self) -> bool {
        self.product.may_be_stickymd() || self.stickymd_window != Ok(false)
    }
}

/// Why a tray click is refused.
#[derive(Debug, PartialEq, Eq)]
pub(super) enum Refusal {
    /// StickyMD processes, by image name, this run did not start.
    ForeignStickyMd(Vec<u32>),
    /// Processes this run did not start that own `tray-icon` owner windows and may be StickyMD
    /// (a renamed copy) or cannot be told apart.
    ForeignTrayOwners(Vec<(u32, Identity)>),
    /// The target owns no `tray-icon` owner window, so the enumeration proves nothing (a stale
    /// class name after a `tray-icon` upgrade would otherwise pass vacuously).
    TargetNotATrayOwner(u32),
}

/// Whether every StickyMD tray icon on the desktop is owned, proven per process rather than by
/// image name (a renamed copy) or icon count (an icon in the closed overflow, an owned copy
/// whose tray window was never created). Every process whose image is named StickyMD
/// (`image_named`) must be in `known`; so must every process with a `tray-icon` owner window
/// (`tray_owners`) whose `identity` may be StickyMD. A renamed copy keeps the ProductName its
/// build embeds and its StickyMD-titled window, so another program built on `tray-icon` is told
/// apart only when its image names another product or none and it has no such window. The
/// `target` about to be clicked must be among the owners.
///
/// Residuals, where the popup is refused by `owned` before any item is invoked: a program
/// outside `tray-icon` that names its icon StickyMD; an owner window created, or a copy started,
/// between the enumeration and the click. A renamed copy without its StickyMD window but with
/// a live tray window would be one too, but the product's exit drops the tray before the
/// window (`WindowEffect::ExitProcess` in `app/window_runtime.rs`), so it is not expected on
/// the current shutdown path; an abnormal lifecycle is unverified.
pub(super) fn exclusivity(
    image_named: &[u32],
    tray_owners: &[u32],
    known: &[u32],
    target: u32,
    identity: impl Fn(u32) -> Identity,
) -> Result<(), Refusal> {
    let foreign = crate::managed_process::foreign_stickymd(image_named, known);
    if !foreign.is_empty() {
        return Err(Refusal::ForeignStickyMd(foreign));
    }
    // Only foreign owners are read; owned copies need no identity.
    let unproven: Vec<_> = crate::managed_process::foreign_stickymd(tray_owners, known)
        .into_iter()
        .map(|pid| (pid, identity(pid)))
        .filter(|(_, identity)| identity.may_be_stickymd())
        .collect();
    if !unproven.is_empty() {
        return Err(Refusal::ForeignTrayOwners(unproven));
    }
    if !tray_owners.contains(&target) {
        return Err(Refusal::TargetNotATrayOwner(target));
    }
    Ok(())
}

fn owner_identity(pid: u32) -> Identity {
    Identity {
        product: crate::managed_process::process_product(pid),
        stickymd_window: crate::window_control::has_stickymd_window(pid),
    }
}

fn check_exclusive(known: &[u32], target: u32) -> Result<(), String> {
    let refusal = match exclusivity(
        // Another sign-in's StickyMD has no icon in this session's tray.
        &crate::managed_process::outside_other_sessions(
            crate::managed_process::running_stickymd_pids()?,
        ),
        &crate::window_control::tray_icon_owner_pids()?,
        known,
        target,
        owner_identity,
    ) {
        Ok(()) => return Ok(()),
        Err(refusal) => refusal,
    };
    let reason = match refusal {
        Refusal::ForeignStickyMd(pids) => {
            let images: Vec<_> = pids
                .iter()
                .map(
                    |&pid| match crate::managed_process::process_executable(pid) {
                        Some(path) => format!("{pid} ({})", path.display()),
                        None => format!("{pid} (image unreadable)"),
                    },
                )
                .collect();
            format!(
                "StickyMD processes not started by this run are running: {}; exit them and run again",
                images.join(", ")
            )
        }
        Refusal::ForeignTrayOwners(owners) => {
            let owners: Vec<_> = owners
                .iter()
                .map(|(pid, identity)| {
                    let product = match &identity.product {
                        Product::Named(name) => format!("ProductName {name:?}"),
                        Product::Unreadable(why) => format!("product unreadable: {why}"),
                        Product::Absent => "no version resource".to_owned(),
                    };
                    let window = match &identity.stickymd_window {
                        Ok(true) => "a StickyMD window".to_owned(),
                        Ok(false) => "no StickyMD window".to_owned(),
                        Err(why) => format!("windows unreadable: {why}"),
                    };
                    format!("{pid} ({product}; {window})")
                })
                .collect();
            format!(
                "tray-icon owners not started by this run may be StickyMD (a renamed copy, or one that cannot be told apart): {}; exit them and run again",
                owners.join(", ")
            )
        }
        Refusal::TargetNotATrayOwner(pid) => format!(
            "target {pid} owns no tray-icon window, so tray ownership is unproven (has tray-icon renamed its window class?)"
        ),
    };
    Err(format!("refusing a tray click: {reason}"))
}

pub(super) fn open(
    repository: &Path,
    target: u32,
    known: &[u32],
    foreign: Foreign,
) -> Result<Opened, String> {
    if !(1..=2).contains(&known.len())
        || !known.contains(&target)
        || known.contains(&0)
        || (known.len() == 2 && known[0] == known[1])
    {
        return Err(format!(
            "invalid owned tray instance set: {known:?}, target={target}"
        ));
    }
    let mut attempts = Vec::new();
    let mut clicks = 0;
    let click_limit = if known.len() == 1 {
        TRAY_MENU_OPEN_ATTEMPTS
    } else {
        32
    };
    // Read before anything moves it, to put it back if the step fails.
    let cursor = crate::window_control::tray_cursor_position()?;
    let search = Search {
        repository,
        target,
        known,
        foreign,
        overflow: RefCell::new(Overflow::Untouched),
    };
    let restore = || Restore {
        cursor,
        overflow: search.overflow.borrow().clone(),
    };
    let run = (|| {
        // At most two enumerations, each with at most sixteen same-name icons.
        // Every physical click still gets only one retry, and no menu is invoked
        // until both its native owner and PID-filtered UIA items agree.
        for _ in 0..TRAY_MENU_OPEN_ATTEMPTS {
            if foreign == Foreign::CheckBeforeEachClick {
                // Before enumeration can expand the overflow flyout or move the cursor.
                check_exclusive(known, target)?;
            }
            if let Some(owner) = close_current(repository, known)? {
                attempts.push(Attempt {
                    outcome: format!("pre-existing owned popup {owner} closed"),
                    ..Attempt::default()
                });
            }
            let icons = match targets(&search)? {
                Targets::Icons(icons) => icons,
                Targets::Open(items) => {
                    return Err(format!("menu remained open before enumeration: {items}"));
                }
            };
            for icon in &icons {
                let mut attempt = Attempt {
                    geometry: Some(icon.geometry.clone()),
                    ..Attempt::default()
                };
                let step = candidate(&search, icon, &icons, &mut attempt, &mut clicks);
                match step {
                    Ok(Candidate::Opened(menu)) => {
                        return Ok(Opened::observed(
                            menu,
                            attempt,
                            std::mem::take(&mut attempts),
                            restore(),
                        ));
                    }
                    Ok(Candidate::Next) => attempts.push(attempt),
                    Ok(Candidate::Reenumerate) => {
                        attempts.push(attempt);
                        break;
                    }
                    Err(error) => {
                        // Every exit keeps what this attempt observed (geometry, cursor
                        // readback, whether the click was sent) in the history.
                        attempt.outcome = if attempt.outcome.is_empty() {
                            error.clone()
                        } else {
                            format!("{}; then {error}", attempt.outcome)
                        };
                        attempts.push(attempt);
                        return Err(error);
                    }
                }
                if clicks >= click_limit {
                    return Err(format!("tray click limit {click_limit} reached"));
                }
            }
        }
        Err("bounded tray candidate search found no matching menu".to_owned())
    })();
    run.map_err(|error| {
        let cleanup = close_current(repository, known);
        // No active popup owner is not a menu observed closed: only a closed observation (after
        // an Esc, or from the helper for every owned copy now) lets the flyout be toggled.
        let menu_closed = match &cleanup {
            Ok(Some(_)) => Ok(()),
            Ok(None) => owned_menus_closed(repository, known),
            Err(error) => Err(error.clone()),
        };
        let restored = super::restore(repository, target, &restore(), &menu_closed);
        format!(
            "{error}; cleanup={cleanup:?}; menus closed={menu_closed:?}; {restored}; attempts: {}",
            super::history(&attempts)
        )
    })
}

/// What one candidate icon led to.
enum Candidate {
    /// The target's popup opened with observed items.
    Opened(super::MenuRead),
    /// Not the target's icon; try the next one.
    Next,
    /// The candidates changed after hover; enumerate again.
    Reenumerate,
}

/// What every candidate of one `open` shares.
struct Search<'a> {
    repository: &'a Path,
    target: u32,
    known: &'a [u32],
    foreign: Foreign,
    /// What this `open`'s enumerations did to the overflow flyout.
    overflow: RefCell<Overflow>,
}

/// Hovers, rechecks and right-clicks one candidate icon. On an error `attempt.outcome` holds
/// what happened before it (empty if nothing yet); the caller adds the error and records it.
fn candidate(
    search: &Search<'_>,
    icon: &Icon,
    icons: &[Icon],
    attempt: &mut Attempt,
    clicks: &mut usize,
) -> Result<Candidate, String> {
    let Search {
        repository,
        target,
        known,
        foreign,
        ..
    } = *search;
    crate::window_control::place_cursor_on_tray_icon(icon.x, icon.y, &mut attempt.cursor)?;
    // Hover can remove a dead notification icon and reflow the flyout.
    // Re-read after hover; never click coordinates from the old layout.
    match targets(search)? {
        Targets::Icons(refreshed) if refreshed == icons => {}
        _ => {
            attempt.outcome = "tray candidates changed after hover; no click sent".to_owned();
            return Ok(Candidate::Reenumerate);
        }
    }
    if let Some(owner) = crate::window_control::active_tray_popup_owner()? {
        owned(owner, known)?;
        return Err(format!("popup {owner} appeared before candidate click"));
    }
    if foreign == Foreign::CheckBeforeEachClick {
        check_exclusive(known, target).inspect_err(|_| {
            attempt.outcome = "no click sent".to_owned();
        })?;
    }
    crate::window_control::right_click_tray_icon(icon.x, icon.y)?;
    *clicks += 1;
    attempt.outcome = "right-click sent".to_owned();
    let Some(owner) = wait_owner()? else {
        attempt.outcome = "no native popup owner observed after click".to_owned();
        super::closed(&super::helper(
            repository,
            "tray-menu-closed",
            target,
            None,
        )?)?;
        return Ok(Candidate::Next);
    };
    owned(owner, known)?;
    if owner == target {
        let menu = super::items(&super::helper(repository, "tray-menu-items", target, None)?)?;
        if !menu.items.is_empty() {
            return Ok(Candidate::Opened(menu));
        }
        attempt.outcome = format!("target popup {owner} had no observed items{}", menu.partial);
    } else {
        attempt.outcome = format!("candidate belongs to other owned instance {owner}");
    }
    close_current(repository, known)?;
    Ok(Candidate::Next)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn candidate_reports_reject_duplicates_partial_and_unbounded_lists() {
        let pair = "UIA_TRAY_TARGET=1,2\nUIA_TRAY_GEOMETRY=g";
        let second = "UIA_TRAY_TARGET=3,4\nUIA_TRAY_GEOMETRY=h";
        let prefix = "UIA_TRAY_MENU_ITEMS_BEFORE=0";
        assert!(
            matches!(parse(&format!("{prefix}\n{pair}\n{second}")).unwrap(), (Targets::Icons(icons), false) if icons.len() == 2)
        );
        // The opening survives a report whose candidates are rejected (a duplicate here).
        assert!(opened_overflow(&format!(
            "{prefix}
UIA_TRAY_OVERFLOW_OPENED
{pair}
{pair}"
        )));
        assert!(
            parse(&format!(
                "{prefix}
UIA_TRAY_OVERFLOW_OPENED
{pair}
{pair}"
            ))
            .is_err()
        );
        assert!(!opened_overflow(&format!(
            "{prefix}
{pair}"
        )));
        assert!(!opened_overflow("UIA_TRAY_OVERFLOW_OPENED"));
        // The helper says when this enumeration opened the overflow flyout.
        assert!(
            matches!(parse(&format!("{prefix}\nUIA_TRAY_OVERFLOW_OPENED\n{pair}")).unwrap(), (Targets::Icons(icons), true) if icons.len() == 1)
        );
        for bad in [
            prefix.to_owned(),
            format!("{prefix}\n{pair}\n{pair}"),
            format!("{prefix}\n{pair}\nnoise"),
            format!("{prefix}\n{}", vec![pair; 17].join("\n")),
            "UIA_TRAY_MENU_UNOBSERVABLE; partial_items=x".to_owned(),
        ] {
            assert!(parse(&bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn unrelated_popup_is_never_authorized_for_cleanup() {
        assert!(owned(11, &[11, 22]).is_ok());
        assert!(owned(22, &[11, 22]).is_ok());
        assert!(
            owned(33, &[11, 22])
                .unwrap_err()
                .contains("no further input sent")
        );
    }
}

#[cfg(test)]
mod exclusivity_tests {
    use super::{Identity, Refusal, exclusivity};
    use crate::managed_process::Product;

    /// 30 is a renamed StickyMD, 40 another program built on tray-icon, 50 one without a
    /// version resource, 60 one whose image cannot be read, 70 a renamed StickyMD whose image
    /// path now holds another program's file, 80 one whose windows cannot be read.
    fn identity(pid: u32) -> Identity {
        let (product, stickymd_window) = match pid {
            30 => (Product::Named("StickyMD".into()), Ok(true)),
            40 => (Product::Named("Clash Verge".into()), Ok(false)),
            50 => (Product::Absent, Ok(false)),
            60 => (Product::Unreadable("access denied".into()), Ok(false)),
            70 => (Product::Named("Clash Verge".into()), Ok(true)),
            80 => (Product::Absent, Err("window destroyed".into())),
            _ => panic!("an owned copy's identity was read: {pid}"),
        };
        Identity {
            product,
            stickymd_window,
        }
    }

    /// A click needs every possible StickyMD tray icon to be owned, by process, not by count.
    #[test]
    fn a_foreign_stickymd_or_tray_owner_refuses_the_click() {
        let known = [10, 20];
        let check = |image_named: &[u32], owners: &[u32]| {
            exclusivity(image_named, owners, &known, 20, identity)
        };
        let refused = |pid| Err(Refusal::ForeignTrayOwners(vec![(pid, identity(pid))]));
        assert_eq!(check(&[10, 20], &[10, 20]), Ok(()));
        // An owned copy whose tray window was never created owns no icon; still owned.
        assert_eq!(check(&[10, 20], &[20]), Ok(()));
        assert_eq!(
            check(&[10, 20, 30], &[10, 20]),
            Err(Refusal::ForeignStickyMd(vec![30]))
        );
        // The round-3 counterexample: owned 10 visible, owned 20 in the closed overflow, a
        // renamed copy 30 visible. Two visible icons match two owned copies, but 30 owns a tray
        // window and is StickyMD, so no click is sent.
        assert_eq!(check(&[10, 20], &[10, 20, 30]), refused(30));
        // An owned copy without a tray window cannot make room for a foreign one either.
        assert_eq!(check(&[10, 20], &[20, 30]), refused(30));
        // Other tray-icon programs are told apart by their own product, or by having none,
        // and by having no StickyMD window.
        assert_eq!(check(&[10, 20], &[10, 20, 40, 50]), Ok(()));
        // An unreadable product proves nothing.
        assert_eq!(check(&[10, 20], &[10, 20, 40, 60]), refused(60));
        // The round-5 counterexample: the file at a renamed copy's image path was replaced, so
        // its ProductName reads as another product; its own window still names it.
        assert_eq!(check(&[10, 20], &[10, 20, 70]), refused(70));
        // Windows that cannot be read prove nothing either.
        assert_eq!(check(&[10, 20], &[10, 20, 80]), refused(80));
        // The target must own a tray window, or the enumeration (a stale class name) proved
        // nothing.
        assert_eq!(
            check(&[10, 20], &[10]),
            Err(Refusal::TargetNotATrayOwner(20))
        );
        assert_eq!(check(&[10, 20], &[]), Err(Refusal::TargetNotATrayOwner(20)));
    }
}

//! Same-name tray icons are candidates; only a native popup owner identifies one.
//!
//! plan_ref: docs/plan/11_testing_and_release.md#physical-input-injection

use std::path::Path;
use std::time::{Duration, Instant};

use super::{Attempt, Located, Opened, TRAY_MENU_OPEN_ATTEMPTS};

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

fn parse(output: &str) -> Result<Targets, String> {
    let lines: Vec<_> = output
        .lines()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .collect();
    if lines.first() != Some(&"UIA_TRAY_MENU_ITEMS_BEFORE=0") {
        return match super::located(output)? {
            Located::Open(items) => Ok(Targets::Open(items)),
            _ => Err(format!("malformed tray-targets report: {output:?}")),
        };
    }
    let pairs = &lines[1..];
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
    Ok(Targets::Icons(icons))
}

fn targets(repository: &Path, target: u32, count: usize) -> Result<Targets, String> {
    parse(&super::helper(
        repository,
        "tray-targets",
        target,
        Some(&count.to_string()),
    )?)
}

fn owned(owner: u32, known: &[u32]) -> Result<(), String> {
    if known.contains(&owner) {
        Ok(())
    } else {
        Err(format!(
            "popup belongs to PID {owner}, outside owned test instances {known:?}; no input sent"
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

pub(super) fn open(repository: &Path, target: u32, known: &[u32]) -> Result<Opened, String> {
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
    let run = (|| {
        // At most two enumerations, each with at most sixteen same-name icons.
        // Every physical click still gets only one retry, and no menu is invoked
        // until both its native owner and PID-filtered UIA items agree.
        for _ in 0..TRAY_MENU_OPEN_ATTEMPTS {
            if let Some(owner) = close_current(repository, known)? {
                attempts.push(Attempt {
                    outcome: format!("pre-existing owned popup {owner} closed"),
                    ..Attempt::default()
                });
            }
            let icons = match targets(repository, target, known.len())? {
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
                crate::window_control::place_cursor_on_tray_icon(
                    icon.x,
                    icon.y,
                    &mut attempt.cursor,
                )?;
                // Hover can remove a dead notification icon and reflow the flyout.
                // Re-read after hover; never click coordinates from the old layout.
                match targets(repository, target, known.len())? {
                    Targets::Icons(refreshed) if refreshed == icons => {}
                    _ => {
                        attempt.outcome =
                            "tray candidates changed after hover; no click sent".to_owned();
                        attempts.push(attempt);
                        break;
                    }
                }
                if let Some(owner) = crate::window_control::active_tray_popup_owner()? {
                    owned(owner, known)?;
                    return Err(format!("popup {owner} appeared before candidate click"));
                }
                if let Err(error) = crate::window_control::right_click_tray_icon(icon.x, icon.y) {
                    attempt.outcome = error.clone();
                    attempts.push(attempt);
                    return Err(error);
                }
                clicks += 1;
                let owner = wait_owner()?;
                if let Some(owner) = owner {
                    owned(owner, known)?;
                    if owner == target {
                        let menu = super::items(&super::helper(
                            repository,
                            "tray-menu-items",
                            target,
                            None,
                        )?)?;
                        if !menu.items.is_empty() {
                            return Ok(Opened::observed(
                                menu,
                                attempt,
                                std::mem::take(&mut attempts),
                            ));
                        }
                        attempt.outcome =
                            format!("target popup {owner} had no observed items{}", menu.partial);
                    } else {
                        attempt.outcome =
                            format!("candidate belongs to other owned instance {owner}");
                    }
                    close_current(repository, known)?;
                } else {
                    attempt.outcome = "no native popup owner observed after click".to_owned();
                    super::closed(&super::helper(
                        repository,
                        "tray-menu-closed",
                        target,
                        None,
                    )?)?;
                }
                attempts.push(attempt);
                if clicks >= click_limit {
                    return Err(format!("tray click limit {click_limit} reached"));
                }
            }
        }
        Err("bounded tray candidate search found no matching menu".to_owned())
    })();
    run.map_err(|error| {
        let cleanup = close_current(repository, known);
        format!(
            "{error}; cleanup={cleanup:?}; attempts: {}",
            super::history(&attempts)
        )
    })
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
            matches!(parse(&format!("{prefix}\n{pair}\n{second}")).unwrap(), Targets::Icons(icons) if icons.len() == 2)
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
        assert!(owned(33, &[11, 22]).unwrap_err().contains("no input sent"));
    }
}

//! The pure part of W12's logout cases: what each case types, the facts the system log gives,
//! and how a case is judged. A FAIL needs an observed violation; a lost precondition or an
//! unreadable fact is NOT_TESTED.
//! plan_ref: docs/plan/11_testing_and_release.md#phase-16-macos-host
//! plan_ref: docs/plan/09_windows_shell.md#macos-adapter-mapping
use crate::mac_gui::model::{Row, Verdict};

/// The note before every case, and what each case types (Cmd+A first, so the typing replaces it).
pub(in crate::mac_gui) const NOTE: &str = "before logout\n";
pub(in crate::mac_gui) const TYPED: &str = "typed just before logout";
pub(in crate::mac_gui) const SAVED: &str = "saved line";
pub(in crate::mac_gui) const UNSAVED: &str = "unsaved line";
pub(in crate::mac_gui) const LOST: &str = "line the failing save cannot keep";
/// The runningboard records of a normal exit and of the system's final SIGKILL. A SIGTERM that
/// killed the process by default, `(2, 15, 15)`, is neither: plan 09 has the host take SIGTERM
/// into its own exit path.
pub(in crate::mac_gui) const NORMAL_EXIT: &str = "(0, 0, 0)";
pub(in crate::mac_gui) const SIGKILL: &str = "(2, 9, 9)";

/// One line of the system log about the product: where it is in the log, when it was logged
/// (µs since the epoch, when the line has a time) and what it says.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::mac_gui) struct Logged {
    pub line: usize,
    pub at: Option<i64>,
    pub value: String,
}

/// The note as read: its bytes and its modification time (µs since the epoch).
#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::mac_gui) struct Note {
    pub text: String,
    pub modified: i64,
}

/// What the coordinator observed around the logout.
#[derive(Debug)]
pub(in crate::mac_gui) struct Facts {
    /// `/dev/console` belongs to root afterwards: the GUI session ended.
    pub logged_out: bool,
    /// The product ended within the wait.
    pub gone: bool,
    /// The note just before the logout request and after the product ended.
    pub before: Result<Note, String>,
    pub after: Result<Note, String>,
    /// The dialog case's aimed destination afterwards: whether it exists.
    pub exported: Result<bool, String>,
    /// From the product's AppKit log, for its PID: the first quit event, then the first decision
    /// and reply after it; from runningboard, the PID's termination record.
    pub quit: Option<Logged>,
    pub decision: Option<Logged>,
    pub reply: Option<Logged>,
    pub termination: Option<Logged>,
}

impl Default for Facts {
    fn default() -> Self {
        let unread = || Err("not read".to_string());
        Self {
            logged_out: false,
            gone: false,
            before: unread(),
            after: unread(),
            exported: Err("not read".into()),
            quit: None,
            decision: None,
            reply: None,
            termination: None,
        }
    }
}

/// Days from 1970-01-01 to the proleptic Gregorian date (Howard Hinnant's `days_from_civil`).
fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let year = if month <= 2 { year - 1 } else { year };
    let era = year.div_euclid(400);
    let year_of_era = year - era * 400;
    let day_of_year = (153 * ((month + 9) % 12) + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146_097 + day_of_era - 719_468
}

/// The time of a `log show --style syslog` line, `2026-11-19 12:51:39.066088+0000 …`, in µs
/// since the epoch; `None` for any other shape.
pub(in crate::mac_gui) fn log_time(line: &str) -> Option<i64> {
    let mut words = line.split_whitespace();
    let (date, clock) = (words.next()?, words.next()?);
    let number = |s: &str, len: usize| -> Option<i64> {
        (s.len() == len && s.bytes().all(|b| b.is_ascii_digit()))
            .then(|| s.parse().ok())
            .flatten()
    };
    let mut d = date.split('-');
    let (year, month, day) = (
        number(d.next()?, 4)?,
        number(d.next()?, 2)?,
        number(d.next()?, 2)?,
    );
    let split = clock.rfind(['+', '-'])?;
    let (clock, offset) = clock.split_at(split);
    let sign = if offset.starts_with('-') { -1 } else { 1 };
    let offset = number(&offset[1..], 4)?;
    let (hms, fraction) = clock.split_once('.').unwrap_or((clock, ""));
    let mut t = hms.split(':');
    let (hour, minute, second) = (
        number(t.next()?, 2)?,
        number(t.next()?, 2)?,
        number(t.next()?, 2)?,
    );
    if d.next().is_some()
        || t.next().is_some()
        || !(1..=12).contains(&month)
        || !(1..=31).contains(&day)
        || hour > 23
        || minute > 59
        || second > 60
        || fraction.len() > 9
        || !fraction.bytes().all(|b| b.is_ascii_digit())
    {
        return None;
    }
    let micros: i64 = format!("{fraction:0<6}")[..6].parse().ok()?;
    let seconds = days_from_civil(year, month, day) * 86_400 + hour * 3_600 + minute * 60 + second
        - sign * ((offset / 100) * 3_600 + (offset % 100) * 60);
    Some(seconds * 1_000_000 + micros)
}

/// The facts the system log gives for `pid`: its first quit event logged no earlier than
/// `requested` (µs since the epoch, when this run asked for the logout; an earlier quit request,
/// or one without a time, is not this one), the first decision and reply after that event, and
/// runningboard's termination record for the PID.
pub(in crate::mac_gui) fn from_log(log: &str, pid: u32, requested: i64, facts: &mut Facts) {
    let own = format!("StickyMD[{pid}]");
    let theirs = format!(":{pid}] termination reported by launchd ");
    let logged = |line: usize, text: &str, value: &str| {
        Some(Logged {
            line,
            at: log_time(text),
            value: value.trim().to_owned(),
        })
    };
    for (index, line) in log.lines().enumerate() {
        if line.contains(&own) {
            if facts.quit.is_none()
                && line.contains("Handling Quit AppleEvent")
                && log_time(line).is_some_and(|at| at >= requested)
            {
                facts.quit = logged(index, line, "");
            }
            if facts.quit.is_some() {
                if let Some(rest) = line.split("applicationShouldTerminate: ").nth(1)
                    && facts.decision.is_none()
                {
                    facts.decision = logged(index, line, rest);
                }
                if let Some(rest) = line.split("replyToApplicationShouldTerminate:").nth(1)
                    && facts.reply.is_none()
                {
                    facts.reply = logged(index, line, rest);
                }
            }
        }
        if let Some(rest) = line.split(&theirs).nth(1)
            && facts.termination.is_none()
        {
            facts.termination = logged(index, line, rest);
        }
    }
}

fn record(row: &mut Row, name: &str, verdict: Verdict, detail: String) {
    if verdict == Verdict::NotTested {
        row.skip(name, &detail);
    } else {
        row.check(name, verdict == Verdict::Pass, detail);
    }
}

/// The facts in log order and each with the expected value: PASS; one seen with another value,
/// or out of order with one seen before it: FAIL (whatever else is missing); otherwise one not
/// seen: NOT_TESTED.
fn sequence(steps: &[(&Option<Logged>, &str)]) -> Verdict {
    let mut last = None;
    let mut missing = false;
    for (fact, want) in steps {
        match fact {
            None => missing = true,
            Some(logged) => {
                if logged.value != *want || last.is_some_and(|line| logged.line <= line) {
                    return Verdict::Fail;
                }
                last = Some(logged.line);
            }
        }
    }
    if missing {
        Verdict::NotTested
    } else {
        Verdict::Pass
    }
}

/// A termination record counts only when the product had ended before the driver's cleanup
/// began: after that, a SIGKILL or a normal exit may be the cleanup's own doing.
fn ended_by_itself(f: &Facts, verdict: Verdict) -> Verdict {
    if f.gone { verdict } else { Verdict::NotTested }
}

/// The note saved at the logout: the expected bytes before the request (else the precondition
/// was lost), the expected bytes after it, written no earlier than the quit event (a clean note
/// is not written at quit, so a later write is the request's own save) and, when the product
/// answers `YES`, no later than that answer: a save finishing after it was not the barrier's.
/// The modification time shows when the file was written, not that its directory was synced.
fn saved_at_request(f: &Facts, before: &str, after: &str, answer: Option<&Logged>) -> Verdict {
    let (Ok(b), Ok(a)) = (&f.before, &f.after) else {
        return Verdict::NotTested;
    };
    if b.text != before {
        return Verdict::NotTested;
    }
    if a.text != after {
        return Verdict::Fail;
    }
    if let Some(answered) = answer.and_then(|r| r.at)
        && a.modified > answered
    {
        return Verdict::Fail;
    }
    match f.quit.as_ref().and_then(|q| q.at) {
        Some(quit) if a.modified >= quit => Verdict::Pass,
        // Saved before the request reached the product (an autosave won the race), or no time
        // for the request: the save at the request was not observed.
        _ => Verdict::NotTested,
    }
}

/// The checks of one case.
pub(in crate::mac_gui) fn judge(case: &str, f: &Facts, row: &mut Row) {
    let facts = format!("{f:?}");
    if !f.logged_out {
        row.skip(
            "logout happened",
            &format!("the GUI session did not end; {facts}"),
        );
        return;
    }
    row.check(
        "logout happened",
        true,
        "the console belongs to root".to_string(),
    );
    if f.quit.is_none() {
        row.skip(
            "quit request reached the product",
            &format!("no quit event of the product's PID in the log; {facts}"),
        );
        return;
    }
    row.check(
        "quit request reached the product",
        true,
        format!("{:?}", f.quit),
    );
    let notes = format!("before {:?}, after {:?}", f.before, f.after);
    match case {
        "text" => {
            record(
                row,
                "answers later, then yes",
                sequence(&[(&f.decision, "NSTerminateLater"), (&f.reply, "YES")]),
                format!("decision {:?}, reply {:?}", f.decision, f.reply),
            );
            record(
                row,
                "unsaved text saved at the request",
                saved_at_request(f, NOTE, TYPED, f.reply.as_ref()),
                format!("{notes}, quit {:?}", f.quit),
            );
            record(
                row,
                "exits normally",
                ended_by_itself(
                    f,
                    sequence(&[(&f.reply, "YES"), (&f.termination, NORMAL_EXIT)]),
                ),
                facts,
            );
        }
        "dialog" => {
            // Only an open modal dialog makes the product cancel the request and quit itself.
            record(
                row,
                "request met the open export dialog",
                sequence(&[(&f.decision, "NSTerminateCancel")]),
                format!("decision {:?}", f.decision),
            );
            record(
                row,
                "unsaved line saved at the request",
                saved_at_request(f, SAVED, &format!("{SAVED}\n{UNSAVED}"), None),
                format!("{notes}, quit {:?}", f.quit),
            );
            record(
                row,
                "nothing exported to the aimed destination",
                match f.exported {
                    Ok(false) => Verdict::Pass,
                    Ok(true) => Verdict::Fail,
                    Err(_) => Verdict::NotTested,
                },
                format!("export.md exists: {:?}", f.exported),
            );
            record(
                row,
                "exits normally",
                ended_by_itself(
                    f,
                    sequence(&[
                        (&f.decision, "NSTerminateCancel"),
                        (&f.termination, NORMAL_EXIT),
                    ]),
                ),
                facts,
            );
        }
        "fault" => {
            record(
                row,
                "answers later, then refuses",
                sequence(&[(&f.decision, "NSTerminateLater"), (&f.reply, "NO")]),
                format!("decision {:?}, reply {:?}", f.decision, f.reply),
            );
            record(
                row,
                "old note intact",
                match (&f.before, &f.after) {
                    (Ok(b), _) if b.text != SAVED => Verdict::NotTested,
                    (Ok(b), Ok(a)) if a == b => Verdict::Pass,
                    (Ok(_), Ok(_)) => Verdict::Fail,
                    _ => Verdict::NotTested,
                },
                notes,
            );
            // Refused, so only the system ends it: a normal exit would have dropped the edit.
            record(
                row,
                "ended by the system's SIGKILL",
                ended_by_itself(f, sequence(&[(&f.reply, "NO"), (&f.termination, SIGKILL)])),
                facts,
            );
        }
        _ => row.skip("case", &format!("unknown case {case:?}")),
    }
}

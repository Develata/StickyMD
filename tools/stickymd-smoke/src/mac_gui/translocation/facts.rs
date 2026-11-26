//! R03's facts in its evidence row (`"facts"`): everything [`super::judge`] reads, so mac-ledger
//! can recompute R03's checks rather than trust them (review #24 S4). Written by the driver,
//! read back strictly: exactly these members, each of its type.
//! plan_ref: docs/plan/11_testing_and_release.md#phase-16-macos-host
use super::{Observed, Watched, judge};
use crate::{evidence::escape_json, mac_gui::model::Row, release::json::Value};

const MEMBERS: [&str; 14] = [
    "after",
    "before",
    "exited",
    "exited_early",
    "mount",
    "path",
    "quarantine",
    "stderr",
    "stderr_unreadable",
    "termination",
    "unanswered",
    "watch",
    "window",
    "zip",
];

fn text(value: &str) -> String {
    format!("\"{}\"", escape_json(value))
}

fn optional(value: Option<&str>) -> String {
    value.map_or_else(|| "null".into(), text)
}

fn list(values: &[String]) -> String {
    format!(
        "[{}]",
        values.iter().map(|v| text(v)).collect::<Vec<_>>().join(",")
    )
}

/// The facts as one JSON object.
pub(in crate::mac_gui) fn json(o: &Observed) -> String {
    let watch = o.watch.as_ref().map_or_else(
        || "null".into(),
        |w| {
            format!(
                "{{\"entries\":{},\"events\":[{}]}}",
                list(&w.entries),
                w.events
                    .iter()
                    .map(|(entry, flags)| format!(
                        "{{\"entry\":{},\"flags\":{flags}}}",
                        text(entry)
                    ))
                    .collect::<Vec<_>>()
                    .join(",")
            )
        },
    );
    format!(
        "{{\"zip\":{},\"quarantine\":{},\"path\":{},\"mount\":{},\"exited_early\":{},\"window\":{},\"stderr\":{},\"stderr_unreadable\":{},\"unanswered\":{},\"exited\":{},\"termination\":{},\"before\":{},\"after\":{},\"watch\":{watch}}}",
        optional(o.zip.as_deref()),
        optional(o.quarantine.as_deref()),
        optional(o.path.as_deref()),
        optional(o.mount.as_deref()),
        o.exited_early,
        optional(o.window.as_deref()),
        text(&o.stderr),
        optional(o.stderr_unreadable.as_deref()),
        optional(o.unanswered.as_deref()),
        o.exited.map_or_else(|| "null".into(), |e| e.to_string()),
        optional(o.termination.as_deref()),
        list(&o.before),
        list(&o.after),
    )
}

/// An object with exactly `members`.
fn exactly<'a>(value: &'a Value, members: &[&str], what: &str) -> Result<&'a Value, String> {
    let Value::Object(map) = value else {
        return Err(format!("{what} is not an object"));
    };
    let mut keys: Vec<&str> = map.keys().map(String::as_str).collect();
    let mut wanted = members.to_vec();
    keys.sort_unstable();
    wanted.sort_unstable();
    if keys != wanted {
        return Err(format!("{what} has the members {keys:?}, not {wanted:?}"));
    }
    Ok(value)
}

fn boolean(value: &Value, key: &str) -> Result<bool, String> {
    match value.field(key)? {
        Value::Bool(b) => Ok(*b),
        _ => Err(format!("R03 facts: {key} is not a boolean")),
    }
}

fn optional_bool(value: &Value, key: &str) -> Result<Option<bool>, String> {
    match value.field(key)? {
        Value::Null => Ok(None),
        Value::Bool(b) => Ok(Some(*b)),
        _ => Err(format!("R03 facts: {key} is neither a boolean nor null")),
    }
}

fn optional_text(value: &Value, key: &str) -> Result<Option<String>, String> {
    Ok(value.field(key)?.optional_string()?.map(str::to_owned))
}

fn strings(value: &Value) -> Result<Vec<String>, String> {
    value
        .array()?
        .iter()
        .map(|v| v.string().map(str::to_owned))
        .collect()
}

/// The facts read back from their JSON object.
pub(in crate::mac_gui) fn parse(value: &Value) -> Result<Observed, String> {
    let value = exactly(value, &MEMBERS, "R03 facts")?;
    let watch = match value.field("watch")? {
        Value::Null => None,
        watch => {
            let watch = exactly(watch, &["entries", "events"], "R03 facts' watch")?;
            let events = watch
                .field("events")?
                .array()?
                .iter()
                .map(|event| {
                    let event = exactly(event, &["entry", "flags"], "a watch event")?;
                    let flags = u32::try_from(event.field("flags")?.unsigned()?)
                        .map_err(|_| "a watch event's flags exceed 32 bits".to_owned())?;
                    Ok((event.field("entry")?.string()?.to_owned(), flags))
                })
                .collect::<Result<Vec<_>, String>>()?;
            Some(Watched {
                entries: strings(watch.field("entries")?)?,
                events,
            })
        }
    };
    Ok(Observed {
        zip: optional_text(value, "zip")?,
        quarantine: optional_text(value, "quarantine")?,
        path: optional_text(value, "path")?,
        mount: optional_text(value, "mount")?,
        exited_early: boolean(value, "exited_early")?,
        window: optional_text(value, "window")?,
        stderr: value.field("stderr")?.string()?.to_owned(),
        stderr_unreadable: optional_text(value, "stderr_unreadable")?,
        unanswered: optional_text(value, "unanswered")?,
        exited: optional_bool(value, "exited")?,
        termination: optional_text(value, "termination")?,
        before: strings(value.field("before")?)?,
        after: strings(value.field("after")?)?,
        watch,
    })
}

/// The facts of a run in which every R03 observation holds, opened from `zip`, with these
/// watch events: the sample mac-ledger's tests record and tamper with.
#[cfg(test)]
pub(crate) fn sample(zip: Option<&str>, events: &[(&str, u32)]) -> String {
    let tree = vec![
        "dir . mtime=1".to_owned(),
        "dir StickyMD.app mtime=2".to_owned(),
        "file StickyMD.app/Info.plist bytes=10 sha256=ab mtime=3".to_owned(),
    ];
    json(&Observed {
        zip: zip.map(str::to_owned),
        quarantine: Some(format!("{};6afec4a5;Safari;", super::QUARANTINE_FLAGS)),
        path: Some("/private/var/folders/x/T/AppTranslocation/U/d/StickyMD.app/Contents/MacOS/StickyMD".into()),
        mount: Some("/tmp/download/StickyMD.app on /private/var/folders/x/T/AppTranslocation/U (nullfs, local, read-only, mounted by admin)".into()),
        exited_early: false,
        window: Some("Window { id: 64 }".into()),
        stderr: format!(
            "{}\n\ncannot create a file: {} (os error 30)\n",
            super::TRANSLOCATED_PROMPT,
            super::CAUSE
        ),
        stderr_unreadable: None,
        unanswered: None,
        exited: Some(true),
        termination: Some(super::EXIT_ONE.into()),
        before: tree.clone(),
        after: tree,
        watch: Some(Watched {
            entries: [".", "StickyMD.app", "StickyMD.app/Info.plist"].map(str::to_owned).to_vec(),
            events: events.iter().map(|(e, f)| ((*e).to_owned(), *f)).collect(),
        }),
    })
}

/// R03 judged again from recorded facts.
#[derive(Debug, PartialEq)]
pub(crate) struct Rejudged {
    /// The ZIP the facts say the download was laid out from.
    pub zip: Option<String>,
    /// Each check as `(name, verdict, observation)`, in the order the driver records them.
    pub checks: Vec<(String, String, String)>,
    /// Whether "nothing written" was read through the write watch.
    pub watched: bool,
}

/// Recomputes R03's checks from the facts of its evidence row.
pub(crate) fn rejudge(facts: &Value) -> Result<Rejudged, String> {
    let observed = parse(facts)?;
    let mut row = Row::new("R03");
    judge(&observed, &mut row);
    Ok(Rejudged {
        watched: observed.watch.is_some(),
        zip: observed.zip,
        checks: row
            .checks
            .into_iter()
            .map(|c| (c.name, c.verdict.name().to_owned(), c.observation))
            .collect(),
    })
}

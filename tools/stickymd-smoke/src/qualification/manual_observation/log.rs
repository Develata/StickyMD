//! The append-only manual observation log: its strict format and the append rule. Files,
//! locking and context changes live in `store`; the verdict lives in `reduce`.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use super::super::json::{self, Value};
use super::registry;
use super::role::Facts;
use super::target::{Machine, Role, is_context_id};

pub(in crate::qualification) const MAX_OBSERVATIONS: usize = 10_000;
pub(in crate::qualification) const MAX_NOTE_BYTES: usize = 2048;
const MAX_DECLARED: usize = 16;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::qualification) enum Status {
    Pass,
    Fail,
}

impl Status {
    pub(in crate::qualification) const fn as_str(self) -> &'static str {
        match self {
            Self::Pass => "MANUAL_PASS",
            Self::Fail => "MANUAL_FAIL",
        }
    }

    fn parse(value: &str) -> Result<Self, String> {
        match value {
            "MANUAL_PASS" => Ok(Self::Pass),
            "MANUAL_FAIL" => Ok(Self::Fail),
            _ => Err(format!(
                "observation status `{value}` is not MANUAL_PASS or MANUAL_FAIL"
            )),
        }
    }
}

/// The process the recorder started and the observer watched.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::qualification) struct Instance {
    pub(in crate::qualification) exe_sha256: String,
    pub(in crate::qualification) pid: u32,
}

/// An earlier context's unresolved FAIL that a reset carried into this one.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::qualification) struct Carried {
    pub(in crate::qualification) context_id: String,
    pub(in crate::qualification) index: u64,
}

/// One recorded answer for one scenario of one case on one role. Its position in the log
/// is its registration index; `base` is the log length when its session started, so a
/// FAIL at index `i` was visible to this observation exactly when `i < base`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::qualification) struct Observation {
    pub(in crate::qualification) session: String,
    pub(in crate::qualification) seq: u64,
    pub(in crate::qualification) base: u64,
    pub(in crate::qualification) case: String,
    pub(in crate::qualification) scenario: String,
    pub(in crate::qualification) role: Role,
    pub(in crate::qualification) status: Status,
    pub(in crate::qualification) note: String,
    pub(in crate::qualification) observed: Facts,
    /// Scenario parameters the observer declared (DPI, topology, theme, ...).
    pub(in crate::qualification) declared: BTreeMap<String, String>,
    /// `None` only for the M34 first run, which the tool did not start.
    pub(in crate::qualification) instance: Option<Instance>,
    pub(in crate::qualification) carried_from: Vec<Carried>,
    pub(in crate::qualification) recorded_at_unix: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::qualification) struct Reset {
    pub(in crate::qualification) decision: String,
    pub(in crate::qualification) previous_context_id: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::qualification) struct Header {
    pub(in crate::qualification) context_id: String,
    pub(in crate::qualification) candidate_receipt_sha256: String,
    pub(in crate::qualification) target_sha256: String,
    pub(in crate::qualification) registry_sha256: String,
    pub(in crate::qualification) reset: Option<Reset>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::qualification) struct Log {
    pub(in crate::qualification) header: Header,
    pub(in crate::qualification) observations: Vec<Observation>,
}

impl Log {
    /// Appends `batch` in order. An observation whose ID is already logged with the same
    /// content is skipped (re-import); the same ID with other content rejects the whole
    /// batch, as does a `base` beyond the log or a full log. Returns how many were added.
    pub(in crate::qualification) fn append(
        &mut self,
        batch: Vec<Observation>,
    ) -> Result<usize, String> {
        let length = self.observations.len() as u64;
        // ID -> position among the logged and the newly accepted observations.
        let mut index: HashMap<(&str, u64), usize> = self
            .observations
            .iter()
            .enumerate()
            .map(|(position, logged)| ((logged.session.as_str(), logged.seq), position))
            .collect();
        let mut added: Vec<&Observation> = Vec::new();
        for observation in &batch {
            validate_observation(observation, length + added.len() as u64)?;
            if observation.base > length {
                return Err(format!(
                    "observation {}#{} starts after the log ({} > {length})",
                    observation.session, observation.seq, observation.base
                ));
            }
            match index.get(&(observation.session.as_str(), observation.seq)) {
                Some(&position) => {
                    let logged = self
                        .observations
                        .get(position)
                        .unwrap_or_else(|| added[position - self.observations.len()]);
                    if logged != observation {
                        return Err(format!(
                            "observation {}#{} is already logged with different content",
                            observation.session, observation.seq
                        ));
                    }
                }
                None => {
                    index.insert(
                        (observation.session.as_str(), observation.seq),
                        self.observations.len() + added.len(),
                    );
                    added.push(observation);
                }
            }
        }
        if self.observations.len() + added.len() > MAX_OBSERVATIONS {
            return Err(format!(
                "the manual observation log is full ({MAX_OBSERVATIONS} observations); a USER reset starts a new context"
            ));
        }
        let count = added.len();
        let added = added.into_iter().cloned().collect::<Vec<_>>();
        drop(index);
        self.observations.extend(added);
        Ok(count)
    }
}

pub(in crate::qualification) fn parse(text: &str) -> Result<Log, String> {
    let document = json::parse_object(text)?;
    json::exact_keys(
        &document,
        &[
            "schema_version",
            "context_id",
            "candidate_receipt_sha256",
            "target_sha256",
            "registry_sha256",
            "reset",
            "observations",
        ],
    )?;
    if json::u64_field(&document, "schema_version")? != 1 {
        return Err("manual observation log schema is not version 1".to_owned());
    }
    let context_id = json::string_field(&document, "context_id")?;
    if !is_context_id(&context_id) {
        return Err(format!("log context `{context_id}` is not 32 hex digits"));
    }
    let header = Header {
        context_id,
        candidate_receipt_sha256: sha256_field(&document, "candidate_receipt_sha256")?,
        target_sha256: sha256_field(&document, "target_sha256")?,
        registry_sha256: sha256_field(&document, "registry_sha256")?,
        reset: parse_reset(json::field(&document, "reset")?)?,
    };
    let objects = json::objects(&document, "observations")?;
    if objects.len() > MAX_OBSERVATIONS {
        return Err(format!(
            "manual observation log holds {} observations, more than {MAX_OBSERVATIONS}",
            objects.len()
        ));
    }
    let mut ids = BTreeSet::new();
    let mut observations = Vec::with_capacity(objects.len());
    for (index, object) in objects.iter().enumerate() {
        let observation = parse_observation(object)?;
        validate_observation(&observation, index as u64)?;
        if !ids.insert((observation.session.clone(), observation.seq)) {
            return Err(format!(
                "observation {}#{} is logged twice",
                observation.session, observation.seq
            ));
        }
        observations.push(observation);
    }
    Ok(Log {
        header,
        observations,
    })
}

fn sha256_field(object: &Value, key: &str) -> Result<String, String> {
    let value = json::string_field(object, key)?;
    crate::integrity::validate_sha256(&value, key)?;
    Ok(value)
}

fn parse_reset(value: &Value) -> Result<Option<Reset>, String> {
    if *value == Value::Null {
        return Ok(None);
    }
    json::exact_keys(value, &["decision", "previous_context_id"])?;
    let decision = json::string_field(value, "decision")?;
    if !is_reset_decision(&decision) {
        return Err(format!("log reset decision `{decision}` is malformed"));
    }
    let previous_context_id = json::string_field(value, "previous_context_id")?;
    if !is_context_id(&previous_context_id) {
        return Err("log reset names a malformed previous context".to_owned());
    }
    Ok(Some(Reset {
        decision,
        previous_context_id,
    }))
}

pub(in crate::qualification) fn is_reset_decision(key: &str) -> bool {
    key.strip_prefix("RESET-MANUAL-LOG-")
        .is_some_and(|digest| crate::integrity::validate_sha256(digest, "reset digest").is_ok())
}

fn parse_observation(object: &Value) -> Result<Observation, String> {
    json::exact_keys(
        object,
        &[
            "session",
            "seq",
            "base",
            "case",
            "scenario",
            "role",
            "status",
            "note",
            "observed",
            "declared",
            "instance",
            "carried_from",
            "recorded_at_unix",
        ],
    )?;
    let observed = json::object_field(object, "observed")?;
    json::exact_keys(
        observed,
        &[
            "display_version",
            "current_build",
            "ubr",
            "edition_id",
            "native_machine",
        ],
    )?;
    let instance = match json::field(object, "instance")? {
        Value::Null => None,
        value => {
            json::exact_keys(value, &["exe_sha256", "pid"])?;
            Some(Instance {
                exe_sha256: sha256_field(value, "exe_sha256")?,
                pid: u32_field(value, "pid")?,
            })
        }
    };
    let mut carried_from = Vec::new();
    for carried in json::objects(object, "carried_from")? {
        json::exact_keys(carried, &["context_id", "index"])?;
        carried_from.push(Carried {
            context_id: json::string_field(carried, "context_id")?,
            index: json::u64_field(carried, "index")?,
        });
    }
    let Value::Object(declared) = json::object_field(object, "declared")? else {
        unreachable!("object_field returns objects");
    };
    let declared = declared
        .iter()
        .map(|(key, value)| match value {
            Value::String(text) => Ok((key.clone(), text.clone())),
            _ => Err(format!("declared parameter `{key}` is not a string")),
        })
        .collect::<Result<BTreeMap<_, _>, _>>()?;
    Ok(Observation {
        session: json::string_field(object, "session")?,
        seq: json::u64_field(object, "seq")?,
        base: json::u64_field(object, "base")?,
        case: json::string_field(object, "case")?,
        scenario: json::string_field(object, "scenario")?,
        role: Role::parse(&json::string_field(object, "role")?)?,
        status: Status::parse(&json::string_field(object, "status")?)?,
        note: json::string_field(object, "note")?,
        observed: Facts {
            display_version: json::string_field(observed, "display_version")?,
            current_build: u32_field(observed, "current_build")?,
            ubr: u32_field(observed, "ubr")?,
            edition_id: json::string_field(observed, "edition_id")?,
            native_machine: Machine::parse(&json::string_field(observed, "native_machine")?)?,
        },
        declared,
        instance,
        carried_from,
        recorded_at_unix: json::u64_field(object, "recorded_at_unix")?,
    })
}

fn u32_field(object: &Value, key: &str) -> Result<u32, String> {
    u32::try_from(json::u64_field(object, key)?)
        .map_err(|_| format!("JSON field `{key}` does not fit 32 bits"))
}

/// Checks one observation in isolation, given the index it has or would have in the log.
fn validate_observation(observation: &Observation, index: u64) -> Result<(), String> {
    let label = format!("observation {}#{}", observation.session, observation.seq);
    if !is_context_id(&observation.session) || observation.seq == 0 {
        return Err(format!("{label} has a malformed ID"));
    }
    if observation.base > index {
        return Err(format!("{label} starts after its own position"));
    }
    let case = registry::find(&observation.case)
        .ok_or_else(|| format!("{label} names unknown case `{}`", observation.case))?;
    if !case.scenarios.contains(&observation.scenario.as_str()) {
        return Err(format!(
            "{label} names unknown scenario `{}/{}`",
            observation.case, observation.scenario
        ));
    }
    if observation.note.len() > MAX_NOTE_BYTES {
        return Err(format!("{label} note exceeds {MAX_NOTE_BYTES} bytes"));
    }
    let facts = &observation.observed;
    if facts.display_version.is_empty()
        || facts.display_version.len() > 16
        || facts.edition_id.is_empty()
        || facts.edition_id.len() > 64
    {
        return Err(format!("{label} has malformed environment facts"));
    }
    if observation.declared.len() > MAX_DECLARED
        || observation.declared.iter().any(|(key, value)| {
            key.is_empty()
                || key.len() > 32
                || !key
                    .bytes()
                    .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
                || value.len() > 128
        })
    {
        return Err(format!("{label} has malformed declared parameters"));
    }
    if observation
        .instance
        .as_ref()
        .is_some_and(|instance| instance.pid == 0)
    {
        return Err(format!("{label} names process 0"));
    }
    if observation
        .carried_from
        .iter()
        .any(|carried| !is_context_id(&carried.context_id))
    {
        return Err(format!("{label} carries from a malformed context"));
    }
    if !observation.carried_from.is_empty() && observation.status != Status::Fail {
        return Err(format!("{label} is carried but is not a FAIL"));
    }
    Ok(())
}

pub(in crate::qualification) fn render(log: &Log) -> String {
    let header = &log.header;
    let reset = header.reset.as_ref().map_or_else(
        || "null".to_owned(),
        |reset| {
            format!(
                "{{\"decision\":\"{}\",\"previous_context_id\":\"{}\"}}",
                json::escape(&reset.decision),
                reset.previous_context_id
            )
        },
    );
    let observations = log
        .observations
        .iter()
        .map(render_observation)
        .collect::<Vec<_>>()
        .join(",\n");
    format!(
        "{{\"schema_version\":1,\"context_id\":\"{}\",\"candidate_receipt_sha256\":\"{}\",\"target_sha256\":\"{}\",\"registry_sha256\":\"{}\",\"reset\":{reset},\"observations\":[\n{observations}\n]}}\n",
        header.context_id,
        header.candidate_receipt_sha256,
        header.target_sha256,
        header.registry_sha256,
    )
}

fn render_observation(observation: &Observation) -> String {
    let facts = &observation.observed;
    let declared = observation
        .declared
        .iter()
        .map(|(key, value)| format!("\"{}\":\"{}\"", json::escape(key), json::escape(value)))
        .collect::<Vec<_>>()
        .join(",");
    let instance = observation.instance.as_ref().map_or_else(
        || "null".to_owned(),
        |instance| {
            format!(
                "{{\"exe_sha256\":\"{}\",\"pid\":{}}}",
                instance.exe_sha256, instance.pid
            )
        },
    );
    let carried = observation
        .carried_from
        .iter()
        .map(|carried| {
            format!(
                "{{\"context_id\":\"{}\",\"index\":{}}}",
                carried.context_id, carried.index
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "{{\"session\":\"{}\",\"seq\":{},\"base\":{},\"case\":\"{}\",\"scenario\":\"{}\",\"role\":\"{}\",\"status\":\"{}\",\"note\":\"{}\",\"observed\":{{\"display_version\":\"{}\",\"current_build\":{},\"ubr\":{},\"edition_id\":\"{}\",\"native_machine\":\"{}\"}},\"declared\":{{{declared}}},\"instance\":{instance},\"carried_from\":[{carried}],\"recorded_at_unix\":{}}}",
        observation.session,
        observation.seq,
        observation.base,
        json::escape(&observation.case),
        json::escape(&observation.scenario),
        observation.role.as_str(),
        observation.status.as_str(),
        json::escape(&observation.note),
        json::escape(&facts.display_version),
        facts.current_build,
        facts.ubr,
        json::escape(&facts.edition_id),
        facts.native_machine.as_str(),
        observation.recorded_at_unix,
    )
}

#[cfg(test)]
pub(in crate::qualification) mod fixtures {
    use std::collections::BTreeMap;

    use super::{Header, Instance, Log, Observation, Status};
    use crate::qualification::manual_observation::role::fixtures::{current, previous};
    use crate::qualification::manual_observation::target::Role;

    pub(in crate::qualification) const SESSION: &str = "fedcba9876543210fedcba9876543210";

    pub(in crate::qualification) fn empty() -> Log {
        Log {
            header: Header {
                context_id: "0123456789abcdef0123456789abcdef".to_owned(),
                candidate_receipt_sha256: "c".repeat(64),
                target_sha256: "d".repeat(64),
                registry_sha256: "e".repeat(64),
                reset: None,
            },
            observations: Vec::new(),
        }
    }

    /// An observation of `case/scenario` on `role` made from a session that saw `base`
    /// logged observations.
    pub(in crate::qualification) fn observation(
        seq: u64,
        base: u64,
        case: &str,
        scenario: &str,
        role: Role,
        status: Status,
    ) -> Observation {
        Observation {
            session: SESSION.to_owned(),
            seq,
            base,
            case: case.to_owned(),
            scenario: scenario.to_owned(),
            role,
            status,
            note: String::new(),
            observed: match role {
                Role::Current => current(),
                Role::Previous => previous(),
            },
            declared: BTreeMap::from([("dpi".to_owned(), "150".to_owned())]),
            instance: Some(Instance {
                exe_sha256: "a".repeat(64),
                pid: 4242,
            }),
            carried_from: Vec::new(),
            recorded_at_unix: 1_791_900_000,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::fixtures::{empty, observation};
    use super::{MAX_OBSERVATIONS, Status, parse, render};
    use crate::qualification::manual_observation::target::Role::{Current, Previous};

    #[test]
    fn a_rendered_log_parses_back_unchanged() {
        let mut log = empty();
        let mut first = observation(1, 0, "P12-M01", "pinyin-100", Current, Status::Fail);
        first.note = "candidate window \"clipped\"\nat 150%".to_owned();
        log.append(vec![
            first,
            observation(2, 0, "P12-M23", "system-to-dark", Previous, Status::Pass),
        ])
        .unwrap();
        assert_eq!(parse(&render(&log)).unwrap(), log);
    }

    #[test]
    fn re_importing_the_same_observation_is_a_no_op_but_a_changed_one_is_rejected() {
        let mut log = empty();
        let first = observation(1, 0, "P12-M05", "restore-tray", Current, Status::Pass);
        assert_eq!(log.append(vec![first.clone()]).unwrap(), 1);
        assert_eq!(log.append(vec![first.clone()]).unwrap(), 0);
        let mut changed = first;
        changed.status = Status::Fail;
        let error = log.append(vec![changed]).unwrap_err();
        assert!(error.contains("different content"), "{error}");
        assert_eq!(log.observations.len(), 1);
    }

    #[test]
    fn a_bad_batch_adds_nothing() {
        let mut log = empty();
        let good = observation(1, 0, "P12-M05", "restore-tray", Current, Status::Pass);
        for bad in [
            observation(2, 5, "P12-M05", "restore-tray", Current, Status::Pass),
            observation(2, 0, "P12-M99", "restore-tray", Current, Status::Pass),
            observation(2, 0, "P12-M05", "restore-email", Current, Status::Pass),
            observation(0, 0, "P12-M05", "restore-tray", Current, Status::Pass),
        ] {
            assert!(log.append(vec![good.clone(), bad]).is_err());
            assert!(log.observations.is_empty());
        }
    }

    #[test]
    fn a_full_log_rejects_new_observations() {
        let mut log = empty();
        log.observations = (1..=MAX_OBSERVATIONS as u64)
            .map(|seq| {
                observation(
                    seq,
                    0,
                    "P12-M24",
                    "representative-markdown",
                    Current,
                    Status::Pass,
                )
            })
            .collect();
        let next = observation(
            MAX_OBSERVATIONS as u64 + 1,
            0,
            "P12-M24",
            "representative-markdown",
            Current,
            Status::Pass,
        );
        assert!(log.append(vec![next]).unwrap_err().contains("full"));
    }

    #[test]
    fn malformed_logs_are_rejected_whole() {
        let mut log = empty();
        log.append(vec![observation(
            1,
            0,
            "P12-M05",
            "restore-tray",
            Current,
            Status::Pass,
        )])
        .unwrap();
        let good = render(&log);
        for (from, to) in [
            ("\"status\":\"MANUAL_PASS\"", "\"status\":\"NOT_TESTED\""),
            ("\"base\":0", "\"base\":1"),
            ("\"role\":\"current\"", "\"role\":\"both\""),
            ("\"recorded_at_unix\"", "\"extra\":1,\"recorded_at_unix\""),
            (
                "\"reset\":null",
                "\"reset\":{\"decision\":\"RESET-MANUAL-LOG-x\",\"previous_context_id\":\"0123456789abcdef0123456789abcdef\"}",
            ),
            ("\"pid\":4242", "\"pid\":0"),
            ("\"dpi\":\"150\"", "\"dpi\":150"),
        ] {
            let broken = good.replacen(from, to, 1);
            assert_ne!(broken, good, "{from}");
            assert!(parse(&broken).is_err(), "{to}");
        }
        let twice = good.replacen(
            "\n]}",
            &format!(",{}\n]}}", good.lines().nth(1).unwrap()),
            1,
        );
        assert!(parse(&twice).unwrap_err().contains("twice"));
    }
}

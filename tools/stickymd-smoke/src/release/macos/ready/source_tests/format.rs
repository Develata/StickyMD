//! The receipt as JSON (schema 3): written whole by the runner, read whole and strictly by the
//! gate. Every object has exactly its fields; a missing, extra or mistyped one rejects the file.
//! Earlier schemas are refused, not migrated: schema 1 had counts only, schema 2 had no run
//! identity, no driver and no account of how a process ended (review round 4).
//! plan_ref: docs/plan/11_testing_and_release.md#macos-readiness-check

use super::{Case, CaseOutcome, Counts, Driver, Exit, Explicit, Host, Receipt, Run, Side, Target};
use crate::evidence::escape_json;
use crate::release::json::{self, Value};

fn text(value: &str) -> String {
    format!("\"{}\"", escape_json(value))
}

fn list(values: &[String]) -> String {
    let items: Vec<String> = values.iter().map(|value| text(value)).collect();
    format!("[{}]", items.join(","))
}

/// An exit code as a number; the other endings as words.
fn exit(exit: Exit) -> String {
    match exit {
        Exit::Code(code) => code.to_string(),
        Exit::Signal => "\"signal\"".to_owned(),
        Exit::NotStarted => "\"not-started\"".to_owned(),
        Exit::Unknown => "\"unknown\"".to_owned(),
    }
}

fn driver(driver: &Driver) -> String {
    match &driver.sha256 {
        Ok(sha256) => format!(
            "{{\"command\":{},\"sha256\":{}}}",
            list(&driver.command),
            text(sha256)
        ),
        Err(error) => format!(
            "{{\"command\":{},\"error\":{}}}",
            list(&driver.command),
            text(error)
        ),
    }
}

fn counts(counts: &Counts) -> String {
    format!(
        "{{\"ran\":{},\"passed\":{},\"failed\":{},\"ignored\":{},\"measured\":{},\"filtered_out\":{}}}",
        counts.ran,
        counts.passed,
        counts.failed,
        counts.ignored,
        counts.measured,
        counts.filtered_out
    )
}

fn target(target: &Target) -> String {
    format!(
        "{{\"package\":{},\"kind\":{},\"name\":{}}}",
        text(&target.package),
        text(&target.kind),
        text(&target.name)
    )
}

fn cases(cases: &[Case]) -> String {
    let items: Vec<String> = cases
        .iter()
        .map(|case| {
            format!(
                "{{\"name\":{},\"ignored\":{},\"outcome\":\"{}\"}}",
                text(&case.name),
                case.ignored,
                case.outcome.label()
            )
        })
        .collect();
    format!("[{}]", items.join(","))
}

fn side(side: &Side) -> String {
    format!("{{\"head\":{},\"clean\":{}}}", text(&side.head), side.clean)
}

fn optional<T>(value: Option<&T>, render: impl Fn(&T) -> String) -> String {
    value.map_or_else(|| "null".to_owned(), render)
}

pub(in super::super) fn render(receipt: &Receipt) -> String {
    let host = &receipt.host;
    let expected: Vec<String> = receipt.expected.iter().map(target).collect();
    let binaries: Vec<String> = receipt
        .binaries
        .iter()
        .map(|run| {
            format!(
                "{{\"target\":{},\"counts\":{},\"cases\":{}}}",
                target(&run.target),
                counts(&run.counts),
                cases(&run.cases)
            )
        })
        .collect();
    let explicit: Vec<String> = receipt
        .explicit
        .iter()
        .map(|entry| {
            format!(
                "{{\"item\":{},\"command\":{},\"tests\":{},\"target\":{},\"counts\":{},\"cases\":{},\"listed\":{},\"exit\":{},\"temporary_volume\":{},\"image_volume\":{}}}",
                text(&entry.item),
                list(&entry.command),
                list(&entry.tests),
                optional(entry.target.as_ref(), target),
                optional(entry.counts.as_ref(), counts),
                cases(&entry.cases),
                list(&entry.listed),
                exit(entry.exit),
                optional(entry.temporary_volume.as_ref(), |v: &String| text(v)),
                optional(entry.image_volume.as_ref(), |v: &String| text(v)),
            )
        })
        .collect();
    format!(
        "{{\"schema_version\":3,\"kind\":\"macos-release-source-tests\",\"release_source\":{},\"run\":{},\"host\":{{\"product_name\":{},\"product_version\":{},\"build_version\":{},\"arch\":{},\"virtual_machine\":{},\"cargo\":{},\"rustc\":{}}},\"start\":{},\"end\":{},\"command\":{},\"exit\":{},\"expected\":[{}],\"binaries\":[{}],\"explicit\":[{}],\"driver\":{},\"problems\":{}}}\n",
        text(&receipt.release_source),
        text(&receipt.run),
        text(&host.product_name),
        text(&host.product_version),
        text(&host.build_version),
        text(&host.arch),
        text(&host.virtual_machine),
        text(&host.cargo),
        text(&host.rustc),
        side(&receipt.start),
        side(&receipt.end),
        list(&receipt.command),
        exit(receipt.exit),
        expected.join(","),
        binaries.join(","),
        explicit.join(","),
        driver(&receipt.driver),
        list(&receipt.problems),
    )
}

/// `value` is an object with exactly `keys`.
fn exactly(value: &Value, keys: &[&str]) -> Result<(), String> {
    let Value::Object(fields) = value else {
        return Err("expected a JSON object".to_owned());
    };
    let mut wanted = keys.to_vec();
    wanted.sort_unstable();
    let found: Vec<&str> = fields.keys().map(String::as_str).collect();
    if found == wanted {
        Ok(())
    } else {
        Err(format!(
            "expected exactly the fields {wanted:?}, found {found:?}"
        ))
    }
}

fn string(value: &Value, key: &str) -> Result<String, String> {
    Ok(value.field(key)?.string()?.to_owned())
}

fn strings(value: &Value, key: &str) -> Result<Vec<String>, String> {
    value
        .field(key)?
        .array()?
        .iter()
        .map(|item| item.string().map(str::to_owned))
        .collect()
}

fn boolean(value: &Value, key: &str) -> Result<bool, String> {
    match value.field(key)? {
        Value::Bool(flag) => Ok(*flag),
        _ => Err(format!("{key} is not a boolean")),
    }
}

fn read_exit(value: &Value, key: &str) -> Result<Exit, String> {
    match value.field(key)? {
        Value::Number(number) => number
            .parse()
            .map(Exit::Code)
            .map_err(|_| format!("{key} is not an exit code")),
        Value::String(word) => match word.as_str() {
            "signal" => Ok(Exit::Signal),
            "not-started" => Ok(Exit::NotStarted),
            "unknown" => Ok(Exit::Unknown),
            _ => Err(format!("{key} {word:?} is not how a process ends")),
        },
        _ => Err(format!("{key} is not how a process ends")),
    }
}

fn read_driver(value: &Value) -> Result<Driver, String> {
    let sha256 = if exactly(value, &["command", "sha256"]).is_ok() {
        Ok(string(value, "sha256")?)
    } else {
        exactly(value, &["command", "error"])?;
        Err(string(value, "error")?)
    };
    Ok(Driver {
        command: strings(value, "command")?,
        sha256,
    })
}

fn read_counts(value: &Value) -> Result<Counts, String> {
    exactly(
        value,
        &[
            "ran",
            "passed",
            "failed",
            "ignored",
            "measured",
            "filtered_out",
        ],
    )?;
    let number = |key: &str| value.field(key)?.unsigned();
    Ok(Counts {
        ran: number("ran")?,
        passed: number("passed")?,
        failed: number("failed")?,
        ignored: number("ignored")?,
        measured: number("measured")?,
        filtered_out: number("filtered_out")?,
    })
}

fn read_target(value: &Value) -> Result<Target, String> {
    exactly(value, &["package", "kind", "name"])?;
    Ok(Target {
        package: string(value, "package")?,
        kind: string(value, "kind")?,
        name: string(value, "name")?,
    })
}

fn read_cases(value: &Value) -> Result<Vec<Case>, String> {
    value
        .array()?
        .iter()
        .map(|case| {
            exactly(case, &["name", "ignored", "outcome"])?;
            let label = string(case, "outcome")?;
            Ok(Case {
                name: string(case, "name")?,
                ignored: boolean(case, "ignored")?,
                outcome: CaseOutcome::from_label(&label)
                    .ok_or_else(|| format!("unknown test outcome {label:?}"))?,
            })
        })
        .collect()
}

fn read_side(value: &Value) -> Result<Side, String> {
    exactly(value, &["head", "clean"])?;
    Ok(Side {
        head: string(value, "head")?,
        clean: boolean(value, "clean")?,
    })
}

fn nullable<T>(
    value: &Value,
    key: &str,
    read: impl Fn(&Value) -> Result<T, String>,
) -> Result<Option<T>, String> {
    match value.field(key)? {
        Value::Null => Ok(None),
        present => read(present).map(Some),
    }
}

pub(in super::super) fn parse(text: &str) -> Result<Receipt, String> {
    let value = json::parse(text)?;
    exactly(
        &value,
        &[
            "schema_version",
            "kind",
            "release_source",
            "run",
            "host",
            "start",
            "end",
            "command",
            "exit",
            "expected",
            "binaries",
            "explicit",
            "driver",
            "problems",
        ],
    )?;
    if value.field("schema_version")?.unsigned()? != 3
        || string(&value, "kind")? != "macos-release-source-tests"
    {
        return Err("not a schema 3 macOS source-test receipt".to_owned());
    }
    let host = value.field("host")?;
    exactly(
        host,
        &[
            "product_name",
            "product_version",
            "build_version",
            "arch",
            "virtual_machine",
            "cargo",
            "rustc",
        ],
    )?;
    let binaries = value
        .field("binaries")?
        .array()?
        .iter()
        .map(|run| {
            exactly(run, &["target", "counts", "cases"])?;
            Ok(Run {
                target: read_target(run.field("target")?)?,
                counts: read_counts(run.field("counts")?)?,
                cases: read_cases(run.field("cases")?)?,
            })
        })
        .collect::<Result<_, String>>()?;
    let explicit = value
        .field("explicit")?
        .array()?
        .iter()
        .map(|entry| {
            exactly(
                entry,
                &[
                    "item",
                    "command",
                    "tests",
                    "target",
                    "counts",
                    "cases",
                    "listed",
                    "exit",
                    "temporary_volume",
                    "image_volume",
                ],
            )?;
            Ok(Explicit {
                item: string(entry, "item")?,
                command: strings(entry, "command")?,
                tests: strings(entry, "tests")?,
                target: nullable(entry, "target", read_target)?,
                counts: nullable(entry, "counts", read_counts)?,
                cases: read_cases(entry.field("cases")?)?,
                listed: strings(entry, "listed")?,
                exit: read_exit(entry, "exit")?,
                temporary_volume: nullable(entry, "temporary_volume", |v| {
                    v.string().map(str::to_owned)
                })?,
                image_volume: nullable(entry, "image_volume", |v| v.string().map(str::to_owned))?,
            })
        })
        .collect::<Result<_, String>>()?;
    Ok(Receipt {
        release_source: string(&value, "release_source")?,
        run: string(&value, "run")?,
        host: Host {
            product_name: string(host, "product_name")?,
            product_version: string(host, "product_version")?,
            build_version: string(host, "build_version")?,
            arch: string(host, "arch")?,
            virtual_machine: string(host, "virtual_machine")?,
            cargo: string(host, "cargo")?,
            rustc: string(host, "rustc")?,
        },
        start: read_side(value.field("start")?)?,
        end: read_side(value.field("end")?)?,
        command: strings(&value, "command")?,
        exit: read_exit(&value, "exit")?,
        expected: value
            .field("expected")?
            .array()?
            .iter()
            .map(read_target)
            .collect::<Result<_, _>>()?,
        binaries,
        explicit,
        driver: read_driver(value.field("driver")?)?,
        problems: strings(&value, "problems")?,
    })
}

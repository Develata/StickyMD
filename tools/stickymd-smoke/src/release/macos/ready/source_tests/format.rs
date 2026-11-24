//! The receipt as JSON (schema 2): written whole by the runner, read whole and strictly by the
//! gate. Every object has exactly its fields; a missing, extra or mistyped one rejects the file.
//! Schema 1 (counts only, no test names) is refused, not migrated.
//! plan_ref: docs/plan/11_testing_and_release.md#macos-readiness-check

use super::{Case, CaseOutcome, Counts, Explicit, Host, Receipt, Run, Side, Target};
use crate::evidence::escape_json;
use crate::release::json::{self, Value};

fn text(value: &str) -> String {
    format!("\"{}\"", escape_json(value))
}

fn list(values: &[String]) -> String {
    let items: Vec<String> = values.iter().map(|value| text(value)).collect();
    format!("[{}]", items.join(","))
}

fn code(exit: Option<i32>) -> String {
    exit.map_or_else(|| "null".to_owned(), |code| code.to_string())
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
                "{{\"item\":{},\"command\":{},\"tests\":{},\"target\":{},\"counts\":{},\"cases\":{},\"exit_code\":{},\"temporary_volume\":{}}}",
                text(&entry.item),
                list(&entry.command),
                list(&entry.tests),
                optional(entry.target.as_ref(), target),
                optional(entry.counts.as_ref(), counts),
                cases(&entry.cases),
                code(entry.exit_code),
                optional(entry.temporary_volume.as_ref(), |v: &String| text(v)),
            )
        })
        .collect();
    format!(
        "{{\"schema_version\":2,\"kind\":\"macos-release-source-tests\",\"release_source\":{},\"host\":{{\"product_name\":{},\"product_version\":{},\"build_version\":{},\"arch\":{},\"virtual_machine\":{},\"cargo\":{},\"rustc\":{}}},\"start\":{},\"end\":{},\"command\":{},\"exit_code\":{},\"expected\":[{}],\"binaries\":[{}],\"explicit\":[{}],\"problems\":{}}}\n",
        text(&receipt.release_source),
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
        code(receipt.exit_code),
        expected.join(","),
        binaries.join(","),
        explicit.join(","),
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

fn exit(value: &Value, key: &str) -> Result<Option<i32>, String> {
    match value.field(key)? {
        Value::Null => Ok(None),
        Value::Number(number) => number
            .parse()
            .map(Some)
            .map_err(|_| format!("{key} is not an exit code")),
        _ => Err(format!("{key} is not an exit code")),
    }
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
            "host",
            "start",
            "end",
            "command",
            "exit_code",
            "expected",
            "binaries",
            "explicit",
            "problems",
        ],
    )?;
    if value.field("schema_version")?.unsigned()? != 2
        || string(&value, "kind")? != "macos-release-source-tests"
    {
        return Err("not a schema 2 macOS source-test receipt".to_owned());
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
                    "exit_code",
                    "temporary_volume",
                ],
            )?;
            Ok(Explicit {
                item: string(entry, "item")?,
                command: strings(entry, "command")?,
                tests: strings(entry, "tests")?,
                target: nullable(entry, "target", read_target)?,
                counts: nullable(entry, "counts", read_counts)?,
                cases: read_cases(entry.field("cases")?)?,
                exit_code: exit(entry, "exit_code")?,
                temporary_volume: nullable(entry, "temporary_volume", |v| {
                    v.string().map(str::to_owned)
                })?,
            })
        })
        .collect::<Result<_, String>>()?;
    Ok(Receipt {
        release_source: string(&value, "release_source")?,
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
        exit_code: exit(&value, "exit_code")?,
        expected: value
            .field("expected")?
            .array()?
            .iter()
            .map(read_target)
            .collect::<Result<_, _>>()?,
        binaries,
        explicit,
        problems: strings(&value, "problems")?,
    })
}

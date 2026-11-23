//! The receipt as JSON (schema 1): written whole by the runner, read whole and strictly by the
//! gate. Every object has exactly its fields; a missing, extra or mistyped one rejects the file.
//! plan_ref: docs/plan/11_testing_and_release.md#macos-readiness-check

use super::{Counts, Host, Receipt, Run, SameKey, Side, Target};
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

fn side(side: &Side) -> String {
    format!("{{\"head\":{},\"clean\":{}}}", text(&side.head), side.clean)
}

pub(in super::super) fn render(receipt: &Receipt) -> String {
    let host = &receipt.host;
    let expected: Vec<String> = receipt.expected.iter().map(target).collect();
    let binaries: Vec<String> = receipt
        .binaries
        .iter()
        .map(|run| {
            format!(
                "{{\"target\":{},\"counts\":{}}}",
                target(&run.target),
                counts(&run.counts)
            )
        })
        .collect();
    let same = &receipt.same_key;
    format!(
        "{{\"schema_version\":1,\"kind\":\"macos-release-source-tests\",\"release_source\":{},\"host\":{{\"product_name\":{},\"product_version\":{},\"build_version\":{},\"arch\":{},\"virtual_machine\":{},\"cargo\":{},\"rustc\":{}}},\"start\":{},\"end\":{},\"command\":{},\"exit_code\":{},\"expected\":[{}],\"binaries\":[{}],\"same_key\":{{\"command\":{},\"tests\":{},\"counts\":{},\"exit_code\":{}}},\"problems\":{}}}\n",
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
        list(&same.command),
        list(&same.tests),
        same.counts
            .as_ref()
            .map_or_else(|| "null".to_owned(), counts),
        code(same.exit_code),
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

fn read_side(value: &Value) -> Result<Side, String> {
    exactly(value, &["head", "clean"])?;
    Ok(Side {
        head: string(value, "head")?,
        clean: boolean(value, "clean")?,
    })
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
            "same_key",
            "problems",
        ],
    )?;
    if value.field("schema_version")?.unsigned()? != 1
        || string(&value, "kind")? != "macos-release-source-tests"
    {
        return Err("not a schema 1 macOS source-test receipt".to_owned());
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
    let same = value.field("same_key")?;
    exactly(same, &["command", "tests", "counts", "exit_code"])?;
    let binaries = value
        .field("binaries")?
        .array()?
        .iter()
        .map(|run| {
            exactly(run, &["target", "counts"])?;
            Ok(Run {
                target: read_target(run.field("target")?)?,
                counts: read_counts(run.field("counts")?)?,
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
        same_key: SameKey {
            command: strings(same, "command")?,
            tests: strings(same, "tests")?,
            counts: match same.field("counts")? {
                Value::Null => None,
                counts => Some(read_counts(counts)?),
            },
            exit_code: exit(same, "exit_code")?,
        },
        problems: strings(&value, "problems")?,
    })
}

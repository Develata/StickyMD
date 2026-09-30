//! Compatibility routing for all retained phase PowerShell interfaces.
//! plan_ref: docs/plan/11_testing_and_release.md#phase-verification-harness

mod parameters;
#[cfg(test)]
mod tests;

use crate::cli::{CommandLine, Phase};
use parameters::Parameters;

pub(crate) fn parse(arguments: &[String], plan: bool) -> Result<CommandLine, String> {
    let routed = route(arguments)?;
    let command = CommandLine::parse(routed.clone())?;
    if plan {
        Ok(CommandLine::PhaseEntryPlan(routed))
    } else {
        Ok(command)
    }
}

pub(crate) fn print_plan(arguments: &[String]) {
    let arguments = arguments
        .iter()
        .map(|value| format!("\"{}\"", crate::evidence::escape_json(value)))
        .collect::<Vec<_>>()
        .join(",");
    println!("{{\"schema_version\":1,\"status\":\"NOT_RUN\",\"arguments\":[{arguments}]}}");
}

fn route(arguments: &[String]) -> Result<Vec<String>, String> {
    let p = Parameters::parse(arguments)?;
    for name in ["g3", "g4", "g5"] {
        for suffix in ["zip", "case"] {
            if p.active(&format!("{name}{suffix}")) && !p.active(name) {
                return Err(format!("{name}{suffix} requires {name}"));
            }
        }
    }
    let actions = [
        "environment",
        "campaign",
        "candidate",
        "sourcefreeze",
        "attribution",
        "windowstress",
        "decisionkey",
        "manual",
        "manualsession",
        "guided",
        "guidedsession",
        "g3",
        "g4",
        "g5",
        "manuallist",
        "manualstatus",
        "readiness",
        "remoterunid",
        "downloadedzip",
    ];
    let selected = actions
        .into_iter()
        .filter(|name| p.active(name))
        .collect::<Vec<_>>();
    if selected.len() > 1 {
        return Err("Select at most one qualification action".into());
    }
    for (name, label, needs_resume) in [
        ("resourceplan", "ResourcePlan", true),
        ("resourcefailurefirst", "ResourceFailureFirst", true),
        ("resourceresume", "ResourceResume", false),
    ] {
        if p.active(name)
            && (!p.active("resources")
                || (needs_resume && !p.active("resourceresume"))
                || !selected.is_empty())
        {
            return Err(format!(
                "{label} requires Resources{} and cannot run a qualification action",
                if needs_resume {
                    " and ResourceResume"
                } else {
                    ""
                }
            ));
        }
    }
    let mut result: Vec<String> = match selected.first().copied() {
        None => p.phase.map_or_else(
            || vec!["all".into()],
            |phase| vec!["phase".into(), phase.number().into()],
        ),
        Some("manual" | "manualsession") => {
            let mut args = vec!["acceptance".into(), "manual".into()];
            if p.phase != Some(Phase::P12) {
                args.push("run".into());
            }
            p.append(&mut args, "manualsession", "session");
            return Ok(args);
        }
        Some("guided" | "guidedsession") => {
            let mut args = vec!["acceptance".into(), "manual".into(), "guided".into()];
            p.append(&mut args, "guidedsession", "session");
            return Ok(args);
        }
        Some("manuallist" | "manualstatus") => {
            return Ok(vec![
                "acceptance".into(),
                "manual".into(),
                if p.active("manuallist") {
                    "list"
                } else {
                    "status"
                }
                .into(),
            ]);
        }
        Some("windowstress") => return Ok(p.window_stress()),
        Some(name) => vec![
            "qualification".into(),
            match name {
                "campaign" => "local",
                // Retain the old candidate spelling; the canonical parser still refuses it.
                "candidate" => "candidate",
                "sourcefreeze" => "source-freeze",
                "decisionkey" => "decision",
                "remoterunid" => "remote",
                "downloadedzip" => "downloaded",
                other => other,
            }
            .into(),
        ],
    };
    match selected.first().copied() {
        Some("environment") => p.append(&mut result, "evidencefile", "evidence-file"),
        Some(name @ ("g3" | "g4" | "g5")) => {
            p.append(&mut result, &format!("{name}zip"), "zip");
            p.append(&mut result, "evidencefile", "evidence-file");
            p.append(&mut result, &format!("{name}case"), "case");
        }
        Some("decisionkey") => {
            if !p.active("decisionstatus") || !p.active("decisionevidence") {
                return Err(
                    "DecisionStatus and DecisionEvidence are required with DecisionKey".into(),
                );
            }
            for (name, flag) in [
                ("decisionkey", "key"),
                ("decisionstatus", "status"),
                ("decisionevidence", "evidence"),
            ] {
                p.append(&mut result, name, flag);
            }
        }
        Some("readiness") => p.flag(&mut result, "explain", "explain"),
        Some("remoterunid") => {
            if !p.active("remoteattempt") {
                return Err("RemoteAttempt is required with RemoteRunId".into());
            }
            p.append(&mut result, "remoterunid", "run-id");
            p.append(&mut result, "remoteattempt", "attempt");
        }
        Some("downloadedzip") => p.append(&mut result, "downloadedzip", "zip"),
        None => {
            p.flag(&mut result, "ci", "ci");
            p.append(&mut result, "cishard", "ci-shard");
            for flag in ["performance", "runtime", "resources"] {
                p.flag(&mut result, flag, flag);
            }
            p.append(&mut result, "resourcemodule", "resource-module");
            for (name, flag) in [
                ("resourceresume", "resource-resume"),
                ("resourceplan", "resource-plan"),
                ("resourcefailurefirst", "resource-failure-first"),
            ] {
                p.flag(&mut result, name, flag);
            }
            for flag in ["release", "package", "json"] {
                p.flag(&mut result, flag, flag);
            }
            p.append(&mut result, "evidencefile", "evidence-file");
        }
        _ => {}
    }
    Ok(result)
}

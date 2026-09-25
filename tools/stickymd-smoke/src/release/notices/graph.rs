//! Cargo normal-edge reachability; build/dev edges never authorize runtime notices.
//! plan_ref: docs/plan/11_testing_and_release.md#phase-verification-harness

use super::super::json::{self, Value};
use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    path::PathBuf,
};

#[derive(Debug)]
pub(super) struct Package {
    pub id: String,
    pub name: String,
    pub version: String,
    pub source: Option<String>,
    pub license: Option<String>,
    pub repository: Option<String>,
    pub homepage: Option<String>,
    pub manifest: PathBuf,
}

pub(super) fn runtime_packages(metadata: &str) -> Result<Vec<Package>, String> {
    let metadata = json::parse(metadata)?;
    let mut packages = BTreeMap::new();
    for package in metadata.field("packages")?.array()? {
        let string = |key| package.field(key)?.string().map(str::to_owned);
        let optional = |key| {
            package
                .field(key)?
                .optional_string()
                .map(|value| value.map(str::to_owned))
        };
        let package = Package {
            id: string("id")?,
            name: string("name")?,
            version: string("version")?,
            source: optional("source")?,
            license: optional("license")?,
            repository: optional("repository")?,
            homepage: optional("homepage")?,
            manifest: PathBuf::from(string("manifest_path")?),
        };
        if packages.insert(package.id.clone(), package).is_some() {
            return Err("duplicate Cargo package ID".to_owned());
        }
    }
    let roots = packages
        .values()
        .filter(|package| package.name == "stickymd-win")
        .map(|package| package.id.clone())
        .collect::<Vec<_>>();
    if roots.len() != 1 {
        return Err("Cannot identify the stickymd-win package in cargo metadata".to_owned());
    }
    let mut nodes = BTreeMap::new();
    for node in metadata.field("resolve")?.field("nodes")?.array()? {
        let id = node.field("id")?.string()?;
        if nodes.insert(id, node).is_some() {
            return Err(format!("duplicate Cargo resolve node {id}"));
        }
    }
    let mut visited = BTreeSet::new();
    let mut pending = VecDeque::from(roots);
    while let Some(id) = pending.pop_front() {
        if !visited.insert(id.clone()) {
            continue;
        }
        if !packages.contains_key(&id) {
            return Err(format!("missing Cargo package {id}"));
        }
        let node = nodes
            .get(id.as_str())
            .ok_or_else(|| format!("missing Cargo resolve node {id}"))?;
        for dependency in node.field("deps")?.array()? {
            let mut runtime = false;
            let kinds = dependency.field("dep_kinds")?.array()?;
            if kinds.is_empty() {
                return Err("missing Cargo dependency classification".to_owned());
            }
            for kind in kinds {
                match kind.field("kind")? {
                    Value::Null => runtime = true,
                    Value::String(value) if matches!(value.as_str(), "dev" | "build") => {}
                    _ => return Err("unknown Cargo dependency classification".to_owned()),
                }
            }
            if runtime {
                pending.push_back(dependency.field("pkg")?.string()?.to_owned());
            }
        }
    }
    let mut runtime = packages
        .into_values()
        .filter(|package| visited.contains(&package.id) && package.source.is_some())
        .collect::<Vec<_>>();
    // Match PowerShell StringComparer.Ordinal, including non-BMP text in fixture paths/names.
    runtime.sort_by(|left, right| {
        ordinal(&left.name, &right.name)
            .then_with(|| ordinal(&left.version, &right.version))
            .then_with(|| ordinal(&left.id, &right.id))
    });
    for package in &runtime {
        if !package
            .source
            .as_deref()
            .is_some_and(|source| source.starts_with("registry+"))
        {
            return Err(format!(
                "Runtime dependency notice generation does not support non-registry sources: {} {}: {}",
                package.name,
                package.version,
                package.source.as_deref().unwrap_or_default()
            ));
        }
    }
    if runtime.is_empty() {
        return Err("The Windows runtime dependency graph is empty".to_owned());
    }
    Ok(runtime)
}

pub(super) fn ordinal(left: &str, right: &str) -> std::cmp::Ordering {
    left.encode_utf16().cmp(right.encode_utf16())
}

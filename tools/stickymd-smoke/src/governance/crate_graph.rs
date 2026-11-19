//! The shared crates' dependency boundary, read from `cargo metadata` (plan 03
//! `#shared-crates`) rather than from manifest text, so a renamed dependency
//! (`ui = { package = "winit" }`), a `[target.'cfg(...)'.dependencies]` table and a crate
//! reached through another crate are all seen.
//!
//! The input is `cargo metadata --format-version 1 --locked --all-features` with no platform
//! filter. Packages are named through `packages[]` (a declaration's `name` is the package, its
//! `rename` the alias; a resolved edge's `pkg` is an ID), never by parsing an ID. Three checks per
//! shared crate:
//!
//! - **Declarations** (`packages[].dependencies[]`, every table: normal, build, dev, any target,
//!   optional or not): no listed platform crate is declared, as the manifest scan this replaces
//!   refused. A path dependency of any kind must be a workspace member, matched by directory, and
//!   a member's name declared without a path is refused; a normal or build one must also be a
//!   member the layering allows. Cargo keeps declarations of crates without a library (the hosts,
//!   stickymd-smoke) out of the resolved graph, so only this check sees them.
//! - **Resolved local edges**: an edge of any kind to a package without a registry or git source
//!   must be a workspace member, matched by ID (a same-name package outside the workspace is
//!   refused); a normal or build one must also be allowed. Only the layering exempts dev edges:
//!   test builds may reach across.
//! - **Platform reach**: no crate on the capability lists may be reachable through normal edges,
//!   direct or transitive. Dev and build edges are not followed: neither enters the crate's
//!   runtime. Third-party crates may hold platform backends (plan 03 rule 8), so bindings
//!   (`windows`; `raw-window-handle` below stickymd-desktop) are refused only as declarations,
//!   and `libc`, `windows-sys` and `objc2` not at all. Whether a backend is approved, and safe
//!   native calls through a binding the lists do not name, stay with review.
//!
//! The resolved graph unifies every workspace member's features (with `--all-features`, every
//! feature of every member) and every target, so the reach check may over-report a path that the
//! crate alone never builds, such as a feature only a host enables or targets that cannot hold
//! together. Given that graph, normal edges and the lists, it does not under-report. Its message
//! names the per-target `cargo tree` that shows the crate's own path; an over-report still fails.
//!
//! A dependency entry counts as normal if any of its kinds is normal; an empty or unknown kind
//! list fails, as does a missing shared crate or a missing resolve node on a walked path.
//!
//! plan_ref: docs/plan/03_system_architecture.md#shared-crates

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::path::Path;
use std::process::Command;

use crate::release::json::{self, Value};

use rules::{BOUNDARIES, Boundary, OUTSIDE, allowed_text, local_route, matches};

pub(super) fn verify(root: &Path) -> Result<(), String> {
    let output = Command::new("cargo")
        .args([
            "metadata",
            "--format-version",
            "1",
            "--locked",
            "--all-features",
        ])
        .current_dir(root)
        .output()
        .map_err(|error| format!("cannot start `cargo metadata`: {error}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let lines = stderr.lines().collect::<Vec<_>>();
        // Cargo's own error is usually last; a verbose log keeps its head and its tail.
        let reason = if lines.len() <= 40 {
            lines.join("\n")
        } else {
            format!(
                "{}\n[{} lines omitted]\n{}",
                lines[..10].join("\n"),
                lines.len() - 40,
                lines[lines.len() - 30..].join("\n")
            )
        };
        return Err(format!(
            "`cargo metadata --locked --all-features` failed ({}):\n{reason}",
            output.status
        ));
    }
    let text = String::from_utf8(output.stdout)
        .map_err(|error| format!("`cargo metadata` output is not UTF-8: {error}"))?;
    verify_metadata(&text)
}

fn verify_metadata(text: &str) -> Result<(), String> {
    let metadata = json::parse(text)?;
    let graph = Graph::read(&metadata)?;
    for boundary in BOUNDARIES {
        graph.check(boundary)?;
    }
    Ok(())
}

struct Edge<'a> {
    package: &'a str,
    /// The extern name the depending crate uses (`deps[].name`).
    alias: &'a str,
    normal: bool,
    build: bool,
    /// Targets of the normal kinds; empty when one of them is unconditional.
    normal_targets: Vec<&'a str>,
    /// Targets of the normal and build kinds; empty when one of them is unconditional.
    linked_targets: Vec<&'a str>,
}

struct Declaration<'a> {
    name: &'a str,
    rename: Option<&'a str>,
    /// `None` for a normal dependency, else `build` or `dev`.
    kind: Option<&'a str>,
    target: Option<&'a str>,
    path: Option<&'a str>,
}

struct Package<'a> {
    name: &'a str,
    version: &'a str,
    local: bool,
    value: &'a Value,
}

struct Graph<'a> {
    packages: BTreeMap<&'a str, Package<'a>>,
    /// Workspace member name → ID.
    members: BTreeMap<&'a str, &'a str>,
    /// Workspace member manifest directory → name.
    member_directories: BTreeMap<&'a Path, &'a str>,
    edges: BTreeMap<&'a str, Vec<Edge<'a>>>,
}

impl<'a> Graph<'a> {
    fn read(metadata: &'a Value) -> Result<Self, String> {
        let mut packages = BTreeMap::new();
        for value in metadata.field("packages")?.array()? {
            let id = value.field("id")?.string()?;
            let name = value.field("name")?.string()?;
            let version = value.field("version")?.string()?;
            let local = value.field("source")?.optional_string()?.is_none();
            let package = Package {
                name,
                version,
                local,
                value,
            };
            if packages.insert(id, package).is_some() {
                return Err(format!("duplicate Cargo package {id}"));
            }
        }
        let mut members = BTreeMap::new();
        let mut member_directories = BTreeMap::new();
        for member in metadata.field("workspace_members")?.array()? {
            let id = member.string()?;
            let package = packages
                .get(id)
                .ok_or_else(|| format!("workspace member {id} has no Cargo package"))?;
            if members.insert(package.name, id).is_some() {
                return Err(format!("duplicate workspace member {}", package.name));
            }
            let manifest = Path::new(package.value.field("manifest_path")?.string()?);
            let directory = manifest
                .parent()
                .ok_or_else(|| format!("{} has no manifest directory", package.name))?;
            member_directories.insert(directory, package.name);
        }
        let resolve = metadata.field("resolve")?;
        if *resolve == Value::Null {
            return Err("cargo metadata has no resolved graph".to_owned());
        }
        let mut edges = BTreeMap::new();
        for node in resolve.field("nodes")?.array()? {
            let id = node.field("id")?.string()?;
            if !packages.contains_key(id) {
                return Err(format!("Cargo resolve node {id} has no package"));
            }
            let mut deps = Vec::new();
            for dependency in node.field("deps")?.array()? {
                let edge = read_edge(dependency)?;
                if !packages.contains_key(edge.package) {
                    return Err(format!(
                        "{id} depends on {} that has no package",
                        edge.package
                    ));
                }
                deps.push(edge);
            }
            if edges.insert(id, deps).is_some() {
                return Err(format!("duplicate Cargo resolve node {id}"));
            }
        }
        Ok(Self {
            packages,
            members,
            member_directories,
            edges,
        })
    }

    fn name(&self, id: &str) -> &'a str {
        self.packages[id].name
    }

    fn deps(&self, id: &str) -> Result<&[Edge<'a>], String> {
        self.edges
            .get(id)
            .map(Vec::as_slice)
            .ok_or_else(|| format!("missing Cargo resolve node for {}", self.name(id)))
    }

    fn check(&self, boundary: &Boundary) -> Result<(), String> {
        let id = *self.members.get(boundary.name).ok_or_else(|| {
            format!(
                "shared crate {} is not a workspace member; a check of nothing proves nothing",
                boundary.name
            )
        })?;
        self.check_declarations(boundary, id)?;
        self.check_local_edges(boundary, id)?;
        self.check_reach(boundary, id)
    }

    fn check_declarations(&self, boundary: &Boundary, id: &str) -> Result<(), String> {
        for value in self.packages[id].value.field("dependencies")?.array()? {
            let declaration = read_declaration(value)?;
            let hop = label(
                declaration.name,
                declaration.rename,
                declaration.kind,
                declaration.target.as_slice(),
            );
            let banned = boundary
                .unreachable
                .iter()
                .chain(boundary.undeclared)
                .flat_map(|group| group.iter())
                .find(|banned| matches(banned.pattern, declaration.name));
            if let Some(banned) = banned {
                return Err(format!(
                    "{} declares {} in its manifest ({} -> {hop}); {} (plan 03 #shared-crates)",
                    boundary.name, declaration.name, boundary.name, banned.route
                ));
            }
            // Identity holds for every kind: a local dependency is the workspace member itself.
            let member = match declaration.path {
                Some(path) => *self
                    .member_directories
                    .get(Path::new(path))
                    .ok_or_else(|| {
                        format!(
                            "{} declares {} from outside the workspace ({} -> {hop}, path {path}); \
                         {OUTSIDE}",
                            boundary.name, declaration.name, boundary.name
                        )
                    })?,
                None if self.members.contains_key(declaration.name) => {
                    return Err(format!(
                        "{} declares {} without a path to the workspace member ({} -> {hop}); \
                         declare a workspace member by path, so it is the member plan 03 layers",
                        boundary.name, declaration.name, boundary.name
                    ));
                }
                None => continue,
            };
            // Test builds may cross the layering.
            if declaration.kind == Some("dev") || boundary.permits(member) {
                continue;
            }
            return Err(format!(
                "{} declares {member} ({} -> {hop}); plan 03 #shared-crates lets {} depend {}. {}",
                boundary.name,
                boundary.name,
                boundary.name,
                allowed_text(boundary),
                local_route(member)
            ));
        }
        Ok(())
    }

    fn check_local_edges(&self, boundary: &Boundary, id: &str) -> Result<(), String> {
        for edge in self.deps(id)? {
            let target = &self.packages[edge.package];
            if !target.local {
                continue;
            }
            let hop = self.hop(edge, &edge.linked_targets);
            if self.members.get(target.name) != Some(&edge.package) {
                return Err(format!(
                    "{} depends on {} from outside the workspace ({} -> {hop}); {OUTSIDE}",
                    boundary.name, target.name, boundary.name
                ));
            }
            // Test builds may cross the layering.
            if !(edge.normal || edge.build) || boundary.permits(target.name) {
                continue;
            }
            return Err(format!(
                "{} depends on {} ({} -> {hop}); plan 03 #shared-crates lets {} depend {}. {}",
                boundary.name,
                target.name,
                boundary.name,
                boundary.name,
                allowed_text(boundary),
                local_route(target.name)
            ));
        }
        Ok(())
    }

    /// Breadth-first over normal edges, so the reported path is a shortest one.
    fn check_reach(&self, boundary: &Boundary, root: &'a str) -> Result<(), String> {
        let mut reached: BTreeMap<&str, (&str, &Edge)> = BTreeMap::new();
        let mut visited = BTreeSet::from([root]);
        let mut pending = VecDeque::from([root]);
        while let Some(id) = pending.pop_front() {
            for edge in self.deps(id)?.iter().filter(|edge| edge.normal) {
                if !visited.insert(edge.package) {
                    continue;
                }
                reached.insert(edge.package, (id, edge));
                let name = self.name(edge.package);
                let banned = boundary
                    .unreachable
                    .iter()
                    .flat_map(|group| group.iter())
                    .find(|banned| matches(banned.pattern, name));
                if let Some(banned) = banned {
                    return Err(format!(
                        "{crate_name} reaches {name} through normal dependencies ({path}); \
                         {route} (plan 03 #shared-crates). The graph unifies every member's \
                         features and targets and may over-report: run `cargo tree -p \
                         {crate_name} -e normal --all-features --target <triple> -i \
                         {name}@{version}` for each supported target; an over-report still fails \
                         this check and goes to the USER",
                        crate_name = boundary.name,
                        path = self.path(&reached, root, edge.package),
                        route = banned.route,
                        version = self.packages[edge.package].version
                    ));
                }
                pending.push_back(edge.package);
            }
        }
        Ok(())
    }

    fn path(&self, reached: &BTreeMap<&str, (&str, &Edge)>, root: &str, end: &str) -> String {
        let mut hops = Vec::new();
        let mut current = end;
        while current != root {
            let (parent, edge) = reached[current];
            hops.push(self.hop(edge, &edge.normal_targets));
            current = parent;
        }
        hops.push(self.name(root).to_owned());
        hops.reverse();
        hops.join(" -> ")
    }

    fn hop(&self, edge: &Edge, targets: &[&str]) -> String {
        let name = self.name(edge.package);
        let rename = (edge.alias != name.replace('-', "_")).then_some(edge.alias);
        let kind = match (edge.normal, edge.build) {
            (true, _) => None,
            (false, true) => Some("build"),
            (false, false) => Some("dev"),
        };
        label(name, rename, kind, targets)
    }
}

/// A dependency's package name with its alias when renamed, its kind when not normal, and its
/// targets when every counted kind is conditional.
fn label(name: &str, rename: Option<&str>, kind: Option<&str>, targets: &[&str]) -> String {
    let notes = rename
        .map(|alias| format!("as `{alias}`"))
        .into_iter()
        .chain(kind.map(str::to_owned))
        .chain(targets.iter().map(|target| (*target).to_owned()))
        .collect::<Vec<_>>();
    if notes.is_empty() {
        name.to_owned()
    } else {
        format!("{name} [{}]", notes.join(", "))
    }
}

fn read_declaration(value: &Value) -> Result<Declaration<'_>, String> {
    let name = value.field("name")?.string()?;
    let kind = value.field("kind")?.optional_string()?;
    if !matches!(kind, None | Some("build" | "dev")) {
        return Err(format!("{name} has an unknown Cargo dependency kind"));
    }
    Ok(Declaration {
        name,
        rename: value.field("rename")?.optional_string()?,
        kind,
        target: value.field("target")?.optional_string()?,
        // Cargo writes `path` only for a path dependency.
        path: match value {
            Value::Object(fields) => fields.get("path").map(Value::string).transpose()?,
            _ => None,
        },
    })
}

fn read_edge(dependency: &Value) -> Result<Edge<'_>, String> {
    let package = dependency.field("pkg")?.string()?;
    let alias = dependency.field("name")?.string()?;
    let kinds = dependency.field("dep_kinds")?.array()?;
    if kinds.is_empty() {
        return Err(format!("{package} has no Cargo dependency kind"));
    }
    let (mut normal, mut build) = (false, false);
    let (mut normal_targets, mut linked_targets) = (Some(Vec::new()), Some(Vec::new()));
    for kind in kinds {
        let target = kind.field("target")?.optional_string()?;
        let (is_normal, is_build) = match kind.field("kind")? {
            Value::Null => (true, false),
            Value::String(value) if value == "build" => (false, true),
            Value::String(value) if value == "dev" => (false, false),
            _ => return Err(format!("{package} has an unknown Cargo dependency kind")),
        };
        if is_normal {
            record(&mut normal_targets, target);
        }
        if is_normal || is_build {
            record(&mut linked_targets, target);
        }
        normal |= is_normal;
        build |= is_build;
    }
    Ok(Edge {
        package,
        alias,
        normal,
        build,
        normal_targets: normal_targets.unwrap_or_default(),
        linked_targets: linked_targets.unwrap_or_default(),
    })
}

/// Adds one counted kind's target; the list becomes `None` once a counted kind applies on every
/// target.
fn record<'a>(targets: &mut Option<Vec<&'a str>>, target: Option<&'a str>) {
    match (target, targets) {
        (None, targets) => *targets = None,
        (Some(target), Some(list)) => list.push(target),
        (Some(_), None) => {}
    }
}

mod rules;
#[cfg(test)]
mod tests;

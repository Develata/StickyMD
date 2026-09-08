//! Compare declared engineering dependencies with Cargo's direct dependency facts.
//! plan_ref: docs/plan/11_testing_and_release.md#modular-headless-ci

use crate::{headless::Module, repository::command_text};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
};

pub(super) fn verify(root: &Path) -> Result<(), String> {
    let mut observed = BTreeMap::new();
    for manifest in [
        None,
        Module::MarkdownMath.manifest(),
        Module::Persistence.manifest(),
    ] {
        let mut args = vec![
            "tree",
            "--depth",
            "1",
            "--prefix",
            "depth",
            "--format",
            "{p}",
            "--edges",
            "normal,build,dev",
            "--target",
            "all",
            "--locked",
            "--color",
            "never",
        ];
        if let Some(manifest) = manifest {
            args.extend(["--manifest-path", manifest]);
        } else {
            args.push("--workspace");
        }
        observe(&command_text(root, "cargo", &args)?, &mut observed)?;
    }
    let expected: BTreeMap<_, BTreeSet<_>> = Module::ALL
        .into_iter()
        .map(|module| (module, module.dependencies().iter().copied().collect()))
        .collect();
    if observed != expected {
        return Err(format!(
            "dependency mapping differs: observed={observed:?}, expected={expected:?}"
        ));
    }
    Ok(())
}

fn observe(listing: &str, graph: &mut BTreeMap<Module, BTreeSet<Module>>) -> Result<(), String> {
    let mut parent = None;
    for line in listing.lines().filter(|line| !line.trim().is_empty()) {
        let token = line
            .split_whitespace()
            .next()
            .ok_or("empty Cargo tree line")?;
        let (depth, name) = token
            .split_at_checked(1)
            .ok_or("missing Cargo tree depth")?;
        let module = Module::ALL
            .into_iter()
            .find(|module| module.cargo_name() == name);
        match depth {
            "0" => {
                let module = module.ok_or_else(|| format!("unregistered Cargo root {name}"))?;
                if graph.insert(module, BTreeSet::new()).is_some() {
                    return Err("duplicate Cargo root".to_owned());
                }
                parent = Some(module);
            }
            "1" => {
                let parent = parent.ok_or("Cargo dependency precedes its root")?;
                if let Some(module) = module {
                    graph.get_mut(&parent).unwrap().insert(module);
                }
            }
            _ => return Err("unexpected Cargo tree depth".to_owned()),
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ci_registry_tracks_only_internal_edges_and_rejects_unknown_roots() {
        let mut graph = BTreeMap::new();
        observe(
            "0stickymd-render v1 (path with spaces)\n1comrak v1\n1stickymd-core v1 (*)\n",
            &mut graph,
        )
        .unwrap();
        assert_eq!(graph[&Module::Render], BTreeSet::from([Module::Core]));
        assert!(observe("0new-crate v1\n", &mut graph).is_err());
        assert!(observe("0stickymd-render v1\n", &mut graph).is_err());
        assert!(observe("1stickymd-core v1\n", &mut BTreeMap::new()).is_err());
    }

    #[test]
    fn ci_registry_matches_current_cargo_workspace_and_experiments() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(Path::parent)
            .unwrap();
        verify(root).unwrap();
    }
}

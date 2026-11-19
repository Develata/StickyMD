use super::{BOUNDARIES, Graph, verify, verify_metadata};
use crate::release::json;

/// `(kind, target)` of one `dep_kinds` entry, and of one declaration.
type Kind = (Option<&'static str>, Option<&'static str>);

const NORMAL: &[Kind] = &[(None, None)];
const DEV: &[Kind] = &[(Some("dev"), None)];
const BUILD: &[Kind] = &[(Some("build"), None)];
const WINDOWS: &[Kind] = &[(None, Some("cfg(windows)"))];
const MACOS: &[Kind] = &[(None, Some("cfg(target_os = \"macos\")"))];

struct Package {
    key: &'static str,
    name: &'static str,
    local: bool,
    member: bool,
}

struct Dependency {
    from: &'static str,
    to: &'static str,
    alias: String,
    kinds: &'static [Kind],
    /// In the depending package's `dependencies[]`.
    declared: bool,
    /// In its resolve node's `deps[]`.
    resolved: bool,
}

/// A `cargo metadata` document. Package IDs are opaque (`pkg-N`) so nothing can pass by reading
/// a name out of an ID; packages are addressed by a key that is their name unless a test needs
/// two packages of one name.
struct Fixture {
    packages: Vec<Package>,
    dependencies: Vec<Dependency>,
    without_node: Option<&'static str>,
    /// A member whose `manifest_path` Cargo spells differently from the paths that name it.
    respelled: Option<&'static str>,
}

impl Fixture {
    /// The real layering in miniature, with third-party platform backends under winit and libc
    /// under sha2 as plan 03 rule 8 allows.
    fn allowed() -> Self {
        let mut fixture = Self {
            packages: Vec::new(),
            dependencies: Vec::new(),
            without_node: None,
            respelled: None,
        };
        for name in [
            "stickymd-core",
            "stickymd-render",
            "stickymd-app",
            "stickymd-desktop",
            "stickymd-posix",
            "stickymd-win",
            "stickymd-mac",
            "stickymd-smoke",
        ] {
            fixture = fixture.member(name);
        }
        let app_from_desktop: &[Kind] = &[(None, None), (Some("dev"), None)];
        let winit_from_desktop: &[Kind] = &[(None, None), (None, Some("cfg(unix)"))];
        for (from, to, kinds) in [
            ("stickymd-core", "sha2", NORMAL),
            ("sha2", "cpufeatures", NORMAL),
            ("cpufeatures", "libc", &[(None, Some("cfg(unix)"))][..]),
            ("stickymd-render", "stickymd-core", NORMAL),
            ("stickymd-render", "comrak", NORMAL),
            ("stickymd-render", "tiny-skia", NORMAL),
            ("stickymd-render", "walkdir", NORMAL),
            ("walkdir", "windows-sys", WINDOWS),
            ("stickymd-app", "stickymd-core", NORMAL),
            ("stickymd-app", "stickymd-render", NORMAL),
            ("stickymd-desktop", "stickymd-core", NORMAL),
            ("stickymd-desktop", "stickymd-render", NORMAL),
            ("stickymd-desktop", "stickymd-app", app_from_desktop),
            ("stickymd-desktop", "winit", winit_from_desktop),
            ("stickymd-desktop", "softbuffer", NORMAL),
            ("stickymd-desktop", "raw-window-handle", NORMAL),
            ("winit", "raw-window-handle", NORMAL),
            ("winit", "objc2", MACOS),
            ("winit", "windows-sys", WINDOWS),
            ("stickymd-posix", "libc", MACOS),
            ("stickymd-win", "stickymd-app", NORMAL),
            ("stickymd-win", "stickymd-desktop", WINDOWS),
            ("stickymd-win", "windows", WINDOWS),
            ("stickymd-win", "arboard", WINDOWS),
            ("stickymd-win", "tray-icon", WINDOWS),
            ("stickymd-win", "notify", WINDOWS),
            ("stickymd-mac", "stickymd-app", NORMAL),
            ("stickymd-mac", "stickymd-posix", MACOS),
            ("stickymd-mac", "stickymd-desktop", MACOS),
            ("stickymd-mac", "objc2", MACOS),
            ("stickymd-mac", "notify", MACOS),
        ] {
            fixture = fixture.with(from, to, &to.replace('-', "_"), kinds);
        }
        fixture
    }

    fn package(mut self, key: &'static str, name: &'static str, local: bool, member: bool) -> Self {
        self.packages.push(Package {
            key,
            name,
            local,
            member,
        });
        self
    }

    fn member(self, name: &'static str) -> Self {
        self.package(name, name, true, true)
    }

    fn edge(
        mut self,
        (from, to, alias): (&'static str, &'static str, &str),
        kinds: &'static [Kind],
        declared: bool,
        resolved: bool,
    ) -> Self {
        for key in [from, to] {
            if !self.packages.iter().any(|package| package.key == key) {
                self = self.package(key, key, false, false);
            }
        }
        self.dependencies.push(Dependency {
            from,
            to,
            alias: alias.to_owned(),
            kinds,
            declared,
            resolved,
        });
        self
    }

    /// Declared and resolved, creating either end as a registry package when it is new.
    fn with(
        self,
        from: &'static str,
        to: &'static str,
        alias: &str,
        kinds: &'static [Kind],
    ) -> Self {
        self.edge((from, to, alias), kinds, true, true)
    }

    /// Declared but absent from the resolved graph, as Cargo does for a crate without a library.
    fn declared_only(self, from: &'static str, to: &'static str, kinds: &'static [Kind]) -> Self {
        self.edge((from, to, &to.replace('-', "_")), kinds, true, false)
    }

    /// Resolved without a matching declaration, as a `[patch]` or a third party's own edge.
    fn resolved_only(self, from: &'static str, to: &'static str, kinds: &'static [Kind]) -> Self {
        self.edge((from, to, &to.replace('-', "_")), kinds, false, true)
    }

    fn without(mut self, key: &'static str) -> Self {
        self.packages.retain(|package| package.key != key);
        self.dependencies
            .retain(|dependency| dependency.from != key && dependency.to != key);
        self
    }

    fn without_node(mut self, key: &'static str) -> Self {
        self.without_node = Some(key);
        self
    }

    fn get(&self, key: &str) -> (usize, &Package) {
        self.packages
            .iter()
            .enumerate()
            .find(|(_, package)| package.key == key)
            .unwrap()
    }

    fn directory(&self, key: &str) -> String {
        let (_, package) = self.get(key);
        match (package.local, package.member) {
            (true, true) => format!("/repo/{}", package.name),
            (true, false) => format!("/outside/{key}"),
            (false, _) => format!("/registry/{key}"),
        }
    }

    fn json(&self) -> String {
        let text = |value: Option<&str>| {
            value.map_or("null".to_owned(), |value| {
                format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
            })
        };
        let registry = "registry+https://github.com/rust-lang/crates.io-index";
        let id = |key: &str| format!("pkg-{}", self.get(key).0);
        let packages = self
            .packages
            .iter()
            .map(|package| {
                let declarations = self
                    .dependencies
                    .iter()
                    .filter(|dependency| dependency.from == package.key && dependency.declared)
                    .flat_map(|dependency| {
                        let (_, to) = self.get(dependency.to);
                        let rename = (dependency.alias != to.name.replace('-', "_"))
                            .then_some(dependency.alias.as_str());
                        // Cargo writes `path` only for a path dependency.
                        let path = if to.local {
                            format!(r#","path":{}"#, text(Some(&self.directory(to.key))))
                        } else {
                            String::new()
                        };
                        dependency.kinds.iter().map(move |(kind, target)| {
                            format!(
                                r#"{{"name":"{}","source":{},"req":"*","kind":{},"rename":{},"optional":false,"uses_default_features":true,"features":[],"target":{},"registry":null{path}}}"#,
                                to.name,
                                text((!to.local).then_some(registry)),
                                text(*kind),
                                text(rename),
                                text(*target)
                            )
                        })
                    })
                    .collect::<Vec<_>>();
                format!(
                    r#"{{"id":"{}","name":"{}","version":"1.0.0","source":{},"manifest_path":"{}/Cargo.toml","dependencies":[{}]}}"#,
                    id(package.key),
                    package.name,
                    text((!package.local).then_some(registry)),
                    if self.respelled == Some(package.key) {
                        self.directory(package.key).replace("/repo/", "/REPO/")
                    } else {
                        self.directory(package.key)
                    },
                    declarations.join(",")
                )
            })
            .collect::<Vec<_>>();
        let members = self
            .packages
            .iter()
            .filter(|package| package.member)
            .map(|package| format!("\"{}\"", id(package.key)))
            .collect::<Vec<_>>();
        let nodes = self
            .packages
            .iter()
            .filter(|package| self.without_node != Some(package.key))
            .map(|package| {
                let deps = self
                    .dependencies
                    .iter()
                    .filter(|dependency| dependency.from == package.key && dependency.resolved)
                    .map(|dependency| {
                        let kinds = dependency
                            .kinds
                            .iter()
                            .map(|(kind, target)| {
                                format!(r#"{{"kind":{},"target":{}}}"#, text(*kind), text(*target))
                            })
                            .collect::<Vec<_>>();
                        format!(
                            r#"{{"name":"{}","pkg":"{}","dep_kinds":[{}]}}"#,
                            dependency.alias,
                            id(dependency.to),
                            kinds.join(",")
                        )
                    })
                    .collect::<Vec<_>>();
                format!(
                    r#"{{"id":"{}","deps":[{}]}}"#,
                    id(package.key),
                    deps.join(",")
                )
            })
            .collect::<Vec<_>>();
        format!(
            r#"{{"packages":[{}],"workspace_members":[{}],"resolve":{{"nodes":[{}]}}}}"#,
            packages.join(","),
            members.join(","),
            nodes.join(",")
        )
    }

    fn passes(&self) {
        verify_metadata(&self.json()).unwrap();
    }

    fn error(&self) -> String {
        verify_metadata(&self.json()).unwrap_err()
    }
}

fn assert_mentions(error: &str, parts: &[&str]) {
    for part in parts {
        assert!(error.contains(part), "`{part}` missing from: {error}");
    }
}

#[test]
fn an_allowed_graph_passes() {
    Fixture::allowed().passes();
    // Test builds may cross the layering, and the plan leaves stickymd-posix's use of the lower
    // crates open, so those edges are not refused.
    Fixture::allowed()
        .with("stickymd-core", "stickymd-render", "stickymd_render", DEV)
        .with("stickymd-posix", "stickymd-core", "stickymd_core", NORMAL)
        .with("stickymd-posix", "stickymd-app", "stickymd_app", NORMAL)
        .passes();
}

#[test]
fn a_renamed_dependency_is_caught() {
    let error = Fixture::allowed()
        .with("stickymd-app", "winit", "ui", NORMAL)
        .error();
    assert_mentions(
        &error,
        &[
            "stickymd-app declares winit",
            "stickymd-app -> winit [as `ui`]",
            "WindowHost port",
        ],
    );
    // Renamed deeper down, where only the resolved edge says what the package is.
    let error = Fixture::allowed()
        .with("stickymd-app", "helper", "helper", NORMAL)
        .with("helper", "winit", "ui", NORMAL)
        .error();
    assert_mentions(
        &error,
        &[
            "stickymd-app reaches winit",
            "stickymd-app -> helper -> winit [as `ui`]",
        ],
    );
    let error = Fixture::allowed()
        .with("stickymd-core", "stickymd-render", "projection", NORMAL)
        .error();
    assert_mentions(
        &error,
        &[
            "stickymd-core declares stickymd-render",
            "stickymd-core -> stickymd-render [as `projection`]",
            "lets stickymd-core depend on no local crate",
            "keep what both need in stickymd-core",
        ],
    );
}

#[test]
fn a_target_specific_dependency_is_caught() {
    let error = Fixture::allowed()
        .with("stickymd-render", "windows", "windows", WINDOWS)
        .error();
    assert_mentions(
        &error,
        &[
            "stickymd-render declares windows",
            "stickymd-render -> windows [cfg(windows)]",
            "apps/stickymd-win/src/platform/windows",
        ],
    );
    let error = Fixture::allowed()
        .with("stickymd-app", "helper", "helper", WINDOWS)
        .with("helper", "notify", "notify", NORMAL)
        .error();
    assert_mentions(
        &error,
        &[
            "stickymd-app reaches notify",
            "stickymd-app -> helper [cfg(windows)] -> notify",
            "Host::start_watch",
        ],
    );
    let error = Fixture::allowed()
        .with("stickymd-desktop", "stickymd-win", "stickymd_win", WINDOWS)
        .error();
    assert_mentions(
        &error,
        &[
            "stickymd-desktop declares stickymd-win",
            "stickymd-desktop -> stickymd-win [cfg(windows)]",
            "define a Capability Port in stickymd-app",
        ],
    );
}

#[test]
fn a_transitive_platform_crate_is_caught() {
    let error = Fixture::allowed()
        .with("cpufeatures", "notify", "notify", MACOS)
        .error();
    assert_mentions(
        &error,
        &[
            "stickymd-core reaches notify",
            "stickymd-core -> sha2 -> cpufeatures -> notify [cfg(target_os = \"macos\")]",
            "cargo tree -p stickymd-core -e normal --all-features --target <triple> -i notify@1.0.0",
        ],
    );
    // Reached through another workspace crate: stickymd-desktop's own check names the path.
    let fixture = Fixture::allowed().with("stickymd-app", "arboard", "arboard", NORMAL);
    let metadata = json::parse(&fixture.json()).unwrap();
    let graph = Graph::read(&metadata).unwrap();
    let desktop = BOUNDARIES
        .iter()
        .find(|boundary| boundary.name == "stickymd-desktop")
        .unwrap();
    let error = graph.check(desktop).unwrap_err();
    assert_mentions(
        &error,
        &[
            "stickymd-desktop reaches arboard",
            "stickymd-desktop -> stickymd-app -> arboard",
            "ClipboardPort",
        ],
    );
}

/// Plan 03 rule 8: a third-party backend may hold a binding a shared crate may not declare.
#[test]
fn bindings_may_sit_under_a_third_party_backend() {
    Fixture::allowed()
        .with("walkdir", "windows", "windows", WINDOWS)
        .with("walkdir", "raw-window-handle", "raw_window_handle", NORMAL)
        .passes();
    for (crate_name, binding) in [
        ("stickymd-core", "raw-window-handle"),
        ("stickymd-app", "raw-window-handle"),
        ("stickymd-desktop", "windows"),
    ] {
        let error = Fixture::allowed()
            .with(crate_name, binding, "x", NORMAL)
            .error();
        assert_mentions(&error, &[&format!("{crate_name} declares {binding}")]);
    }
}

#[test]
fn edge_kinds_decide_what_counts() {
    // A declaration in any table is refused, as the manifest scan refused it.
    for (kinds, note) in [(DEV, "[as `x`, dev]"), (BUILD, "[as `x`, build]")] {
        let error = Fixture::allowed()
            .with("stickymd-app", "winit", "x", kinds)
            .error();
        assert_mentions(&error, &["stickymd-app declares winit", note]);
    }
    // A resolved dependency that is both dev and normal is normal.
    let both: &[Kind] = &[(Some("dev"), None), (None, None)];
    let error = Fixture::allowed()
        .with("stickymd-app", "helper", "helper", NORMAL)
        .resolved_only("helper", "winit", both)
        .error();
    assert_mentions(&error, &["stickymd-app -> helper -> winit"]);
    // A build script does not enter the runtime, so reach does not follow it…
    Fixture::allowed()
        .with("stickymd-app", "helper", "helper", NORMAL)
        .resolved_only("helper", "winit", BUILD)
        .passes();
    // …but it does couple the layers.
    let error = Fixture::allowed()
        .with("stickymd-render", "stickymd-app", "stickymd_app", BUILD)
        .error();
    assert_mentions(
        &error,
        &["stickymd-render declares stickymd-app (stickymd-render -> stickymd-app [build])"],
    );
    let error = Fixture::allowed()
        .resolved_only("stickymd-render", "stickymd-app", BUILD)
        .error();
    assert_mentions(
        &error,
        &["stickymd-render depends on stickymd-app (stickymd-render -> stickymd-app [build])"],
    );
    for kinds in [&[][..], &[(Some("weird"), None)][..]] {
        let fixture = Fixture::allowed()
            .with("stickymd-app", "helper", "helper", NORMAL)
            .resolved_only("helper", "foo", kinds);
        assert!(verify_metadata(&fixture.json()).is_err(), "{kinds:?}");
    }
    // Neither a target nor `optional` hides a declaration.
    let conditional: &[Kind] = &[(Some("dev"), Some("cfg(windows)"))];
    let optional = Fixture::allowed()
        .declared_only("stickymd-app", "winit", conditional)
        .json()
        .replace(r#""optional":false"#, r#""optional":true"#);
    assert_mentions(
        &verify_metadata(&optional).unwrap_err(),
        &["stickymd-app declares winit", "[dev, cfg(windows)]"],
    );
    let unknown_declaration = Fixture::allowed()
        .declared_only("stickymd-app", "foo", &[(Some("weird"), None)])
        .error();
    assert_mentions(&unknown_declaration, &["unknown Cargo dependency kind"]);
}

/// The manifest cases the text scan covered, and the lists derived from the plan.
#[test]
fn every_listed_platform_crate_is_refused() {
    for (crate_name, banned) in [
        ("stickymd-app", "winit"),
        ("stickymd-app", "softbuffer"),
        ("stickymd-app", "raw-window-handle"),
        ("stickymd-app", "windows"),
        ("stickymd-app", "arboard"),
        ("stickymd-app", "tray-icon"),
        ("stickymd-app", "notify"),
        ("stickymd-desktop", "windows"),
        ("stickymd-desktop", "arboard"),
        ("stickymd-desktop", "tray-icon"),
        ("stickymd-desktop", "notify"),
        ("stickymd-core", "winit"),
        ("stickymd-core", "windows"),
        ("stickymd-core", "comrak"),
        ("stickymd-core", "ratex-parser"),
        ("stickymd-core", "cosmic-text"),
        ("stickymd-core", "tiny-skia"),
        ("stickymd-render", "softbuffer"),
        ("stickymd-render", "tray-icon"),
    ] {
        let error = Fixture::allowed()
            .with(crate_name, banned, "x", NORMAL)
            .error();
        assert_mentions(&error, &[&format!("{crate_name} declares {banned}")]);
        // The same crate one hop down is caught on the resolved graph, except a binding.
        let fixture = Fixture::allowed()
            .with(crate_name, "helper", "helper", NORMAL)
            .with("helper", banned, "x", NORMAL);
        if matches!(banned, "windows" | "raw-window-handle") {
            fixture.passes();
        } else {
            assert_mentions(
                &fixture.error(),
                &[&format!("{crate_name} reaches {banned}")],
            );
        }
    }
}

#[test]
fn local_crates_follow_the_layering() {
    for (from, to) in [
        ("stickymd-core", "stickymd-render"),
        ("stickymd-core", "stickymd-app"),
        ("stickymd-core", "stickymd-smoke"),
        ("stickymd-render", "stickymd-app"),
        ("stickymd-render", "stickymd-desktop"),
        ("stickymd-app", "stickymd-desktop"),
        ("stickymd-app", "stickymd-posix"),
        ("stickymd-desktop", "stickymd-mac"),
        ("stickymd-desktop", "stickymd-posix"),
        ("stickymd-posix", "stickymd-desktop"),
        ("stickymd-posix", "stickymd-mac"),
    ] {
        let error = Fixture::allowed().with(from, to, "x", NORMAL).error();
        assert_mentions(&error, &[&format!("{from} declares {to}")]);
        let error = Fixture::allowed().resolved_only(from, to, NORMAL).error();
        assert_mentions(&error, &[&format!("{from} depends on {to}")]);
    }
    let error = Fixture::allowed()
        .with("stickymd-posix", "stickymd-desktop", "x", NORMAL)
        .error();
    assert_mentions(
        &error,
        &[
            "lets stickymd-posix depend on no local crate (stickymd-core, stickymd-render, \
           stickymd-app undecided",
        ],
    );
    let error = Fixture::allowed()
        .member("stickymd-extra")
        .with("stickymd-app", "stickymd-extra", "stickymd_extra", NORMAL)
        .error();
    assert_mentions(
        &error,
        &[
            "stickymd-app declares stickymd-extra",
            "lets stickymd-app depend only on stickymd-core, stickymd-render",
            "add it to plan 03 first",
        ],
    );
}

/// Cargo leaves a dependency on a crate without a library out of the resolved graph; the hosts
/// and stickymd-smoke are such crates.
#[test]
fn a_declaration_without_a_resolved_edge_is_still_checked() {
    for (from, to, kinds) in [
        ("stickymd-core", "stickymd-smoke", NORMAL),
        ("stickymd-desktop", "stickymd-win", WINDOWS),
        ("stickymd-app", "stickymd-mac", BUILD),
    ] {
        let error = Fixture::allowed().declared_only(from, to, kinds).error();
        assert_mentions(&error, &[&format!("{from} declares {to}")]);
    }
}

/// A member directory spelled two ways is still the member when the resolved IDs agree; a copy
/// outside the workspace stays refused.
#[test]
fn a_respelled_member_directory_falls_back_to_the_resolved_id() {
    let mut fixture = Fixture::allowed();
    fixture.respelled = Some("stickymd-core");
    fixture.passes();
    let mut fixture = Fixture::allowed()
        .package("other-core", "stickymd-core", true, false)
        .with("stickymd-render", "other-core", "core_copy", NORMAL);
    fixture.respelled = Some("stickymd-core");
    assert_mentions(
        &fixture.error(),
        &["stickymd-render declares stickymd-core from outside the workspace"],
    );
    // A library-less member has no resolved edge to fall back on.
    let mut fixture = Fixture::allowed().declared_only("stickymd-core", "stickymd-smoke", DEV);
    fixture.respelled = Some("stickymd-smoke");
    assert_mentions(
        &fixture.error(),
        &["stickymd-core declares stickymd-smoke from outside the workspace"],
    );
}

/// An allowed name is not enough: the package must be the workspace member.
#[test]
fn a_same_name_crate_outside_the_workspace_is_refused() {
    let outside = || Fixture::allowed().package("other-core", "stickymd-core", true, false);
    let error = outside()
        .with("stickymd-render", "other-core", "core_copy", NORMAL)
        .error();
    assert_mentions(
        &error,
        &[
            "stickymd-render declares stickymd-core from outside the workspace",
            "stickymd-render -> stickymd-core [as `core_copy`]",
        ],
    );
    let error = outside()
        .resolved_only("stickymd-render", "other-core", NORMAL)
        .error();
    assert_mentions(
        &error,
        &["stickymd-render depends on stickymd-core from outside the workspace"],
    );
    let error = Fixture::allowed()
        .package("registry-core", "stickymd-core", false, false)
        .declared_only("stickymd-app", "registry-core", NORMAL)
        .error();
    assert_mentions(
        &error,
        &["stickymd-app declares stickymd-core without a path to the workspace member"],
    );
    // Identity holds for test builds too, while the layering does not.
    let error = outside()
        .with("stickymd-render", "other-core", "core_copy", DEV)
        .error();
    assert_mentions(
        &error,
        &["stickymd-render declares stickymd-core from outside the workspace"],
    );
    let error = outside()
        .resolved_only("stickymd-render", "other-core", DEV)
        .error();
    assert_mentions(
        &error,
        &[
            "stickymd-render depends on stickymd-core from outside the workspace (stickymd-render -> stickymd-core [as `other_core`, dev])",
        ],
    );
}

#[test]
fn incomplete_metadata_fails_closed() {
    for missing in ["stickymd-posix", "stickymd-core"] {
        let error = Fixture::allowed().without(missing).error();
        assert_mentions(&error, &[missing, "not a workspace member"]);
    }
    let error = Fixture::allowed().without_node("stickymd-app").error();
    assert_mentions(&error, &["missing Cargo resolve node for stickymd-app"]);
    // A transitive package without its node fails too, rather than ending the walk.
    let error = Fixture::allowed().without_node("cpufeatures").error();
    assert_mentions(&error, &["missing Cargo resolve node for cpufeatures"]);
    let no_resolve = Fixture::allowed().json().replace(
        r#""resolve":{"nodes":"#,
        r#""resolve":null,"unused":{"nodes":"#,
    );
    assert!(verify_metadata(&no_resolve).is_err());
    let dangling = Fixture::allowed()
        .json()
        .replace(r#""pkg":"pkg-0""#, r#""pkg":"pkg-999""#);
    assert!(verify_metadata(&dangling).is_err());
}

#[test]
fn the_real_workspace_passes() {
    let repository =
        crate::governance::find_repository_root(std::path::Path::new(env!("CARGO_MANIFEST_DIR")))
            .unwrap();
    verify(&repository).unwrap();
}

//! The rules of the shared crates' dependency boundary, derived from plan 03 `#shared-crates`:
//! which workspace members each shared crate may depend on, which crates it may not reach or
//! declare, and where each refused use belongs instead. `crate_graph` applies them.
//!
//! plan_ref: docs/plan/03_system_architecture.md#shared-crates

/// A crate a shared crate may not use, and where its use belongs instead.
pub(super) struct Banned {
    /// A package name, or a prefix when it ends in `*`.
    pub(super) pattern: &'static str,
    pub(super) route: &'static str,
}

const WINDOW: &str = "the window system belongs to stickymd-desktop and the Platform Hosts; \
                      stickymd-app asks for window actions through its WindowHost port, which \
                      the host implements, and the crates below it return results to \
                      stickymd-app";
const WIN32: &str = "Win32 calls belong to the Windows host's adapters \
                     (apps/stickymd-win/src/platform/windows), behind a port the host implements";
const POSIX: &str = "POSIX calls belong to stickymd-posix and the macOS host's adapters \
                     (apps/stickymd-mac/src/platform), behind a port the host implements";
const OBJC: &str = "Objective-C calls belong to the macOS host's adapters \
                    (apps/stickymd-mac/src/platform), behind a port the host implements";
const CLIPBOARD: &str = "the clipboard belongs to the Platform Host, which implements \
                         stickymd-app's ClipboardPort";
const TRAY: &str = "the tray belongs to the Platform Host, which implements stickymd-desktop's \
                    TrayHost; shared crates only build TrayView and read TrayCommand";
const WATCH: &str = "file watching belongs to the Platform Host, which implements \
                     stickymd-desktop's Host::start_watch; shared crates receive the changes as \
                     HostSignal events";
const RENDERING: &str = "rendering belongs to stickymd-render; stickymd-core keeps only domain \
                         state and pure computation";

/// The window system stickymd-app may not depend on (plan 03 table); every crate stickymd-app
/// depends on inherits that.
const WINDOW_SYSTEM: &[Banned] = &[
    Banned {
        pattern: "winit",
        route: WINDOW,
    },
    Banned {
        pattern: "softbuffer",
        route: WINDOW,
    },
];

/// Native capabilities that enter through a Platform Host (plan 03 rule 3, `#execution-domain`).
const HOST_CAPABILITIES: &[Banned] = &[
    Banned {
        pattern: "arboard",
        route: CLIPBOARD,
    },
    Banned {
        pattern: "tray-icon",
        route: TRAY,
    },
    Banned {
        pattern: "notify",
        route: WATCH,
    },
];

/// Native bindings: hand-written native calls stay in a host's adapters or stickymd-posix, so no
/// platform-independent shared crate declares one, while a third-party backend may hold it
/// (plan 03 rule 8, USER 2026-11-19). A binding crate a host or stickymd-posix adopts joins this
/// list in the same commit.
const NATIVE_BINDINGS: &[Banned] = &[
    Banned {
        pattern: "windows",
        route: WIN32,
    },
    Banned {
        pattern: "windows-sys",
        route: WIN32,
    },
    Banned {
        pattern: "libc",
        route: POSIX,
    },
    Banned {
        pattern: "objc2",
        route: OBJC,
    },
    Banned {
        pattern: "objc2-*",
        route: OBJC,
    },
    Banned {
        pattern: "block2",
        route: OBJC,
    },
];

/// Window handles, which only the window layers declare; a third-party library may carry them.
const WINDOW_HANDLE: &[Banned] = &[Banned {
    pattern: "raw-window-handle",
    route: WINDOW,
}];

/// The rendering engines plan 03 `#execution-domain` names (Comrak, RaTeX, cosmic-text,
/// tiny-skia); stickymd-core does not depend on rendering (plan 03 table).
const RENDER_ENGINES: &[Banned] = &[
    Banned {
        pattern: "comrak",
        route: RENDERING,
    },
    Banned {
        pattern: "ratex-*",
        route: RENDERING,
    },
    Banned {
        pattern: "cosmic-text",
        route: RENDERING,
    },
    Banned {
        pattern: "tiny-skia",
        route: RENDERING,
    },
];

/// One shared crate's boundary.
pub(super) struct Boundary {
    pub(super) name: &'static str,
    /// Workspace members it may depend on through a normal or build edge.
    pub(super) allowed: &'static [&'static str],
    /// Where this crate's refused local edges belong instead, when that depends on the crate
    /// rather than on the target; `None` uses the target's route.
    pub(super) route: Option<&'static str>,
    /// Neither declared nor reached through normal edges.
    pub(super) unreachable: &'static [&'static [Banned]],
    /// Not declared; a third-party crate may hold them.
    pub(super) undeclared: &'static [&'static [Banned]],
}

/// The layering of plan 03 `#shared-crates`: its diagram gives a direction
/// (core < render < app < desktop < Platform Host), not a list of the only edges, so any edge
/// pointing down is allowed (desktop → core is in the real workspace and not drawn). Every other
/// local crate is refused: stickymd-posix (only the macOS host uses it, and it uses no other
/// StickyMD crate), the hosts (no shared crate depends on a host), stickymd-smoke (not product
/// runtime) and any crate the plan does not name.
pub(super) const BOUNDARIES: &[Boundary] = &[
    Boundary {
        name: "stickymd-core",
        allowed: &[],
        route: None,
        unreachable: &[WINDOW_SYSTEM, HOST_CAPABILITIES, RENDER_ENGINES],
        undeclared: &[NATIVE_BINDINGS, WINDOW_HANDLE],
    },
    Boundary {
        name: "stickymd-render",
        allowed: &["stickymd-core"],
        route: None,
        unreachable: &[WINDOW_SYSTEM, HOST_CAPABILITIES],
        undeclared: &[NATIVE_BINDINGS, WINDOW_HANDLE],
    },
    Boundary {
        name: "stickymd-app",
        allowed: &["stickymd-core", "stickymd-render"],
        route: None,
        unreachable: &[WINDOW_SYSTEM, HOST_CAPABILITIES],
        undeclared: &[NATIVE_BINDINGS, WINDOW_HANDLE],
    },
    Boundary {
        name: "stickymd-desktop",
        allowed: &["stickymd-core", "stickymd-render", "stickymd-app"],
        route: None,
        unreachable: &[HOST_CAPABILITIES],
        undeclared: &[NATIVE_BINDINGS],
    },
    // A native adapter in the Execution Domain that depends on no other StickyMD crate (plan 03
    // table, USER 2026-11-19); the plan names no third-party crate it may not use.
    Boundary {
        name: "stickymd-posix",
        allowed: &[],
        route: Some(
            "stickymd-posix provides native primitives only; the macOS host wraps them into \
             Capability Port implementations and combines them with the shared crates",
        ),
        unreachable: &[],
        undeclared: &[],
    },
];

impl Boundary {
    pub(super) fn permits(&self, member: &str) -> bool {
        self.allowed.contains(&member)
    }

    /// Where a refused edge from this crate to `member` belongs instead.
    pub(super) fn route_to(&self, member: &str) -> &'static str {
        self.route.unwrap_or_else(|| local_route(member))
    }
}

pub(super) const OUTSIDE: &str =
    "plan 03 layers the workspace members, not copies of them; depend on the member by path";

/// What each local crate is, for the message that refuses an edge to it.
fn local_route(name: &str) -> &'static str {
    match name {
        "stickymd-core" => "stickymd-core is the lowest shared crate",
        "stickymd-render" => {
            "stickymd-render is the rendering projection above stickymd-core; keep what both \
             need in stickymd-core and let stickymd-render or stickymd-app call down to it"
        }
        "stickymd-app" => {
            "stickymd-app holds the intents, coordinators and Capability Ports above this crate; \
             return results upward and let stickymd-app call down"
        }
        "stickymd-desktop" => {
            "stickymd-desktop is the shared Interaction Shell; it calls down through \
             stickymd-app and nothing below calls it"
        }
        "stickymd-win" | "stickymd-mac" => {
            "a Platform Host is the top layer; define a Capability Port in stickymd-app and let \
             the host implement and inject it"
        }
        "stickymd-posix" => {
            "stickymd-posix is the macOS host's native adapter; go through a Capability Port in \
             stickymd-app that the macOS host implements"
        }
        "stickymd-smoke" => "stickymd-smoke is the verification CLI, never product runtime",
        _ => "the plan's crate table does not name this crate; add it to plan 03 first",
    }
}

pub(super) fn allowed_text(boundary: &Boundary) -> String {
    if boundary.allowed.is_empty() {
        "on no local crate".to_owned()
    } else {
        format!("only on {}", boundary.allowed.join(", "))
    }
}

pub(super) fn matches(pattern: &str, name: &str) -> bool {
    match pattern.strip_suffix('*') {
        Some(prefix) => name.starts_with(prefix),
        None => name == pattern,
    }
}

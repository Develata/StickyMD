//! VM-first standalone entry point, isolated scenarios, incremental atomic evidence.
//! plan_ref: docs/plan/11_testing_and_release.md#phase-16-macos-host
use super::{
    Options,
    input::Injection,
    live::{Live, command},
    model::*,
    native::{
        events::{self, Events},
        permissions, windows,
    },
    options::IDS,
    scenarios::{self, Driver},
};
use crate::mac_common::{
    fixture::{bundle_hash, contained_executable, copy_fixture},
    session::Session,
};
use std::{
    fs,
    path::Path,
    process::{Command, Stdio},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
const CONFIG: &str = "version = 1\nview_mode = \"source\"\nopacity = 100\nalways_on_top = false\ncontent_zoom_percent = 100\n[window]\nwidth_dip = 520\nheight_dip = 480\ndock_edge = \"none\"\n";
struct AppHash {
    executable: String,
    bundle: String,
    /// This driver's own executable, so the ledger can tell which driver produced the evidence.
    driver: String,
}
pub(super) fn execute(options: &Options) -> Result<(), String> {
    // This must precede app canonicalization, TCC checks, output creation and native GUI APIs.
    let vm = Command::new("/usr/sbin/sysctl")
        .args(["-n", "kern.hv_vmm_present"])
        .output()
        .map_err(|e| format!("VM detection failed (refused): {e}"))?;
    let value = if vm.status.success() {
        String::from_utf8_lossy(&vm.stdout).into_owned()
    } else {
        "unavailable".into()
    };
    require_vm(&value, options.allow_host)?;
    if !cfg!(panic = "unwind") {
        return Err("mac-gui refuses panic=abort builds: input release requires unwinding; build with CARGO_PROFILE_RELEASE_PANIC=unwind or use the dev binary".into());
    }
    let ids = if options.only.is_empty() {
        IDS.iter().map(|s| s.to_string()).collect()
    } else {
        options.only.clone()
    };
    let permissions = permissions::check();
    permission_error(&ids, permissions)?;
    let screen = windows::screen();
    if screen.size.width < 800.0 || screen.size.height < 600.0 {
        return Err(format!(
            "dedicated desktop needs at least 800x600 DIP for this fixture; observed {screen:?}"
        ));
    }
    let app = options
        .app
        .canonicalize()
        .map_err(|e| format!("app path: {e}"))?;
    if app.extension().is_none_or(|e| e != "app") || !app.is_dir() {
        return Err("--app must be a .app directory".into());
    }
    let hash = AppHash {
        executable: crate::integrity::sha256(&contained_executable(&app)?)?,
        driver: crate::integrity::sha256(
            &std::env::current_exe().map_err(|e| format!("driver path: {e}"))?,
        )?,
        bundle: bundle_hash(&app)?,
    };
    // Prevent output from ever targeting the supplied bundle or its canonical note directory.
    let out = output_path(&options.out)?;
    if out.starts_with(&app)
        || out.starts_with(app.parent().ok_or("bundle has no parent")?.join("note"))
    {
        return Err("evidence cannot be inside the input bundle or its note directory".into());
    }
    fs::create_dir_all(&out).map_err(|e| e.to_string())?;
    let environment = format!(
        "{{\"os\":{},\"architecture\":{},\"hv_vmm_present\":{},\"allow_host\":{},\"permissions\":{{\"accessibility\":{},\"post_event\":{},\"screen_recording\":{}}},\"unix_seconds\":{},\"fixture_config\":{},\"main_display\":{{\"x\":{},\"y\":{},\"width\":{},\"height\":{}}},\"coordinate_space\":\"main-display global CG points (DIP)\",\"keyboard_precondition\":\"ABC/US; not automatically verified\"}}",
        quote(&command("/usr/bin/sw_vers", &[])?),
        quote(std::env::consts::ARCH),
        quote(value.trim()),
        options.allow_host,
        permissions.ax,
        permissions.post,
        permissions.screen,
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| e.to_string())?
            .as_secs(),
        quote(CONFIG),
        screen.origin.x,
        screen.origin.y,
        screen.size.width,
        screen.size.height
    );
    let mut rows = ids
        .iter()
        .map(|id| {
            let mut row = Row::new(id);
            row.skip("execution", "not started");
            row
        })
        .collect::<Vec<_>>();
    save(&out, &environment, &hash, &rows)?;
    events::install_cancel()?;
    let mut input = Injection::new(Events::new());
    for (index, id) in ids.iter().enumerate() {
        let mut row = Row::new(id);
        if let Some(reason) = scenarios::skipped(id) {
            row.skip("deferred", reason);
        } else if let Err(reason) = input.require_running().and_then(|()| events::running()) {
            row.skip("shared stop", &reason);
        } else {
            println!("{id}: isolated VM scenario starting");
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                one(&app, &out, &hash, id, permissions, &mut input, &mut row)
            }));
            match result {
                Ok(Ok(())) => {}
                Ok(Err(error)) => row.check("execution", false, error),
                Err(payload) => {
                    std::mem::forget(payload);
                    input.stopped = Some("scenario unwound; owned child cleanup attempted".into());
                    row.check(
                        "execution",
                        false,
                        "scenario panicked; all later injection stopped",
                    );
                }
            }
        }
        row.trace.append(&mut input.backend.trace);
        row.trace.sort_by_key(|t| t.ms);
        println!("{id}: {}", aggregate(&row.checks).name());
        rows[index] = row;
        save(&out, &environment, &hash, &rows)?;
    }
    println!("Evidence: {}", out.join("mac-gui.json").display());
    let count = |verdict| {
        rows.iter()
            .filter(|r| aggregate(&r.checks) == verdict)
            .count()
    };
    println!(
        "PASS={} FAIL={} NOT_TESTED={}; real GUI evidence only, no qualification status changed",
        count(Verdict::Pass),
        count(Verdict::Fail),
        count(Verdict::NotTested)
    );
    if count(Verdict::Fail) > 0 {
        Err("mac-gui scenario failed; see mac-gui.json".into())
    } else {
        Ok(())
    }
}
fn output_path(path: &Path) -> Result<std::path::PathBuf, String> {
    let absolute = std::path::absolute(path).map_err(|e| e.to_string())?;
    if absolute
        .components()
        .any(|c| c == std::path::Component::ParentDir)
    {
        return Err("--out must not contain '..'".into());
    }
    let mut ancestor = absolute.as_path();
    let mut missing = Vec::new();
    while !ancestor.exists() {
        missing.push(
            ancestor
                .file_name()
                .ok_or("invalid output path")?
                .to_owned(),
        );
        ancestor = ancestor.parent().ok_or("output has no existing ancestor")?;
    }
    let mut resolved = ancestor.canonicalize().map_err(|e| e.to_string())?;
    for name in missing.into_iter().rev() {
        resolved.push(name);
    }
    Ok(resolved)
}
fn save(out: &Path, environment: &str, hash: &AppHash, rows: &[Row]) -> Result<(), String> {
    let json = format!(
        "{{\"schema_version\":1,\"acceptance_status\":\"INCOMPLETE\",\"environment\":{environment},\"app_executable_sha256\":{},\"app_bundle_manifest_sha256\":{},\"driver_sha256\":{},\"scenarios\":[{}]}}\n",
        quote(&hash.executable),
        quote(&hash.bundle),
        quote(&hash.driver),
        rows.iter().map(Row::json).collect::<Vec<_>>().join(",")
    );
    crate::atomic_evidence::write(&out.join("mac-gui.json"), json.as_bytes())
}
fn one(
    app: &Path,
    out: &Path,
    hash: &AppHash,
    id: &str,
    permissions: Permissions,
    input: &mut Injection<Events>,
    row: &mut Row,
) -> Result<(), String> {
    let mut session = Session::new()?;
    // A save fault left in place would make cleanup fail and stop every later scenario. Declared
    // after the session, the guard drops first, on unwind too.
    let mut fault =
        super::fault::RestoreOnDrop::new(matches!(id, "W11" | "W12").then(|| session.anchor()));
    let result = (|| {
        let note = match id {
            "F01" => "中文字体验证\n\n`Menlo code 012345`\n",
            // Old bytes a save that truncates before it fails would destroy.
            "W11" | "W12" => super::lifecycle::DURABLE,
            _ => "",
        };
        let executable = copy_fixture(&mut session, app, note, CONFIG, &hash.executable)?;
        if bundle_hash(&session.path().join("StickyMD.app"))? != hash.bundle {
            return Err("copied bundle manifest changed; refused before launch".into());
        }
        let stderr = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(session.path().join("stderr.log"))
            .map_err(|e| e.to_string())?;
        session.spawn(
            Command::new(executable)
                .current_dir(session.path())
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(stderr),
        )?;
        let pid = session.child.as_ref().ok_or("missing child")?.id();
        row.trace.push(Trace {
            ms: input.backend.started.elapsed().as_millis() as u64,
            event: format!(
                "spawn PID={pid}; program_dir={}; fixture_sha256={}",
                session.path().display(),
                crate::integrity::sha256_bytes(note.as_bytes())?
            ),
        });
        let start = Instant::now();
        let window = loop {
            if let Some(status) = session.poll()? {
                return Err(format!("product exited before ready: {status}"));
            }
            if let Ok(w) = windows::window(pid, None)
                && w.onscreen
            {
                break w;
            }
            if start.elapsed() > Duration::from_secs(20) {
                return Err("no unique on-screen product window within 20s".into());
            }
            events::wait(100)?;
        };
        events::wait(500)?;
        let mut live = Live {
            session: &mut session,
            input,
            pid,
            window_id: window.id,
            out,
            permissions,
        };
        scenarios::run(
            &mut Driver {
                backend: &mut live,
                row,
                pid,
            },
            id,
        )
    })();
    let release = input.release();
    let writable = fault.restore();
    // Capture stderr even on scenario failure, before owned temporary-directory cleanup.
    let log = match fs::read(session.path().join("stderr.log")) {
        Ok(bytes) => crate::atomic_evidence::write(&out.join(format!("{id}-stderr.log")), &bytes),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e.to_string()),
    };
    let cleanup = session.cleanup();
    row.trace.push(Trace {
        ms: input.backend.started.elapsed().as_millis() as u64,
        event: format!("owned child/directory cleanup: {cleanup:?}; input release: {release:?}"),
    });
    if cleanup.is_err() {
        input.stopped = Some("child/directory cleanup failed; no more scenarios may launch".into());
    }
    let errors = [result, release, writable, log, cleanup]
        .into_iter()
        .filter_map(Result::err)
        .collect::<Vec<_>>();
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors.join("; "))
    }
}

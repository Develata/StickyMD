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
    staging::{self, Bytes, Candidate},
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
    /// The product inputs the bundle was built from, when `build-app.sh` recorded them.
    inputs: Option<String>,
    /// This driver's own executable, so the ledger can tell which driver produced the evidence.
    driver: String,
    /// The candidate ZIP R03 opens (`--zip`), checked to hold this bundle before any row runs.
    zip: Option<Candidate>,
    /// R03's write watch calibration (`--r03-write-restore`): its evidence cannot be recorded.
    write_restore: bool,
}

impl AppHash {
    fn bytes(&self) -> Bytes {
        Bytes {
            executable: self.executable.clone(),
            bundle: self.bundle.clone(),
        }
    }
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
    let mut hash = AppHash {
        executable: crate::integrity::sha256(&contained_executable(&app)?)?,
        driver: crate::integrity::sha256(
            &std::env::current_exe().map_err(|e| format!("driver path: {e}"))?,
        )?,
        bundle: bundle_hash(&app)?,
        inputs: crate::mac_common::fixture::app_inputs(&app)?,
        zip: None,
        write_restore: options.r03_write_restore,
    };
    // A ZIP that does not hold this bundle refuses the run: it is the operator's mistake, not an
    // R03 result (plan 11 #phase-16-macos-host).
    if let Some(zip) = &options.zip {
        let mut scratch = Session::new()?;
        let checked = staging::check(zip, &hash.bytes(), scratch.path());
        scratch.cleanup()?;
        hash.zip = Some(checked?);
    }
    // Prevent output from ever targeting the supplied bundle or its canonical note directory.
    let out = crate::mac_common::output::guarded(&options.out, &app)?;
    fs::create_dir_all(&out).map_err(|e| e.to_string())?;
    // Review B7: nothing of an earlier run (or a file named like one, such as the --zip input) is
    // replaced; the first checkpoint is created exclusively.
    refuse_earlier_evidence(&out)?;
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
    save(
        &out,
        &environment,
        &hash,
        &rows,
        false,
        options.batch.as_deref(),
        true,
    )?;
    events::install_cancel()?;
    let mut input = Injection::new(Events::new());
    // A run cut short (cancelled, or stopped after a panic or a failed cleanup) is never marked
    // finished: its unstarted rows would read as results (plan 11 #phase-16-macos-host).
    let mut cut_short = false;
    for (index, id) in ids.iter().enumerate() {
        let mut row = Row::new(id);
        if let Some(reason) = scenarios::skipped(id) {
            row.skip("deferred", reason);
        } else if let Err(reason) = input.require_running().and_then(|()| events::running()) {
            cut_short = true;
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
        save(
            &out,
            &environment,
            &hash,
            &rows,
            false,
            options.batch.as_deref(),
            false,
        )?;
    }
    cut_short |= input.stopped.is_some() || events::running().is_err();
    save(
        &out,
        &environment,
        &hash,
        &rows,
        !cut_short,
        options.batch.as_deref(),
        false,
    )?;
    if cut_short {
        println!(
            "the run was cut short; its evidence is not marked finished and cannot be recorded"
        );
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
    } else if cut_short {
        Err("mac-gui run was cut short; see mac-gui.json".into())
    } else {
        Ok(())
    }
}
/// The product's input-latency diagnostic file in the session directory (W10 reads its
/// presenter counters).
pub(super) const DIAGNOSTIC: &str = "latency.txt";
/// Starts the copied product in the session directory and waits for its one on-screen window.
/// The first launch creates `stderr.log`; a relaunch appends to it. With `diagnostic` the
/// product records frames and publishes them to `DIAGNOSTIC` on SIGUSR1.
pub(super) fn launch(
    session: &mut Session,
    executable: &Path,
    relaunch: bool,
    diagnostic: bool,
) -> Result<(u32, Window), String> {
    let stderr = fs::OpenOptions::new()
        .append(relaunch)
        .write(true)
        .create_new(!relaunch)
        .open(session.path().join("stderr.log"))
        .map_err(|e| e.to_string())?;
    let mut command = Command::new(executable);
    if diagnostic {
        command.env(
            super::latency::DIAGNOSTIC_ENV,
            session.path().join(DIAGNOSTIC),
        );
    }
    session.spawn(
        command
            .current_dir(session.path())
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(stderr),
    )?;
    let pid = session.child.as_ref().ok_or("missing child")?.id();
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
    Ok((pid, window))
}
/// `path` made absolute with its existing part canonical; `..` is refused. W12's logout cases use
/// it too.
/// Writes the evidence; `finished` is true only on the write after the last row, so a checkpoint
/// of an interrupted run is never taken for a finished one (mac-ledger records only finished runs).
/// Schema 2 is the first whose `finished` only this driver's rules set, and the first that names
/// the batch the run was started for; mac-ledger records no older schema.
/// Refuses an --out holding a `mac-gui.json` or an attachment name this driver writes.
fn refuse_earlier_evidence(out: &Path) -> Result<(), String> {
    for entry in fs::read_dir(out).map_err(|e| format!("{}: {e}", out.display()))? {
        let name = entry.map_err(|e| e.to_string())?.file_name();
        let name = name.to_string_lossy();
        if name == "mac-gui.json" || name.ends_with("-stderr.log") || name.starts_with("R03-") {
            return Err(format!(
                "{} already holds {name}; each run needs an --out of its own",
                out.display()
            ));
        }
    }
    Ok(())
}

/// `first`: the run's first checkpoint, created exclusively; later ones replace only it.
fn save(
    out: &Path,
    environment: &str,
    hash: &AppHash,
    rows: &[Row],
    finished: bool,
    batch: Option<&str>,
    first: bool,
) -> Result<(), String> {
    let batch = batch.map_or_else(String::new, |b| format!(",\"batch\":{}", quote(b)));
    // The candidate ZIP R03 opened; mac-ledger records it with R03 alone.
    let zip = hash.zip.as_ref().map_or_else(String::new, |zip| {
        format!(",\"zip_sha256\":{}", quote(&zip.sha256))
    });
    // A calibration run's evidence says so, and mac-ledger refuses it.
    let zip = if hash.write_restore {
        format!("{zip},\"calibration\":\"r03-write-restore\"")
    } else {
        zip
    };
    let json = format!(
        "{{\"schema_version\":2,\"acceptance_status\":\"INCOMPLETE\",\"finished\":{finished}{batch},\"environment\":{environment}{},\"app_executable_sha256\":{},\"app_bundle_manifest_sha256\":{},\"driver_sha256\":{}{zip},\"scenarios\":[{}]}}\n",
        hash.inputs
            .as_ref()
            .map_or_else(String::new, |inputs| format!(
                ",\"app_inputs\":{}",
                quote(inputs)
            )),
        quote(&hash.executable),
        quote(&hash.bundle),
        quote(&hash.driver),
        rows.iter().map(Row::json).collect::<Vec<_>>().join(",")
    );
    let path = out.join("mac-gui.json");
    if first {
        crate::atomic_evidence::write_new(&path, json.as_bytes())
    } else {
        crate::atomic_evidence::write(&path, json.as_bytes())
    }
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
        if id == "R03" {
            let stopped = super::translocation::run(
                app,
                session.path(),
                &hash.bytes(),
                hash.zip.as_ref(),
                hash.write_restore,
                input.backend.started,
                row,
            )?;
            // Not a product result: the row stays untested and the run is not finished.
            if let Some(reason) = stopped {
                row.skip("execution", &reason);
                input.stopped = Some(format!("R03 stopped the run: {reason}"));
            }
            return Ok(());
        }
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
        let (pid, window) = launch(&mut session, &executable, false, id == "W10")?;
        row.trace.push(Trace {
            ms: input.backend.started.elapsed().as_millis() as u64,
            event: format!(
                "spawn PID={pid}; program_dir={}; fixture_sha256={}",
                session.path().display(),
                crate::integrity::sha256_bytes(note.as_bytes())?
            ),
        });
        let mut live = Live {
            session: &mut session,
            input,
            pid,
            window_id: window.id,
            out,
            permissions,
            executable,
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
    // Capture stderr (and R03's prompt image and termination log) even on scenario failure,
    // before owned temporary-directory cleanup.
    let attachments: &[&str] = if id == "R03" {
        &["stderr.log", "prompt.ppm", "system.log"]
    } else {
        &["stderr.log"]
    };
    // Each attachment this run made is named in its trace with its SHA-256; a file of the same
    // name that an earlier run left in --out is not removed (it is not provably this tool's),
    // and the trace says it is not this run's.
    let log = attachments.iter().try_for_each(|name| {
        let target = out.join(format!("{id}-{name}"));
        let ms = input.backend.started.elapsed().as_millis() as u64;
        match fs::read(session.path().join(name)) {
            Ok(bytes) => {
                crate::atomic_evidence::write_new(&target, &bytes)?;
                row.trace.push(Trace {
                    ms,
                    event: format!(
                        "attachment {} sha256={}",
                        target.display(),
                        crate::integrity::sha256_bytes(&bytes)?
                    ),
                });
                Ok(())
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                if target.exists() {
                    row.trace.push(Trace {
                        ms,
                        event: format!("{} is from an earlier run, not this one", target.display()),
                    });
                }
                Ok(())
            }
            Err(e) => Err(e.to_string()),
        }
    });
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

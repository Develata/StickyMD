//! Opt-in Windows runtime smoke using copied Release executables.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use crate::runner::RuntimeScenario;

const START_TIMEOUT: Duration = Duration::from_secs(10);
const EXIT_TIMEOUT: Duration = Duration::from_secs(5);

pub(crate) fn run(repository: &Path, scenario: RuntimeScenario) -> Result<(), String> {
    let root = create_smoke_root()?;
    let mut children = Vec::new();
    let result = run_inner(repository, &root, scenario, &mut children);
    stop_children(&mut children);
    let cleanup = cleanup_root(&root);
    match (result, cleanup) {
        (Err(error), _) => Err(error),
        (Ok(()), Err(error)) => Err(error),
        (Ok(()), Ok(())) => Ok(()),
    }
}

fn run_inner(
    repository: &Path,
    root: &Path,
    scenario: RuntimeScenario,
    children: &mut Vec<Child>,
) -> Result<(), String> {
    let source = repository.join("target/release/stickymd-win.exe");
    if !source.is_file() {
        return Err(format!(
            "Release executable is missing: {}; run the planned Release build first",
            source.display()
        ));
    }
    let first_dir = root.join("first");
    let first_exe = copy_executable(&source, &first_dir)?;
    if scenario == RuntimeScenario::Preview {
        prepare_preview_layout(&first_dir, "preview")?;
    }
    children.push(start(&first_exe)?);
    wait_for_layout(&first_dir)?;
    ensure_alive(&mut children[0], "first portable instance")?;

    if scenario == RuntimeScenario::Launch {
        return Ok(());
    }

    if scenario == RuntimeScenario::Preview {
        let second_dir = root.join("split");
        let second_exe = copy_executable(&source, &second_dir)?;
        prepare_preview_layout(&second_dir, "split")?;
        children.push(start(&second_exe)?);
        wait_for_layout(&second_dir)?;
        thread::sleep(Duration::from_millis(500));
        ensure_alive(&mut children[0], "Preview-mode portable instance")?;
        ensure_alive(&mut children[1], "Split-mode portable instance")?;
        return Ok(());
    }

    thread::sleep(Duration::from_millis(300));
    let note = first_dir.join("note/note.md");
    let config = first_dir.join("note/config.toml");
    let before = (file_state(&note)?, file_state(&config)?);
    let mut secondary = start(&first_exe)?;
    let secondary_status = wait_for_exit(&mut secondary, EXIT_TIMEOUT)?;
    if !secondary_status.success() {
        return Err(format!(
            "same-directory secondary exited unsuccessfully: {secondary_status}"
        ));
    }
    let after = (file_state(&note)?, file_state(&config)?);
    if before != after {
        return Err("same-directory secondary modified durable files".to_owned());
    }

    let second_dir = root.join("second");
    let second_exe = copy_executable(&source, &second_dir)?;
    children.push(start(&second_exe)?);
    wait_for_layout(&second_dir)?;
    ensure_alive(&mut children[0], "first portable instance")?;
    ensure_alive(&mut children[1], "different-directory portable instance")?;
    Ok(())
}

fn prepare_preview_layout(program_directory: &Path, view_mode: &str) -> Result<(), String> {
    let note_directory = program_directory.join("note");
    fs::create_dir(&note_directory).map_err(|error| {
        format!(
            "cannot create preview smoke note directory {}: {error}",
            note_directory.display()
        )
    })?;
    let fixture = concat!(
        "# StickyMD Preview Smoke\n\n",
        "中文 **粗体** and *italic* with [safe link](https://example.com).\n\n",
        "> quote\n\n- [x] task\n\n",
        "| left | right |\n| :--- | ---: |\n| A | B |\n\n",
        "`inline` and $x^2$\n\n",
        "![remote placeholder](https://example.invalid/no-fetch.png)\n\n",
        "<script>throw new Error('must remain literal')</script>\n\n",
        "<iframe src=\"https://example.invalid/must-not-load\"></iframe>\n"
    );
    fs::write(note_directory.join("note.md"), fixture)
        .map_err(|error| format!("cannot seed preview smoke note: {error}"))?;
    fs::write(
        note_directory.join("config.toml"),
        format!("version = 1\nview_mode = \"{view_mode}\"\n"),
    )
    .map_err(|error| format!("cannot seed preview smoke config: {error}"))?;
    Ok(())
}

fn create_smoke_root() -> Result<PathBuf, String> {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| format!("system clock is before Unix epoch: {error}"))?
        .as_nanos();
    let root = std::env::temp_dir().join(format!("stickymd-smoke-{}-{nonce}", std::process::id()));
    fs::create_dir(&root)
        .map_err(|error| format!("cannot create smoke root {}: {error}", root.display()))?;
    Ok(root)
}

fn copy_executable(source: &Path, directory: &Path) -> Result<PathBuf, String> {
    fs::create_dir(directory)
        .map_err(|error| format!("cannot create {}: {error}", directory.display()))?;
    let destination = directory.join("StickyMD.exe");
    fs::copy(source, &destination).map_err(|error| {
        format!(
            "cannot copy {} to {}: {error}",
            source.display(),
            destination.display()
        )
    })?;
    Ok(destination)
}

fn start(executable: &Path) -> Result<Child, String> {
    Command::new(executable)
        .current_dir(
            executable
                .parent()
                .ok_or_else(|| format!("{} has no parent", executable.display()))?,
        )
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|error| format!("cannot start {}: {error}", executable.display()))
}

fn wait_for_layout(program_directory: &Path) -> Result<(), String> {
    let note = program_directory.join("note/note.md");
    let config = program_directory.join("note/config.toml");
    let deadline = Instant::now() + START_TIMEOUT;
    while Instant::now() < deadline {
        if note.is_file() && config.is_file() {
            return Ok(());
        }
        thread::sleep(Duration::from_millis(50));
    }
    Err(format!(
        "portable layout was not created within {:?}: {}",
        START_TIMEOUT,
        program_directory.display()
    ))
}

fn ensure_alive(child: &mut Child, label: &str) -> Result<(), String> {
    match child
        .try_wait()
        .map_err(|error| format!("cannot inspect {label}: {error}"))?
    {
        None => Ok(()),
        Some(status) => Err(format!("{label} exited early with {status}")),
    }
}

fn wait_for_exit(child: &mut Child, timeout: Duration) -> Result<std::process::ExitStatus, String> {
    let deadline = Instant::now() + timeout;
    loop {
        if let Some(status) = child
            .try_wait()
            .map_err(|error| format!("cannot inspect secondary process: {error}"))?
        {
            return Ok(status);
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            return Err(format!("secondary process did not exit within {timeout:?}"));
        }
        thread::sleep(Duration::from_millis(50));
    }
}

fn file_state(path: &Path) -> Result<(Vec<u8>, SystemTime), String> {
    let bytes =
        fs::read(path).map_err(|error| format!("cannot read {}: {error}", path.display()))?;
    let modified = fs::metadata(path)
        .and_then(|metadata| metadata.modified())
        .map_err(|error| format!("cannot inspect {}: {error}", path.display()))?;
    Ok((bytes, modified))
}

fn stop_children(children: &mut [Child]) {
    for child in children {
        match child.try_wait() {
            Ok(Some(_)) => {}
            Ok(None) => {
                let _ = child.kill();
                let _ = child.wait();
            }
            Err(_) => {
                let _ = child.kill();
                let _ = child.wait();
            }
        }
    }
}

fn cleanup_root(root: &Path) -> Result<(), String> {
    let temporary = std::env::temp_dir();
    let safe_name = root
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.starts_with("stickymd-smoke-"));
    if !root.starts_with(&temporary) || !safe_name {
        return Err(format!(
            "refusing to remove unverified smoke directory {}",
            root.display()
        ));
    }
    let retry_delays = [0, 50, 100, 200, 400, 800];
    let mut last_error = None;
    for delay_ms in retry_delays {
        if delay_ms != 0 {
            thread::sleep(Duration::from_millis(delay_ms));
        }
        match fs::remove_dir_all(root) {
            Ok(()) => return Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(error) => last_error = Some(error),
        }
    }
    Err(format!(
        "cannot remove smoke directory {} after bounded retries: {}",
        root.display(),
        last_error.map_or_else(|| "unknown error".to_owned(), |error| error.to_string())
    ))
}

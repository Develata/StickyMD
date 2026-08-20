#[cfg(windows)]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use std::process::Command;
    use std::time::{SystemTime, UNIX_EPOCH};

    use spike_persistence::windows_adapter;
    use spike_persistence::{atomic_write, directory_identity, writable_check};

    let mut args = std::env::args();
    let _program = args.next();
    if args.next().as_deref() == Some("--probe") {
        let mutex = args.next().ok_or("missing mutex name")?;
        let event = args.next().ok_or("missing event name")?;
        if windows_adapter::acquire_first_instance(&mutex)?.is_none() {
            windows_adapter::signal_activation_event(&event)?;
            return Ok(());
        }
        return Err("probe unexpectedly became first instance".into());
    }

    let nonce = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
    let base = std::env::temp_dir().join(format!(
        "stickymd-phase1-demo-{}-{nonce}",
        std::process::id()
    ));
    let note_dir = base.join("note");
    writable_check(&note_dir)?;
    atomic_write(&note_dir, "note.md", b"first\r\n")?;
    atomic_write(&note_dir, "note.md", b"second complete\r\n")?;

    let canonical = windows_adapter::canonical_directory(&base)?;
    let identity = directory_identity(&canonical);
    let mutex_name = format!("{identity}.Mutex");
    let event_name = format!("{identity}.Show");
    let _first = windows_adapter::acquire_first_instance(&mutex_name)?
        .ok_or("first instance mutex already existed")?;
    let event = windows_adapter::create_activation_event(&event_name)?;
    let status = Command::new(std::env::current_exe()?)
        .args(["--probe", &mutex_name, &event_name])
        .status()?;
    if !status.success() || !windows_adapter::wait_for_activation(&event, 1_000) {
        return Err("second-instance wake verification failed".into());
    }

    println!("canonical directory: {canonical}");
    println!("identity: {identity}");
    println!("atomic create/replace: PASS");
    println!("second-instance wake: PASS");
    std::fs::remove_dir_all(base)?;
    Ok(())
}

#[cfg(not(windows))]
fn main() {
    eprintln!("This persistence spike's platform adapter must be run on Windows.");
}

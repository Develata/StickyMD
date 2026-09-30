//! Exclusive bounded scratch bytes, hashed by the existing platform SHA-256 adapter.
//! plan_ref: docs/plan/11_testing_and_release.md#resource-module-qualification

use std::{
    fs::{self, File},
    io::Write,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

struct Scratch {
    path: PathBuf,
    file: Option<File>,
}
impl Drop for Scratch {
    fn drop(&mut self) {
        drop(self.file.take());
        let _ = fs::remove_file(&self.path);
    }
}

pub(super) fn bytes(bytes: &[u8]) -> Result<String, String> {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let path = std::env::temp_dir().join(format!(
        "stickymd-diagnostic-digest-{}-{}-{}.tmp",
        std::process::id(),
        super::now()?,
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let file = File::create_new(&path).map_err(|e| e.to_string())?;
    let mut scratch = Scratch {
        path,
        file: Some(file),
    };
    let file = scratch.file.as_mut().expect("open scratch");
    file.write_all(bytes)
        .and_then(|()| file.flush())
        .map_err(|e| e.to_string())?;
    drop(scratch.file.take());
    crate::integrity::sha256(&scratch.path)
}

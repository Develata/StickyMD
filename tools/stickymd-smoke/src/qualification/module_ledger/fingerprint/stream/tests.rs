//! Real filesystem failure and scratch ownership regressions; no candidate receipts.
use super::*;

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let path = temporary_path().unwrap().with_extension("streams");
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

#[test]
fn scratch_ownership_survives_collisions_success_and_unwind() {
    let fixture = Fixture::new();
    let path = fixture.0.join("stream.bin");
    fs::write(&path, b"not owned by this invocation").unwrap();
    assert!(Stream::create(path.clone(), None).is_err());
    assert_eq!(fs::read(&path).unwrap(), b"not owned by this invocation");
    fs::remove_file(&path).unwrap();
    {
        let mut stream = Stream::create(path.clone(), None).unwrap();
        stream.writer().write_all(&vec![17; 65_537]).unwrap();
        let digest = stream.finish().unwrap();
        assert_eq!(digest, receipt::sha256(&path).unwrap());
    }
    assert!(!path.exists());
    let unwound = std::panic::catch_unwind(|| {
        let _stream = Stream::create(path.clone(), None).unwrap();
        panic!("injected interruption");
    });
    assert!(unwound.is_err());
    assert!(!path.exists());
}

#[test]
fn input_and_flush_failures_discard_every_owned_stream() {
    let fixture = Fixture::new();
    fs::write(fixture.0.join("source.bin"), b"input").unwrap();
    let paths = [fixture.0.join("first.bin"), fixture.0.join("second.bin")];
    {
        let mut streams = paths
            .iter()
            .map(|p| Stream::create(p.clone(), None).unwrap())
            .collect::<Vec<_>>();
        let result = write_inputs(
            &fixture.0,
            &mut streams,
            &["source.bin".into(), "missing.bin".into()],
            &[],
        );
        assert!(result.unwrap_err().contains("missing.bin"));
    }
    assert!(paths.iter().all(|p| !p.exists()));
    {
        let mut stream = Stream::create(paths[0].clone(), None).unwrap();
        // A real read-only handle makes the explicit flush fail after buffered writes succeed.
        stream.writer = Some(BufWriter::new(File::open(&paths[0]).unwrap()));
        stream.writer().write_all(b"buffered bytes").unwrap();
        assert!(stream.finish().is_err());
    }
    assert!(paths.iter().all(|p| !p.exists()));
    assert_eq!(fs::read(fixture.0.join("source.bin")).unwrap(), b"input");
}

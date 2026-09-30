use super::*;
use std::sync::{Arc, Barrier};

fn fixture(root: &Path) -> PackagePublishOptions {
    PackagePublishOptions {
        input: root.join("completed 中文.zip"),
        output: root.join("portable 中文 space.zip"),
        checksums: root.join("SHA256SUMS.txt"),
    }
}

#[test]
fn publication_is_repeatable_and_preserves_conflicting_outputs_and_inputs() {
    let temp = TemporaryDirectory::new("package-output").unwrap();
    let options = fixture(temp.path());
    fs::write(&options.input, b"completed archive").unwrap();
    let hash = publish(&options).unwrap();
    assert_eq!(publish(&options).unwrap(), hash);
    assert_eq!(integrity::sha256(&options.output).unwrap(), hash);
    assert_eq!(fs::read(&options.input).unwrap(), b"completed archive");
    let manifest = fs::read(&options.checksums).unwrap();
    assert_eq!(
        manifest,
        checksums::manifest(&options.output, &hash, None)
            .unwrap()
            .as_bytes()
    );
    fs::write(&options.input, b"different archive").unwrap();
    assert!(
        publish(&options)
            .unwrap_err()
            .starts_with("Refusing to overwrite")
    );
    assert_eq!(fs::read(&options.checksums).unwrap(), manifest);
    assert_eq!(integrity::sha256(&options.output).unwrap(), hash);
    for changed in [
        PackagePublishOptions {
            output: options.input.clone(),
            ..options.clone()
        },
        PackagePublishOptions {
            checksums: options.output.clone(),
            ..options.clone()
        },
        PackagePublishOptions {
            input: temp.path().join("missing.zip"),
            ..options.clone()
        },
        PackagePublishOptions {
            output: temp.path().join("invalid.zip."),
            ..options.clone()
        },
    ] {
        assert!(publish(&changed).is_err(), "accepted {changed:?}");
    }
    assert_eq!(fs::read(&options.checksums).unwrap(), manifest);
    assert_eq!(fs::read_dir(temp.path()).unwrap().count(), 3);
}

#[test]
fn concurrent_publication_never_overwrites_a_winner() {
    for same_bytes in [false, true] {
        let temp = TemporaryDirectory::new("package-race").unwrap();
        let first = fixture(temp.path());
        let second = PackagePublishOptions {
            input: temp.path().join("second.zip"),
            ..first.clone()
        };
        fs::write(&first.input, b"first").unwrap();
        fs::write(&second.input, if same_bytes { b"first" } else { b"other" }).unwrap();
        let barrier = Arc::new(Barrier::new(2));
        let handles = [first.clone(), second]
            .into_iter()
            .map(|options| {
                let barrier = Arc::clone(&barrier);
                std::thread::spawn(move || {
                    barrier.wait();
                    publish(&options)
                })
            })
            .collect::<Vec<_>>();
        let successes = handles
            .into_iter()
            .map(|h| h.join().unwrap())
            .filter(Result::is_ok)
            .count();
        assert_eq!(successes, if same_bytes { 2 } else { 1 });
        let hash = integrity::sha256(&first.output).unwrap();
        assert_eq!(
            fs::read_to_string(&first.checksums).unwrap(),
            checksums::manifest(&first.output, &hash, None).unwrap()
        );
        assert_eq!(fs::read_dir(temp.path()).unwrap().count(), 4);
    }
}

#[cfg(windows)]
#[test]
fn locked_manifest_reports_failure_without_removing_a_published_zip() {
    use std::os::windows::fs::OpenOptionsExt;
    let temp = TemporaryDirectory::new("package-manifest-failure").unwrap();
    let options = fixture(temp.path());
    fs::write(&options.input, b"complete").unwrap();
    fs::write(&options.checksums, b"previous manifest").unwrap();
    let locked = fs::OpenOptions::new()
        .read(true)
        .share_mode(1)
        .open(&options.checksums)
        .unwrap();
    assert!(publish(&options).is_err());
    assert_eq!(fs::read(&options.output).unwrap(), b"complete");
    assert_eq!(fs::read(&options.checksums).unwrap(), b"previous manifest");
    drop(locked);
    publish(&options).unwrap();
    assert_eq!(fs::read_dir(temp.path()).unwrap().count(), 3);
}

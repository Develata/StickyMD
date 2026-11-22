use super::*;

#[test]
fn known_vectors_cover_empty_short_and_multiple_read_buffers() {
    for (input, expected) in [
        (
            Vec::new(),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
        ),
        (
            b"abc".to_vec(),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad",
        ),
        (
            b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq".to_vec(),
            "248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1",
        ),
        (
            vec![b'a'; 1_000_000],
            "cdc76e5c9914fb9281a1c7e284d73e67f1809a48a497200e046d39ccc7112cd0",
        ),
    ] {
        assert_eq!(digest(&mut input.as_slice()).unwrap(), expected);
        let mut short = ShortReads {
            bytes: &input,
            interrupted: false,
        };
        assert_eq!(digest(&mut short).unwrap(), expected);
    }
}

struct ShortReads<'a> {
    bytes: &'a [u8],
    interrupted: bool,
}
impl Read for ShortReads<'_> {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        if !self.interrupted {
            self.interrupted = true;
            return Err(io::ErrorKind::Interrupted.into());
        }
        let size = buffer.len().min(63);
        self.bytes.read(&mut buffer[..size])
    }
}

#[test]
fn read_errors_never_finalize_partial_data_as_a_successful_hash() {
    struct InvalidLength;
    impl Read for InvalidLength {
        fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
            Ok(buffer.len() + 1)
        }
    }
    assert!(
        digest(&mut InvalidLength)
            .unwrap_err()
            .contains("invalid length")
    );
    struct Broken(bool);
    impl Read for Broken {
        fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
            if self.0 {
                return Err(io::Error::other("injected failure"));
            }
            self.0 = true;
            bytes[0] = b'a';
            Ok(1)
        }
    }
    assert!(
        digest(&mut Broken(false))
            .unwrap_err()
            .contains("injected failure")
    );
    assert!(
        check(-1, "injected CNG call")
            .unwrap_err()
            .contains("0xffffffff")
    );
    // A later operation still works after early failure and RAII cleanup.
    assert!(digest(&mut &b"abc"[..]).is_ok());
}

#[test]
fn file_errors_and_changes_are_observed_without_a_digest_cache() {
    use std::{
        fs,
        os::windows::fs::OpenOptionsExt,
        time::{SystemTime, UNIX_EPOCH},
    };
    let root = std::env::temp_dir().join(format!(
        "stickymd-cng-{}-{}-中文 space",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir(&root).unwrap();
    let path = root.join("hash.bin");
    assert!(sha256(&path).is_err());
    assert!(sha256(&root).is_err());
    fs::write(&path, b"abc").unwrap();
    let original = sha256(&path).unwrap();
    let locked = fs::OpenOptions::new()
        .read(true)
        .share_mode(0)
        .open(&path)
        .unwrap();
    assert!(sha256(&path).is_err());
    drop(locked);
    fs::write(&path, b"changed").unwrap();
    assert_ne!(sha256(&path).unwrap(), original);
    fs::remove_dir_all(root).unwrap();
}

use std::env;
use std::fs;
use std::path::PathBuf;

const MANIFEST_FILE: &str = "StickyMD.manifest";
const REQUIRED_DPI_MARKER: &str = ">PerMonitorV2<";

fn main() {
    println!("cargo:rerun-if-changed={MANIFEST_FILE}");
    let manifest = fs::read_to_string(MANIFEST_FILE)
        .unwrap_or_else(|error| panic!("failed to read {MANIFEST_FILE}: {error}"));
    assert!(
        manifest.contains(REQUIRED_DPI_MARKER),
        "{MANIFEST_FILE} must declare PerMonitorV2 DPI awareness"
    );

    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows")
        || env::var("CARGO_CFG_TARGET_ENV").as_deref() != Ok("msvc")
    {
        return;
    }

    let manifest_path = PathBuf::from(
        env::var_os("CARGO_MANIFEST_DIR").expect("Cargo always sets CARGO_MANIFEST_DIR"),
    )
    .join(MANIFEST_FILE);
    println!("cargo:rustc-link-arg-bin=stickymd-win=/MANIFEST:EMBED");
    println!(
        "cargo:rustc-link-arg-bin=stickymd-win=/MANIFESTINPUT:{}",
        manifest_path.display()
    );
}

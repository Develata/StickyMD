use super::*;
use crate::{evidence::escape_json, release::temporary::TemporaryDirectory};
use std::fs;

fn package(id: &str, name: &str, source: &str, directory: &Path) -> String {
    format!(
        r#"{{"id":"{id}","name":"{name}","version":"1.0.0","source":{source},"license":"MIT","repository":null,"homepage":null,"manifest_path":"{}"}}"#,
        escape_json(&directory.join("Cargo.toml").to_string_lossy())
    )
}

fn metadata(directory: &Path) -> String {
    let packages = [
        package("root", "stickymd-win", "null", directory),
        package("z", "z-normal", "\"registry+test\"", directory),
        package("a", "a-transitive", "\"registry+test\"", directory),
        package("build", "build-only", "\"git+unsupported\"", directory),
        package("dev", "dev-only", "\"registry+test\"", directory),
        package("local", "local-lib", "null", directory),
    ]
    .join(",");
    format!(
        r#"{{"packages":[{packages}],"resolve":{{"nodes":[
        {{"id":"root","deps":[{{"pkg":"z","dep_kinds":[{{"kind":null}},{{"kind":"build"}}]}},{{"pkg":"build","dep_kinds":[{{"kind":"build"}}]}},{{"pkg":"dev","dep_kinds":[{{"kind":"dev"}}]}}]}},
        {{"id":"z","deps":[{{"pkg":"local","dep_kinds":[{{"kind":null}}]}}]}},
        {{"id":"local","deps":[{{"pkg":"a","dep_kinds":[{{"kind":null}}]}}]}},
        {{"id":"a","deps":[{{"pkg":"z","dep_kinds":[{{"kind":null}}]}}]}}
    ]}}}}"#
    )
}

#[test]
fn normal_graph_traverses_local_transitive_and_cycles_but_excludes_build_and_dev() {
    let graph = metadata(Path::new("中文 package path"));
    let packages = graph::runtime_packages(&graph).unwrap();
    assert_eq!(
        packages
            .iter()
            .map(|package| package.name.as_str())
            .collect::<Vec<_>>(),
        ["a-transitive", "z-normal"]
    );
    for invalid in [
        graph.replace("registry+test", "git+unsupported"),
        graph.replace("\"pkg\":\"z\"", "\"pkg\":\"missing\""),
        graph.replace("\"kind\":null", "\"kind\":\"unknown\""),
        graph.replace("\"kind\":null", "\"absent\":null"),
        graph.replace("\"id\":\"a\",\"deps\"", "\"id\":\"z\",\"deps\""),
        graph.replace("stickymd-win", "not-the-root"),
        graph.replace("\"kind\":null", "\"kind\":\"build\""),
    ] {
        assert!(graph::runtime_packages(&invalid).is_err(), "{invalid}");
    }
}

#[test]
fn notices_are_stably_sorted_normalized_and_missing_license_information_refuses_output() {
    let scratch = TemporaryDirectory::new("notices-test").unwrap();
    let root = scratch.path();
    let directory = root.join("中文 dependencies with spaces");
    fs::create_dir(&directory).unwrap();
    fs::write(root.join("Cargo.lock"), "locked fixture\n").unwrap();
    fs::write(root.join("THIRD_PARTY_NOTICES.md"), "Header\r\n\r\n").unwrap();
    fs::write(directory.join("NOTICE-Z"), "Notice\r\n ").unwrap();
    fs::write(directory.join("LICENSE-MIT"), "\u{feff}License 中文\r\n").unwrap();
    fs::write(directory.join("LICENSES"), "not selected").unwrap();
    let metadata = metadata(&directory);
    let (output, count) = render(root, &metadata).unwrap();
    assert_eq!(count, 2);
    assert!(!output.contains(['\r', '\u{feff}']));
    assert!(!output.contains("not selected"));
    assert!(
        output.find("PACKAGE: a-transitive").unwrap() < output.find("PACKAGE: z-normal").unwrap()
    );
    assert!(
        output.find("LICENSE FILE: LICENSE-MIT").unwrap()
            < output.find("LICENSE FILE: NOTICE-Z").unwrap()
    );
    let expected = format!(
        "Header\n\n## Generated Rust Runtime Dependency Notices\n\nThis section is generated from the Cargo.lock-resolved normal dependency graph for\nstickymd-win on x86_64-pc-windows-msvc. Build-only and development-only packages are excluded.\nCargo.lock SHA-256: {}\nRuntime registry packages: 2\n",
        integrity::sha256(&root.join("Cargo.lock")).unwrap()
    );
    // The exact body below is independent of metadata enumeration and filesystem creation order.
    assert!(output.starts_with(&expected));
    fs::remove_file(directory.join("NOTICE-Z")).unwrap();
    fs::remove_file(directory.join("LICENSE-MIT")).unwrap();
    assert!(
        render(root, &metadata)
            .unwrap_err()
            .contains("no reviewed fallback")
    );
    let fallback_metadata = metadata
        .replace("a-transitive", "clipboard-win")
        .replace("z-normal", "harfrust");
    assert!(
        render(root, &fallback_metadata)
            .unwrap_err()
            .contains("fallback is missing")
    );
    fs::create_dir_all(root.join("assets/licenses")).unwrap();
    fs::write(root.join("assets/licenses/Boost-1.0.txt"), "Boost").unwrap();
    fs::write(root.join("assets/licenses/HarfRust-MIT.txt"), "HarfRust").unwrap();
    let (output, _) = render(root, &fallback_metadata).unwrap();
    assert!(output.contains("LICENSE FILE: Boost-1.0.txt"));
    fs::write(directory.join("LICENSE"), [0xff]).unwrap();
    assert!(
        render(root, &metadata)
            .unwrap_err()
            .contains("invalid license text")
    );
}

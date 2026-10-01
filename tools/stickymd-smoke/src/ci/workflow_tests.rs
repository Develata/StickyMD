//! Verify the checked-in GitHub adapter preserves the Rust planning protocol.

fn job<'a>(workflow: &'a str, name: &str) -> &'a str {
    let marker = format!("\n  {name}:\n");
    let start = workflow.find(&marker).expect("required CI job") + marker.len();
    let tail = &workflow[start..];
    let end = tail
        .match_indices("\n  ")
        .find_map(|(index, _)| (tail.as_bytes().get(index + 3) != Some(&b' ')).then_some(index))
        .unwrap_or(tail.len());
    &tail[..end]
}

#[test]
fn ci_workflow_wires_all_selected_lanes_and_fails_closed_at_the_aggregate_gate() {
    let workflow = include_str!("../../../../.github/workflows/ci.yml").replace("\r\n", "\n");
    assert!(!workflow.contains("paths-ignore"));
    assert!(!workflow.contains("continue-on-error"));
    let result = job(&workflow, "result");
    assert!(result.contains("if: always()"));
    assert!(result.contains("needs: [plan, linux-smoke, dependency-policy, windows-quality, windows-headless, windows-release, portable-core]"));
    assert!(result.contains("-- ci verify"));
    assert!(result.contains("if: cancelled()"));
    assert!(
        result.contains("CI_CANCELLED: ${{ steps.cancellation.outputs.cancelled || 'false' }}")
    );
    assert!(result.contains("--cancelled=$env:CI_CANCELLED"));
    for (name, field) in [
        ("linux-smoke", "smoke"),
        ("dependency-policy", "dependency"),
        ("windows-quality", "quality"),
        ("windows-headless", "headless"),
        ("windows-release", "release"),
        ("portable-core", "portable"),
    ] {
        let lane = job(&workflow, name);
        assert!(lane.contains("needs: plan"), "{name}");
        assert!(
            lane.contains(&format!("if: needs.plan.outputs.{field}_needed == 'true'")),
            "{name}"
        );
        assert!(result.contains(&format!("needs.{name}.result")), "{name}");
        assert!(
            result.contains(&format!("--{field}=$env:CI_{}", field.to_uppercase())),
            "{name}"
        );
    }
    let plan = job(&workflow, "plan");
    assert!(plan.contains("smoke_needed: ${{ steps.plan.outputs.smoke_needed }}"));
    assert!(plan.contains("'smoke_needed'"));
    assert!(!plan.contains("cargo clippy"));
    assert!(!plan.contains("cargo test"));
    let smoke = job(&workflow, "linux-smoke");
    assert!(smoke.contains("lane: linux-smoke"));
    assert!(smoke.contains("cargo clippy -p stickymd-smoke --all-targets --locked -- -D warnings"));
    assert!(smoke.contains("if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }"));
    assert!(smoke.contains("cargo test -p stickymd-smoke --locked\n          exit $LASTEXITCODE"));
    let headless = job(&workflow, "windows-headless");
    assert!(headless.contains("matrix: ${{ fromJSON(needs.plan.outputs.windows_matrix) }}"));
    assert!(headless.contains("fail-fast: false"));
    assert!(headless.contains("-- all --ci \"--ci-shard=$env:CI_MODE\" --json"));
    assert!(headless.contains("-- modules run $env:CI_MODULE \"--mode=$env:CI_MODE\""));
    assert!(headless.contains("exit $LASTEXITCODE"));
    assert!(headless.contains("$PSNativeCommandUseErrorActionPreference = $false"));
    assert!(headless.contains("2>&1 | Tee-Object -FilePath $log"));
    assert!(headless.contains("if: always()"));
    assert!(headless.contains("${{ runner.temp }}/headless.log"));
    assert!(
        headless
            .contains("headless-${{ matrix.module }}-${{ matrix.mode }}-${{ github.run_attempt }}")
    );
}

#[test]
fn ci_workflow_keeps_manual_scheduled_and_release_complete() {
    let workflow = include_str!("../../../../.github/workflows/ci.yml").replace("\r\n", "\n");
    assert!(workflow.contains("  workflow_dispatch:\n  workflow_call:"));
    let plan = job(&workflow, "plan");
    assert!(plan.contains("fetch-depth: 0"));
    assert!(plan.contains("$selection = @('--full')"));
    assert!(plan.contains("github.event.pull_request.base.sha"));
    assert!(plan.contains("github.event.before"));
    assert!(plan.contains("-- ci plan @selection"));
    assert!(plan.contains("-- phase 00 --json"));
    assert!(
        include_str!("../../../../.github/workflows/scheduled.yml")
            .contains("uses: ./.github/workflows/ci.yml")
    );
    assert!(
        include_str!("../../../../.github/workflows/release.yml")
            .contains("./tools/smoke/all.ps1 -Ci")
    );
}

#[test]
fn ci_cache_is_pinned_and_excludes_candidate_evidence_and_user_documents() {
    let cache = include_str!("../../../../.github/actions/rust-cache/action.yml");
    assert!(!cache.contains("dist/"));
    assert!(!cache.contains("note/"));
    assert!(!cache.contains("cache-hit"));
    for line in cache.lines().filter(|line| line.contains("uses: actions/")) {
        let sha = line
            .split_once('@')
            .unwrap()
            .1
            .split_whitespace()
            .next()
            .unwrap();
        assert_eq!(sha.len(), 40);
        assert!(sha.bytes().all(|byte| byte.is_ascii_hexdigit()));
    }
    assert!(cache.contains("${{ runner.os }}-${{ inputs.lane }}"));
    assert!(cache.contains("${{ github.sha }}"));
}

#[test]
fn cache_downloads_share_keys_but_build_outputs_keep_lane_identity() {
    let cache = include_str!("../../../../.github/actions/rust-cache/action.yml");
    let layers: Vec<_> = cache.split("    - name: ").skip(1).collect();
    assert_eq!(layers.len(), 2);
    let expected_paths = [
        vec![
            "~/.cargo/registry/index/",
            "~/.cargo/registry/cache/",
            "~/.cargo/git/db/",
        ],
        vec![
            "target/debug/",
            "target/release/",
            "target/cargo-deny/",
            "experiments/phase-01/markdown-math/target/",
            "experiments/phase-01/persistence/target/",
        ],
    ];
    for (index, layer) in layers.iter().enumerate() {
        let paths: Vec<_> = layer
            .split_once("path: |")
            .unwrap()
            .1
            .split_once("key:")
            .unwrap()
            .0
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .collect();
        assert_eq!(paths, expected_paths[index]);
        let (key, restore) = layer.split_once("restore-keys: |").unwrap();
        let key = key.split_once("key: ").unwrap().1.trim();
        let restore = restore
            .lines()
            .map(str::trim)
            .find(|s| !s.is_empty())
            .unwrap();
        assert_eq!(key, format!("{restore}${{{{ github.sha }}}}"));
        assert!(key.contains("${{ runner.os }}"));
        for input in [
            "**/Cargo.lock",
            "**/Cargo.toml",
            ".cargo/config.toml",
            ".github/actions/rust-cache/action.yml",
        ] {
            assert!(key.contains(input));
        }
        let lane_a = key.replace("${{ inputs.lane }}", "planner");
        let lane_b = key.replace("${{ inputs.lane }}", "windows-release");
        assert_eq!(lane_a == lane_b, index == 0);
        assert_eq!(key.contains("rust-toolchain.toml"), index == 1);
        assert!(!layer.contains("if:"));
    }
    assert!(!cache.contains("CARGO_INCREMENTAL"));
}

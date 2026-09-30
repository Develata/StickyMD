//! Disposable desktop routing probe, completed before resource sampling starts.
//! plan_ref: docs/plan/11_testing_and_release.md#resource-module-qualification

use super::super::*;

pub(super) fn run(repository: &Path, root: &Path) -> Result<(), String> {
    let source = crate::qualification::release_executable(repository)?;
    let directory = root.join("resource-desktop-probe");
    let executable = copy_executable(&source, &directory)?;
    prepare_resource_layout(&directory, "source", 0, 0, ImageResourceFixture::None)?;
    let mut child = start(&executable)?;
    let outcome = (|| {
        wait_for_layout(&directory)?;
        let window = crate::window_control::visible_window(child.id())?;
        transitions(|mode| {
            ensure_alive(&mut child, "resource desktop probe")?;
            match mode {
                "preview" => crate::window_control::switch_to_preview(window)?,
                "split" => crate::window_control::switch_to_split(window)?,
                _ => crate::window_control::switch_to_source(child.id())?,
            }
            wait_for_view_mode(&directory, mode)
        })?;
        crate::window_control::park_cursor_outside_window(window)
    })();
    stop_child(&mut child);
    outcome.map_err(|error| format!("resource desktop interaction probe failed: {error}"))
}

fn transitions(mut switch: impl FnMut(&str) -> Result<(), String>) -> Result<(), String> {
    for mode in ["preview", "split", "source"] {
        switch(mode)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    #[ignore = "requires an explicitly selected local Release checkout and an exclusive desktop"]
    fn native_resource_desktop_probe() {
        let repository = std::env::var_os("STICKYMD_SMOKE_PROBE_REPOSITORY")
            .expect("set STICKYMD_SMOKE_PROBE_REPOSITORY to a local diagnostic checkout");
        crate::window_control::enable_per_monitor_v2_dpi_awareness().unwrap();
        let root = super::create_smoke_root().unwrap();
        let outcome = super::run(std::path::Path::new(&repository), &root);
        let cleanup = super::cleanup_root(&root);
        outcome.unwrap();
        cleanup.unwrap();
    }

    #[test]
    fn blocked_probe_stops_before_further_input() {
        let mut visited = Vec::new();
        let error = super::transitions(|mode| {
            visited.push(mode.to_owned());
            if mode == "split" {
                Err("occluded".into())
            } else {
                Ok(())
            }
        })
        .unwrap_err();
        assert_eq!(error, "occluded");
        assert_eq!(visited, ["preview", "split"]);
    }
}

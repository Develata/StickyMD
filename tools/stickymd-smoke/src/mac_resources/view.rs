//! Which view each measured run showed (review F7, USER 2026-11-24): the product's opt-in view
//! diagnostic (`apps/stickymd-mac/src/platform/macos/view_report.rs`) writes, on its first
//! complete frame, the view its panes show. `mac-resources` turns it on for every run, reads the
//! report after readiness and before hiding or warming up, and measures only a run whose view is
//! the one its scenario configures; any other run is invalid, never a FAIL.
//! plan_ref: docs/plan/10_performance_reliability.md#macos-gates

/// The product's variable (`view_report::ENV`); a unit test holds it to the product's source.
pub(super) const ENV: &str = "STICKYMD_DIAGNOSTIC_VIEW";
/// The report's name in the run's session directory.
pub(super) const REPORT: &str = "view-report";
/// How long a ready window may take to report its view.
#[cfg(target_os = "macos")]
pub(super) const WAIT_SECONDS: u64 = 5;

/// The view a report names, from exactly the form the product writes:
/// `stickymd-view 1`, `view <source|preview|split>`, `size <width> <height>`.
pub(super) fn parse(text: &str) -> Result<&str, String> {
    let lines: Vec<&str> = text.split('\n').collect();
    let refused = || format!("the view report is not the product's form: {text:?}");
    let ["stickymd-view 1", view, size, ""] = lines.as_slice() else {
        return Err(refused());
    };
    let view = view
        .strip_prefix("view ")
        .filter(|v| matches!(*v, "source" | "preview" | "split"))
        .ok_or_else(refused)?;
    let sizes: Vec<&str> = size
        .strip_prefix("size ")
        .ok_or_else(refused)?
        .split(' ')
        .collect();
    if sizes.len() != 2
        || sizes
            .iter()
            .any(|n| n.is_empty() || n.parse::<u32>().is_err())
    {
        return Err(refused());
    }
    Ok(view)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_products_form_is_read() {
        assert_eq!(
            parse("stickymd-view 1\nview preview\nsize 2080 2720\n"),
            Ok("preview")
        );
        assert_eq!(
            parse("stickymd-view 1\nview split\nsize 1 1\n"),
            Ok("split")
        );
        for bad in [
            "",
            "stickymd-view 1\nview preview\nsize 2080 2720",
            "stickymd-view 2\nview preview\nsize 2080 2720\n",
            "stickymd-view 1\nview hidden\nsize 2080 2720\n",
            "stickymd-view 1\nview preview\nsize 2080\n",
            "stickymd-view 1\nview preview\nsize -1 2720\n",
            "stickymd-view 1\nview preview\nsize 2080 2720\nextra\n",
            "stickymd-view 1\nview  preview\nsize 2080 2720\n",
        ] {
            assert!(parse(bad).is_err(), "{bad:?}");
        }
    }

    /// The variable this tool sets is the one the product reads.
    #[test]
    fn the_variable_is_the_products() {
        let root =
            crate::governance::find_repository_root(&std::env::current_dir().unwrap()).unwrap();
        let source = std::fs::read_to_string(
            root.join("apps/stickymd-mac/src/platform/macos/view_report.rs"),
        )
        .unwrap();
        assert!(source.contains(&format!("pub const ENV: &str = \"{ENV}\";")));
        assert!(source.contains("\"stickymd-view 1\\nview {}\\nsize {width} {height}\\n\""));
    }
}

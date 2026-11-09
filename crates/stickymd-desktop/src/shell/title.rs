//! Lexical guards over the host's title writer and event-pass settlement. The title itself is
//! `crate::title`.
//!
//! plan_ref: docs/plan/09_windows_shell.md#tray-lifecycle

mod tests {
    /// `update_window_title` is the product's only native title write; another `set_title`
    /// would bypass the priority. A lexical guard over the product sources (comments included,
    /// which only makes it stricter).
    #[test]
    fn the_title_has_one_native_writer() {
        // Split so this guard does not count itself.
        let needle = [".set_", "title("].concat();
        let manifest = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let mut pending = vec![
            manifest.join("src"),
            manifest.join("../../apps/stickymd-win/src"),
        ];
        let mut writers = Vec::new();
        while let Some(path) = pending.pop() {
            if path.is_dir() {
                for entry in std::fs::read_dir(&path).unwrap() {
                    pending.push(entry.unwrap().path());
                }
            } else if path.extension().is_some_and(|extension| extension == "rs") {
                let text = std::fs::read_to_string(&path).unwrap();
                let count = text.matches(needle.as_str()).count();
                if count > 0 {
                    writers.push((
                        path.file_name().unwrap().to_string_lossy().into_owned(),
                        count,
                    ));
                }
            }
        }
        assert_eq!(writers, [("persistence_runtime.rs".to_owned(), 1)]);
    }

    /// The notice settles once per complete main-thread event pass: `finish_event_pass` is
    /// called from exactly one place in the pass settlement after the window tick.
    /// Winit's `about_to_wait` (`handler.rs`) delegates to that settlement once (a lexical guard,
    /// supplemented by lifecycle runtime tests of actual calls and state).
    #[test]
    fn the_notice_settles_at_the_end_of_each_event_pass() {
        let call = ["finish_event", "_pass();"].concat();
        let source = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut pending = vec![source];
        let mut calls = Vec::new();
        while let Some(path) = pending.pop() {
            if path.is_dir() {
                for entry in std::fs::read_dir(&path).unwrap() {
                    pending.push(entry.unwrap().path());
                }
            } else if path.extension().is_some_and(|extension| extension == "rs") {
                let text = std::fs::read_to_string(&path).unwrap();
                for (offset, _) in text.match_indices(call.as_str()) {
                    calls.push((path.clone(), offset, text.clone()));
                }
            }
        }
        assert_eq!(
            calls.len(),
            1,
            "{:?}",
            calls.iter().map(|call| &call.0).collect::<Vec<_>>()
        );
        let (path, offset, text) = &calls[0];
        assert!(path.ends_with("lifecycle.rs"), "{path:?}");
        let pass = text
            .find("fn settle_event_pass(")
            .expect("settle_event_pass");
        let end = text[pass + 1..]
            .find("\n    fn ")
            .map_or(text.len(), |next| pass + 1 + next);
        let tick = text[pass..end]
            .find("app.settle(now_ms, out)")
            .expect("the session's ticks in settle_event_pass");
        assert!(
            (pass + tick..end).contains(offset),
            "finish_event_pass runs in settle_event_pass after the session's ticks"
        );
        let handler = std::fs::read_to_string(path.with_file_name("handler.rs")).unwrap();
        let native_pass = handler
            .split("fn about_to_wait(")
            .nth(1)
            .unwrap()
            .split("fn user_event(")
            .next()
            .unwrap();
        assert_eq!(
            native_pass
                .matches("self.settle_event_pass(now_ms);")
                .count(),
            1
        );
    }
}

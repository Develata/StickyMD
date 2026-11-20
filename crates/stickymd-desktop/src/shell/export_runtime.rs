//! Export intent coordination for the native shell.
//!
//! plan_ref: docs/plan/08_assets_and_export.md#export

use super::DesktopShell;
use crate::host::Host;
use stickymd_app::flow::notice::Notice;

impl<H: Host> DesktopShell<H> {
    pub(super) fn request_export(&mut self) {
        let mut free = false;
        self.with_session(|app, _, out| free = app.begin_export(out));
        if !free {
            return;
        }
        let target = match self.host.choose_markdown_export() {
            Ok(Some(target)) => target,
            Ok(None) => return,
            Err(error) => {
                self.notify(Notice::ExportPickerFailed(error.to_string()));
                return;
            }
        };
        self.with_session(|app, _, out| app.submit_export(target, out));
    }
}

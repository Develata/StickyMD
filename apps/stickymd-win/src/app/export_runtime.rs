//! Export intent coordination for the native shell.
//!
//! plan_ref: docs/plan/08_assets_and_export.md#export

use super::StickyApp;
use crate::export::{ExportCompletion, ExportRequest};
use crate::platform::windows::export_dialog::choose_markdown_export;

impl StickyApp {
    pub(super) fn request_export(&mut self) {
        if self.export_in_flight {
            self.diagnostic = Some("已有导出任务正在进行。".into());
            self.request_redraw();
            return;
        }
        let target = match choose_markdown_export() {
            Ok(Some(target)) => target,
            Ok(None) => return,
            Err(error) => {
                self.diagnostic = Some(format!("无法打开导出位置选择器：{error}"));
                self.request_redraw();
                return;
            }
        };
        let request = ExportRequest {
            snapshot: self.coordinator.snapshot(),
            note_dir: self.paths.note_dir.clone(),
            target,
        };
        if self.worker.submit_export(request) {
            self.export_in_flight = true;
            self.diagnostic = Some("正在导出当前快照…".into());
        } else {
            self.diagnostic = Some("导出队列暂时不可用。".into());
        }
        self.request_redraw();
    }

    pub(super) fn handle_export_completion(
        &mut self,
        _generation: stickymd_core::Generation,
        result: Result<ExportCompletion, crate::export::ExportError>,
    ) {
        self.export_in_flight = false;
        self.diagnostic = Some(match result {
            Ok(completion) => format!(
                "导出完成：{}（{} 个本地资源）",
                completion.target.display(),
                completion.copied_assets
            ),
            Err(error) => format!("导出失败；工作文档未改变：{error}"),
        });
        self.request_redraw();
    }
}

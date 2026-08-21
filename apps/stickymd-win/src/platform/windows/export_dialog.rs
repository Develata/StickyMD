//! Native Windows Markdown export destination picker.
//!
//! plan_ref: docs/plan/09_windows_shell.md#windows-adapter-mapping

use std::path::PathBuf;

use thiserror::Error;
use windows::Win32::System::Com::{
    CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx,
    CoTaskMemFree, CoUninitialize,
};
use windows::Win32::UI::Shell::Common::COMDLG_FILTERSPEC;
use windows::Win32::UI::Shell::{FileSaveDialog, IFileSaveDialog, SIGDN_FILESYSPATH};
use windows::core::{HRESULT, w};

const ERROR_CANCELLED_HRESULT: HRESULT = HRESULT(0x8007_04c7_u32 as i32);
const RPC_E_CHANGED_MODE: HRESULT = HRESULT(0x8001_0106_u32 as i32);

#[derive(Debug, Error)]
pub enum ExportDialogError {
    #[error("cannot initialize the Windows dialog apartment: {0}")]
    Initialize(windows::core::Error),
    #[error("cannot create or operate the Windows save dialog: {0}")]
    Dialog(windows::core::Error),
    #[error("the selected export path is not valid Unicode: {0}")]
    Path(std::string::FromUtf16Error),
}

struct ApartmentGuard(bool);

impl Drop for ApartmentGuard {
    fn drop(&mut self) {
        if self.0 {
            // SAFETY: this balances the successful CoInitializeEx call made on
            // the same UI thread in `choose_markdown_export`.
            unsafe { CoUninitialize() };
        }
    }
}

/// Show the native save picker. Cancellation is a normal `Ok(None)` result.
pub fn choose_markdown_export() -> Result<Option<PathBuf>, ExportDialogError> {
    // SAFETY: the reserved pointer is null and apartment initialization is
    // performed and balanced on this UI thread. An existing apartment with a
    // different model may still use the already-initialized COM environment.
    let initialized = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
    let guard = if initialized.is_ok() {
        ApartmentGuard(true)
    } else if initialized == RPC_E_CHANGED_MODE {
        ApartmentGuard(false)
    } else {
        return Err(ExportDialogError::Initialize(
            windows::core::Error::from_hresult(initialized),
        ));
    };

    // SAFETY: COM is initialized on this thread, no outer aggregation object is
    // supplied, and the requested interface matches the FileSaveDialog class.
    let dialog: IFileSaveDialog =
        unsafe { CoCreateInstance(&FileSaveDialog, None, CLSCTX_INPROC_SERVER) }
            .map_err(ExportDialogError::Dialog)?;
    let filters = [COMDLG_FILTERSPEC {
        pszName: w!("Markdown (*.md)"),
        pszSpec: w!("*.md"),
    }];
    // SAFETY: filter and static UTF-16 strings remain live for each synchronous
    // call; the dialog owns any state it retains afterward.
    unsafe {
        dialog
            .SetFileTypes(&filters)
            .map_err(ExportDialogError::Dialog)?;
        dialog
            .SetDefaultExtension(w!("md"))
            .map_err(ExportDialogError::Dialog)?;
        dialog
            .SetFileName(w!("note.md"))
            .map_err(ExportDialogError::Dialog)?;
        dialog
            .SetTitle(w!("导出"))
            .map_err(ExportDialogError::Dialog)?;
        dialog
            .SetOkButtonLabel(w!("导出"))
            .map_err(ExportDialogError::Dialog)?;
    }
    // SAFETY: no owner handle is supplied; Show is synchronous and the dialog
    // object remains alive for the duration of the call.
    if let Err(error) = unsafe { dialog.Show(None) } {
        drop(guard);
        return if error.code() == ERROR_CANCELLED_HRESULT {
            Ok(None)
        } else {
            Err(ExportDialogError::Dialog(error))
        };
    }
    // SAFETY: GetResult returns an owned COM interface. GetDisplayName returns
    // CoTaskMem-allocated UTF-16 which remains valid until explicitly freed.
    let item = unsafe { dialog.GetResult() }.map_err(ExportDialogError::Dialog)?;
    let display =
        unsafe { item.GetDisplayName(SIGDN_FILESYSPATH) }.map_err(ExportDialogError::Dialog)?;
    // SAFETY: `display` is a valid NUL-terminated allocation returned by the
    // shell item and is read before its matching CoTaskMemFree.
    let path = unsafe { display.to_string() }.map_err(ExportDialogError::Path);
    // SAFETY: the pointer was allocated by the Shell with CoTaskMem and is freed
    // exactly once after conversion; no later access occurs.
    unsafe { CoTaskMemFree(Some(display.0.cast())) };
    drop(guard);
    path.map(|value| Some(PathBuf::from(value)))
}

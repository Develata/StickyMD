//! An owned native cover proves sensor polling without pointer-message assistance.
//!
//! plan_ref: docs/plan/09_windows_shell.md#dock-auto-hide

use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
    mpsc,
};
use std::thread;
use std::time::{Duration, Instant};

use super::{NativePoint, WindowFromPoint, WindowHandle, primary_work_area, window_rect};

#[repr(C)]
#[derive(Default)]
struct Message {
    window: isize,
    message: u32,
    wparam: usize,
    lparam: isize,
    time: u32,
    point: NativePoint,
    private: u32,
}

#[link(name = "user32")]
unsafe extern "system" {
    fn CreateWindowExW(
        ex: u32,
        class: *const u16,
        title: *const u16,
        style: u32,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
        parent: isize,
        menu: isize,
        instance: isize,
        param: *const std::ffi::c_void,
    ) -> isize;
    fn DestroyWindow(window: isize) -> i32;
    fn PeekMessageW(message: *mut Message, window: isize, min: u32, max: u32, remove: u32) -> i32;
    fn TranslateMessage(message: *const Message) -> i32;
    fn DispatchMessageW(message: *const Message) -> isize;
}

struct Cover {
    hwnd: isize,
    stop: Arc<AtomicBool>,
    done: mpsc::Receiver<Result<(), String>>,
    closed: bool,
}

impl Cover {
    fn new(x: i32, y: i32) -> Result<Self, String> {
        let stop = Arc::new(AtomicBool::new(false));
        let worker_stop = Arc::clone(&stop);
        let (created_tx, created_rx) = mpsc::sync_channel(1);
        let (done_tx, done) = mpsc::sync_channel(1);
        thread::spawn(move || {
            let class: Vec<u16> = "STATIC\0".encode_utf16().collect();
            let title: Vec<u16> = "StickyMD owned sensor cover\0".encode_utf16().collect();
            // SAFETY: built-in STATIC class, live UTF-16 buffers, no parent/menu
            // or retained caller pointer. This thread owns and destroys the HWND.
            let hwnd = unsafe {
                CreateWindowExW(
                    0x0800_0088,
                    class.as_ptr(),
                    title.as_ptr(),
                    0x9000_0000,
                    x,
                    y,
                    16,
                    32,
                    0,
                    0,
                    0,
                    std::ptr::null(),
                )
            };
            if hwnd == 0 {
                let error = format!(
                    "cannot create sensor cover: {}",
                    std::io::Error::last_os_error()
                );
                let _ = created_tx.send(Err(error.clone()));
                let _ = done_tx.send(Err(error));
                return;
            }
            let _ = created_tx.send(Ok(hwnd));
            let deadline = Instant::now() + Duration::from_secs(20);
            while !worker_stop.load(Ordering::Acquire) && Instant::now() < deadline {
                let mut message = Message::default();
                // SAFETY: MSG has the Win32 x64 layout; APIs synchronously read
                // or write it. Only this thread's ordinary messages are pumped.
                unsafe {
                    for _ in 0..64 {
                        if PeekMessageW(&raw mut message, 0, 0, 0, 1) == 0 {
                            break;
                        }
                        TranslateMessage(&raw const message);
                        DispatchMessageW(&raw const message);
                    }
                }
                thread::sleep(Duration::from_millis(5));
            }
            // SAFETY: the creator thread destroys only its own STATIC window.
            let result = if unsafe { DestroyWindow(hwnd) } != 0 {
                Ok(())
            } else {
                Err(format!(
                    "cannot destroy sensor cover: {}",
                    std::io::Error::last_os_error()
                ))
            };
            let _ = done_tx.send(result);
        });
        let hwnd = match created_rx.recv_timeout(Duration::from_secs(2)) {
            Ok(Ok(hwnd)) => hwnd,
            result => {
                stop.store(true, Ordering::Release);
                let _ = done.recv_timeout(Duration::from_secs(2));
                return Err(format!("sensor cover startup failed: {result:?}"));
            }
        };
        Ok(Self {
            hwnd,
            stop,
            done,
            closed: false,
        })
    }

    fn close(&mut self) -> Result<(), String> {
        self.stop.store(true, Ordering::Release);
        self.closed = true;
        self.done
            .recv_timeout(Duration::from_secs(2))
            .map_err(|error| format!("sensor cover teardown timed out: {error}"))?
    }
}

impl Drop for Cover {
    fn drop(&mut self) {
        if !self.closed {
            let _ = self.close();
        }
    }
}

pub(crate) fn hover(
    hwnd: WindowHandle,
    revealed: impl FnOnce() -> Result<(), String>,
) -> Result<(), String> {
    let rect = window_rect(hwnd)?;
    let work = primary_work_area()?;
    let point = NativePoint {
        x: work.x + 1,
        y: rect.y + (rect.height / 2) as i32,
    };
    super::physical_input::move_physical_cursor(
        work.x + (work.width - 16) as i32,
        point.y,
        "leave covered sensor",
    )?;
    let mut cover = Cover::new(work.x, point.y - 16)?;
    let observed = (|| {
        // The point must route to our opaque cover before the real hover.
        thread::sleep(Duration::from_millis(100));
        // SAFETY: WindowFromPoint takes a copied coordinate and borrows its result.
        if unsafe { WindowFromPoint(point) } != cover.hwnd {
            return Err("owned cover did not obscure the collapsed sensor".to_owned());
        }
        super::physical_input::move_physical_cursor(point.x, point.y, "hover the covered sensor")?;
        revealed()
    })();
    let cleanup = cover.close();
    match (observed, cleanup) {
        (Ok(()), Ok(())) => Ok(()),
        (Err(error), Ok(())) | (Ok(()), Err(error)) => Err(error),
        (Err(error), Err(cleanup)) => Err(format!("{error}; {cleanup}")),
    }
}

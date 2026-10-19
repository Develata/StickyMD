//! `SendInput`, `SetCursorPos` and `GetAsyncKeyState` behind `Backend`; compiled only into builds
//! that unwind.
//!
//! plan_ref: docs/plan/11_testing_and_release.md#physical-input-injection

use std::thread;
use std::time::{Duration, Instant};

use super::{Backend, Button, Input};

const INPUT_MOUSE: u32 = 0;
const INPUT_KEYBOARD: u32 = 1;
const KEYEVENTF_EXTENDEDKEY: u32 = 0x0001;
const KEYEVENTF_KEYUP: u32 = 0x0002;
const MOUSEEVENTF_MOVE: u32 = 0x0001;
const MOUSEEVENTF_LEFTDOWN: u32 = 0x0002;
const MOUSEEVENTF_LEFTUP: u32 = 0x0004;
const MOUSEEVENTF_RIGHTDOWN: u32 = 0x0008;
const MOUSEEVENTF_RIGHTUP: u32 = 0x0010;
const MOUSEEVENTF_WHEEL: u32 = 0x0800;
const MOUSEEVENTF_VIRTUALDESK: u32 = 0x4000;
const MOUSEEVENTF_ABSOLUTE: u32 = 0x8000;
const VK_LBUTTON: i32 = 0x01;

#[repr(C)]
#[derive(Clone, Copy)]
struct MouseInput {
    dx: i32,
    dy: i32,
    mouse_data: u32,
    flags: u32,
    time: u32,
    extra_info: usize,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct KeyboardInput {
    virtual_key: u16,
    scan_code: u16,
    flags: u32,
    time: u32,
    extra_info: usize,
}

#[repr(C)]
#[derive(Clone, Copy)]
union InputData {
    mouse: MouseInput,
    keyboard: KeyboardInput,
}

/// `INPUT`; the union is as large as its largest member, `MOUSEINPUT`.
#[repr(C)]
#[derive(Clone, Copy)]
struct NativeInput {
    input_type: u32,
    data: InputData,
}

#[link(name = "user32")]
unsafe extern "system" {
    fn SendInput(count: u32, inputs: *const NativeInput, size: i32) -> u32;
    fn SetCursorPos(x: i32, y: i32) -> i32;
    fn GetAsyncKeyState(virtual_key: i32) -> i16;
}

pub(crate) struct SendInputBackend;

fn native(input: Input) -> NativeInput {
    let mouse = |dx, dy, mouse_data, flags| NativeInput {
        input_type: INPUT_MOUSE,
        data: InputData {
            mouse: MouseInput {
                dx,
                dy,
                mouse_data,
                flags,
                time: 0,
                extra_info: 0,
            },
        },
    };
    match input {
        Input::Key { key, up } => NativeInput {
            input_type: INPUT_KEYBOARD,
            data: InputData {
                keyboard: KeyboardInput {
                    virtual_key: key.virtual_key,
                    scan_code: key.scan_code,
                    flags: if key.extended {
                        KEYEVENTF_EXTENDEDKEY
                    } else {
                        0
                    } | if up { KEYEVENTF_KEYUP } else { 0 },
                    time: 0,
                    extra_info: 0,
                },
            },
        },
        Input::Button { button, up } => mouse(
            0,
            0,
            0,
            match (button, up) {
                (Button::Left, false) => MOUSEEVENTF_LEFTDOWN,
                (Button::Left, true) => MOUSEEVENTF_LEFTUP,
                (Button::Right, false) => MOUSEEVENTF_RIGHTDOWN,
                (Button::Right, true) => MOUSEEVENTF_RIGHTUP,
            },
        ),
        Input::Move { x, y } => mouse(
            x,
            y,
            0,
            MOUSEEVENTF_MOVE | MOUSEEVENTF_ABSOLUTE | MOUSEEVENTF_VIRTUALDESK,
        ),
        // A negative wheel delta is the two's-complement DWORD Windows expects.
        Input::Wheel { delta } => mouse(0, 0, delta as u32, MOUSEEVENTF_WHEEL),
    }
}

impl Backend for SendInputBackend {
    fn send(&mut self, inputs: &[Input]) -> usize {
        let natives = inputs.iter().copied().map(native).collect::<Vec<_>>();
        let (Ok(count), Ok(size)) = (
            u32::try_from(natives.len()),
            i32::try_from(std::mem::size_of::<NativeInput>()),
        ) else {
            return 0;
        };
        // SAFETY: `natives` holds `count` initialized INPUT values of `size` bytes each and
        // stays alive for the synchronous call; SendInput copies them and keeps no pointer.
        let inserted = unsafe { SendInput(count, natives.as_ptr(), size) };
        usize::try_from(inserted).unwrap_or(0)
    }

    fn place_cursor(&mut self, x: i32, y: i32) -> bool {
        // SAFETY: SetCursorPos consumes two copied coordinates and retains nothing.
        unsafe { SetCursorPos(x, y) != 0 }
    }

    fn left_button_down(&mut self) -> bool {
        // SAFETY: VK_LBUTTON is a copied virtual-key identifier; the query returns
        // process-independent input state and retains no resource.
        unsafe { GetAsyncKeyState(VK_LBUTTON) < 0 }
    }

    fn sleep(&mut self, duration: Duration) {
        thread::sleep(duration);
    }

    fn now(&mut self) -> Instant {
        Instant::now()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input_injection::Key;

    #[test]
    #[cfg(target_pointer_width = "64")]
    fn the_input_layout_is_the_windows_x64_layout() {
        assert_eq!(std::mem::size_of::<NativeInput>(), 40);
        assert_eq!(std::mem::align_of::<NativeInput>(), 8);
    }

    #[test]
    fn events_map_to_their_flags() {
        let key = native(Input::Key {
            key: Key {
                virtual_key: 0x2D,
                scan_code: 0x52,
                extended: true,
            },
            up: true,
        });
        assert_eq!(key.input_type, INPUT_KEYBOARD);
        // SAFETY: the value was built as the keyboard variant just above.
        let keyboard = unsafe { key.data.keyboard };
        assert_eq!(
            (keyboard.virtual_key, keyboard.scan_code, keyboard.flags),
            (0x2D, 0x52, KEYEVENTF_EXTENDEDKEY | KEYEVENTF_KEYUP)
        );
        let right_down = native(Input::Button {
            button: Button::Right,
            up: false,
        });
        assert_eq!(right_down.input_type, INPUT_MOUSE);
        // SAFETY: built as the mouse variant just above.
        assert_eq!(
            unsafe { right_down.data.mouse }.flags,
            MOUSEEVENTF_RIGHTDOWN
        );
        // SAFETY: built as the mouse variant.
        let wheel = unsafe { native(Input::Wheel { delta: -120 }).data.mouse };
        assert_eq!(
            (wheel.flags, wheel.mouse_data),
            (MOUSEEVENTF_WHEEL, (-120_i32) as u32)
        );
    }
}

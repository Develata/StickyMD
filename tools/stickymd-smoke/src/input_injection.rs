//! The only path by which the smoke CLI injects keyboard and mouse input into the real desktop,
//! or places its cursor.
//!
//! Every press is registered to the operation that makes it before it is injected, and each
//! operation releases only its own presses, in reverse order: explicitly on the normal path, by
//! the left-button guard's `Drop`, or by the operation's unwind guard. Each `SendInput` batch is
//! judged by its insertion count: a short count fails, a full count only proves the events
//! entered the input stream. A failed release, an unwound operation or a lock poisoned by one
//! stops all later injection in the process (shared stop), because input the desktop may still
//! hold down would corrupt every later case; releasing what is already registered stays allowed.
//! Builds that cannot unwind refuse to inject at all: an abort cannot run the `Drop` that
//! releases a press.
//!
//! plan_ref: docs/plan/11_testing_and_release.md#physical-input-injection

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::{Mutex, MutexGuard};
use std::time::{Duration, Instant};

#[cfg(all(windows, panic = "unwind"))]
mod win32;

/// How long a released left button may still read down before the release counts as failed.
const LEFT_BUTTON_SETTLE: Duration = Duration::from_millis(500);
const LEFT_BUTTON_SAMPLE: Duration = Duration::from_millis(5);
/// The most samples one wait takes, also when the clock does not advance.
const LEFT_BUTTON_SAMPLES: u32 = 101;

/// A key as `SendInput` names it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Key {
    pub(crate) virtual_key: u16,
    pub(crate) scan_code: u16,
    pub(crate) extended: bool,
}

impl Key {
    pub(crate) const fn new(virtual_key: u16, scan_code: u16) -> Self {
        Self {
            virtual_key,
            scan_code,
            extended: false,
        }
    }
}

pub(crate) const CONTROL: Key = Key::new(0x11, 0x1D);
pub(crate) const SHIFT: Key = Key::new(0x10, 0x2A);
pub(crate) const ESCAPE: Key = Key::new(0x1B, 0x01);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Button {
    Left,
    Right,
}

/// One event for the input stream.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Input {
    Key {
        key: Key,
        up: bool,
    },
    Button {
        button: Button,
        up: bool,
    },
    /// An absolute move to virtual-desktop coordinates normalized to `0..=65535`.
    Move {
        x: i32,
        y: i32,
    },
    Wheel {
        delta: i32,
    },
}

/// What the input stream, the cursor and the button state are, for real or in a test.
pub(crate) trait Backend: Send {
    /// Inserts `inputs` in order; returns how many were inserted.
    fn send(&mut self, inputs: &[Input]) -> usize;
    /// Places the cursor at virtual-desktop pixel `(x, y)`; whether Windows accepted it.
    fn place_cursor(&mut self, x: i32, y: i32) -> bool;
    /// Whether the left button reads down now. Reading up is also what an unreadable state
    /// returns, so it never proves a release.
    fn left_button_down(&mut self) -> bool;
    fn sleep(&mut self, duration: Duration);
    /// The monotonic time `sleep` advances.
    fn now(&mut self) -> Instant;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Held {
    Key(Key),
    Button(Button),
}

impl Held {
    const fn input(self, up: bool) -> Input {
        match self {
            Self::Key(key) => Input::Key { key, up },
            Self::Button(button) => Input::Button { button, up },
        }
    }
}

/// An operation's identity; an operation releases only the presses registered to it.
type Operation = u64;

struct State<B> {
    backend: B,
    /// Presses registered and not yet released, with their operation, in press order.
    held: Vec<(Operation, Held)>,
    last_operation: Operation,
    stopped: Option<String>,
}

impl<B: Backend> State<B> {
    fn ensure_running(&self) -> Result<(), String> {
        match &self.stopped {
            None => Ok(()),
            Some(reason) => Err(format!("input injection stopped: {reason}")),
        }
    }

    /// Stops all later injection, keeping the first reason; returns `reason`.
    fn stop(&mut self, reason: String) -> String {
        self.stopped.get_or_insert_with(|| reason.clone());
        reason
    }

    fn begin(&mut self) -> Result<Operation, String> {
        self.ensure_running()?;
        self.last_operation += 1;
        Ok(self.last_operation)
    }

    fn send_one(&mut self, input: Input) -> Result<(), String> {
        match self.backend.send(&[input]) {
            1 => Ok(()),
            inserted => Err(format!("inserted {inserted}/1 of {input:?}")),
        }
    }

    /// Registers `held` to `operation`, then injects its press. A key or button already held
    /// by any operation is not pressed again: Windows tracks one state per key, so a second
    /// holder's release would end the first's hold.
    fn press(&mut self, operation: Operation, held: Held) -> Result<(), String> {
        self.ensure_running()?;
        if self.held.iter().any(|(_, entry)| *entry == held) {
            return Err(format!("{held:?} is already held by another operation"));
        }
        self.held.push((operation, held));
        if let Err(error) = self.send_one(held.input(false)) {
            // The press may still have reached the stream; release it like any other.
            let release = self.release(operation, held);
            return Err(match release {
                Ok(()) => format!("cannot press {held:?}: {error}"),
                Err(release) => format!("cannot press {held:?}: {error}; {release}"),
            });
        }
        Ok(())
    }

    /// Releases `operation`'s registered `held`, nothing when it holds none. The registration
    /// stays until the release is judged, so an unwind in between still finds it. Any failure
    /// stops all later injection.
    fn release(&mut self, operation: Operation, held: Held) -> Result<(), String> {
        let Some(index) = self
            .held
            .iter()
            .rposition(|entry| *entry == (operation, held))
        else {
            return Ok(());
        };
        let result = self.send_one(held.input(true)).and_then(|()| {
            if held == Held::Button(Button::Left) {
                self.wait_left_button(false)
            } else {
                Ok(())
            }
        });
        self.held.remove(index);
        result.map_err(|error| self.stop(format!("release of {held:?} failed: {error}")))
    }

    /// Releases `operation`'s presses, last pressed first; returns the first failure.
    fn release_operation(&mut self, operation: Operation) -> Result<(), String> {
        let mut first = Ok(());
        while let Some(&(_, held)) = self
            .held
            .iter()
            .rev()
            .find(|(owner, _)| *owner == operation)
        {
            if let Err(error) = self.release(operation, held)
                && first.is_ok()
            {
                first = Err(error);
            }
        }
        first
    }

    /// Releases `operation`'s `held` with any panic caught, for destructors running while
    /// unwinding: a panic leaving one aborts the process before any further release. A caught
    /// panic stops injection and drops the registration, so each release is attempted once.
    fn release_caught(&mut self, operation: Operation, held: Held) {
        if let Err(payload) = catch_unwind(AssertUnwindSafe(|| self.release(operation, held))) {
            // Dropping a panic payload runs its destructor, which can panic in turn, out of
            // the destructor running this. The payload is leaked instead, once per release.
            std::mem::forget(payload);
            self.stop(format!("the release of {held:?} panicked"));
            if let Some(index) = self
                .held
                .iter()
                .rposition(|entry| *entry == (operation, held))
            {
                self.held.remove(index);
            }
        }
    }

    /// Stops injection for `reason` and releases the presses of `operation`, last pressed
    /// first; a release that panics again does not keep the others from running.
    fn release_unwinding(&mut self, operation: Operation, reason: &str) {
        self.stop(reason.to_owned());
        while let Some(index) = self.held.iter().rposition(|(owner, _)| *owner == operation) {
            let held = self.held[index].1;
            self.release_caught(operation, held);
        }
    }

    /// Waits at most `LEFT_BUTTON_SETTLE`, by the backend's clock, for the left button to read
    /// `down`; still reading otherwise at the deadline, or after `LEFT_BUTTON_SAMPLES` samples,
    /// fails. A reading taken past the deadline does not count, whatever it reads; one taken at
    /// it counts, and reading otherwise there fails at once. Right after
    /// a release the button may still read down while Windows processes the input (USER
    /// decision 2026-10-19). Reading up shows only that nothing reads down (an unreadable state
    /// reads up as well), not that the target received the release.
    fn wait_left_button(&mut self, down: bool) -> Result<(), String> {
        let start = self.backend.now();
        for _ in 0..LEFT_BUTTON_SAMPLES {
            let reads_down = self.backend.left_button_down();
            let elapsed = self.backend.now().saturating_duration_since(start);
            let Some(remaining) = LEFT_BUTTON_SETTLE.checked_sub(elapsed) else {
                break;
            };
            if reads_down == down {
                return Ok(());
            }
            if remaining.is_zero() {
                break;
            }
            self.backend.sleep(LEFT_BUTTON_SAMPLE.min(remaining));
        }
        Err(format!(
            "the left button did not read {} within {LEFT_BUTTON_SETTLE:?}",
            if down { "down" } else { "up" }
        ))
    }
}

/// The lock held for one operation, or for a left-button guard's release. If it unwinds, the
/// outcome is unknown: the guard stops all later injection and releases what the operation
/// pressed.
struct Guarded<'a, B: Backend> {
    state: MutexGuard<'a, State<B>>,
    operation: Operation,
}

impl<B: Backend> Drop for Guarded<'_, B> {
    fn drop(&mut self) {
        if std::thread::panicking() {
            self.state
                .release_unwinding(self.operation, "an injection unwound before it finished");
        }
    }
}

/// An injector over one backend. The process has one live injector (`live`); tests make their
/// own over a scripted backend.
pub(crate) struct Injection<B> {
    state: Mutex<State<B>>,
}

impl<B: Backend> Injection<B> {
    #[cfg_attr(
        not(any(test, all(windows, panic = "unwind"))),
        expect(
            dead_code,
            reason = "a build that refuses injection constructs no injector"
        )
    )]
    pub(crate) fn new(backend: B) -> Self {
        Self {
            state: Mutex::new(State {
                backend,
                held: Vec::new(),
                last_operation: 0,
                stopped: None,
            }),
        }
    }

    /// The state. A lock a panic poisoned leaves injection stopped; the panicking operation's
    /// guard has already released what it pressed.
    fn lock(&self) -> MutexGuard<'_, State<B>> {
        match self.state.lock() {
            Ok(state) => state,
            Err(poisoned) => {
                let mut state = poisoned.into_inner();
                state.stop("a panic interrupted an injection".to_owned());
                state
            }
        }
    }

    fn operation(&self) -> Result<Guarded<'_, B>, String> {
        let mut state = self.lock();
        let operation = state.begin()?;
        Ok(Guarded { state, operation })
    }

    /// Why injection stopped, if it did.
    pub(crate) fn stop_reason(&self) -> Option<String> {
        self.lock().stopped.clone()
    }

    /// Fails once injection has stopped. Synthetic pointer messages to a window are not
    /// injected input, but they follow the stop too: with a key possibly held down they would
    /// no longer mean what a case intends.
    pub(crate) fn require_running(&self) -> Result<(), String> {
        self.lock().ensure_running()
    }

    /// Presses `modifiers` in order and `key`, then releases `key` and the modifiers in reverse.
    pub(crate) fn chord(&self, modifiers: &[Key], key: Key) -> Result<(), String> {
        let mut guarded = self.operation()?;
        let operation = guarded.operation;
        let pressed = modifiers
            .iter()
            .chain([&key])
            .try_for_each(|&each| guarded.state.press(operation, Held::Key(each)));
        let released = guarded.state.release_operation(operation);
        pressed.and(released)
    }

    pub(crate) fn tap(&self, key: Key) -> Result<(), String> {
        self.chord(&[], key)
    }

    /// Repeated key-downs under one ownership registration, followed by one
    /// release. No other operation can borrow or release this held key.
    pub(crate) fn repeat_key(&self, key: Key, presses: u8) -> Result<(), String> {
        if !(2..=16).contains(&presses) {
            return Err("repeated key requires 2..=16 presses".to_owned());
        }
        let mut guarded = self.operation()?;
        let operation = guarded.operation;
        let pressed = guarded
            .state
            .press(operation, Held::Key(key))
            .and_then(|()| {
                (1..presses).try_for_each(|_| {
                    guarded.state.backend.sleep(Duration::from_millis(25));
                    guarded.state.send_one(Held::Key(key).input(false))
                })
            });
        let released = guarded.state.release_operation(operation);
        pressed.and(released)
    }

    /// Presses the left button and waits for it to read down; the guard releases it.
    pub(crate) fn press_left(&self) -> Result<LeftButton<'_, B>, String> {
        let mut guarded = self.operation()?;
        let operation = guarded.operation;
        guarded.state.press(operation, Held::Button(Button::Left))?;
        if let Err(error) = guarded.state.wait_left_button(true) {
            // It may have been pressed between samples; release it before failing.
            let released = guarded.state.release(operation, Held::Button(Button::Left));
            return Err(match released {
                Ok(()) => error,
                Err(release) => format!("{error}; {release}"),
            });
        }
        drop(guarded);
        Ok(LeftButton {
            injection: self,
            operation,
            released: false,
        })
    }

    /// An unconditional left-button release, for input an interrupted earlier run may have
    /// left down.
    pub(crate) fn release_left(&self) -> Result<(), String> {
        let mut guarded = self.operation()?;
        let operation = guarded.operation;
        if guarded
            .state
            .held
            .iter()
            .any(|(_, held)| *held == Held::Button(Button::Left))
        {
            return Err(
                "refusing a recovery release while an operation holds the left button".to_owned(),
            );
        }
        guarded
            .state
            .held
            .push((operation, Held::Button(Button::Left)));
        guarded.state.release(operation, Held::Button(Button::Left))
    }

    pub(crate) fn click_right(&self) -> Result<(), String> {
        let mut guarded = self.operation()?;
        let operation = guarded.operation;
        let pressed = guarded.state.press(operation, Held::Button(Button::Right));
        let released = guarded.state.release_operation(operation);
        pressed.and(released)
    }

    pub(crate) fn move_absolute(&self, x: i32, y: i32) -> Result<(), String> {
        self.operation()?.state.send_one(Input::Move { x, y })
    }

    pub(crate) fn wheel(&self, delta: i32) -> Result<(), String> {
        self.operation()?.state.send_one(Input::Wheel { delta })
    }

    /// Keeps modifiers owned until the wheel event, releasing them on every exit.
    pub(crate) fn modified_wheel(&self, modifiers: &[Key], delta: i32) -> Result<(), String> {
        let mut guarded = self.operation()?;
        let operation = guarded.operation;
        let sent = modifiers
            .iter()
            .try_for_each(|&key| guarded.state.press(operation, Held::Key(key)))
            .and_then(|()| guarded.state.send_one(Input::Wheel { delta }));
        let released = guarded.state.release_operation(operation);
        sent.and(released)
    }

    /// Places the cursor at virtual-desktop pixel `(x, y)`.
    pub(crate) fn place_cursor(&self, x: i32, y: i32) -> Result<(), String> {
        if self.operation()?.state.backend.place_cursor(x, y) {
            Ok(())
        } else {
            Err(format!("Windows refused to place the cursor at ({x},{y})"))
        }
    }
}

/// A pressed left button; released explicitly with `release`, or on drop. Releasing stays
/// allowed after the shared stop: it only clears what this guard registered.
pub(crate) struct LeftButton<'a, B: Backend> {
    injection: &'a Injection<B>,
    operation: Operation,
    released: bool,
}

impl<B: Backend> LeftButton<'_, B> {
    /// Releases the button under its operation's unwind guard: if the release unwinds, the
    /// guard stops injection and retries it once.
    fn finish(&mut self) -> Result<(), String> {
        self.released = true;
        let mut guarded = Guarded {
            state: self.injection.lock(),
            operation: self.operation,
        };
        guarded
            .state
            .release(self.operation, Held::Button(Button::Left))
    }

    pub(crate) fn release(mut self) -> Result<(), String> {
        self.finish()
    }
}

impl<B: Backend> Drop for LeftButton<'_, B> {
    fn drop(&mut self) {
        if self.released {
            return;
        }
        if std::thread::panicking() {
            // The case holding the button unwinds, so its outcome is unknown: injection stops
            // first, and the release is isolated, since a panic escaping here would abort.
            self.released = true;
            self.injection.lock().release_unwinding(
                self.operation,
                "a case unwound while it held the left button",
            );
        } else {
            // A failure here sets the shared stop, which every later injection reports.
            let _ = self.finish();
        }
    }
}

#[cfg(all(windows, panic = "unwind"))]
pub(crate) type LiveBackend = win32::SendInputBackend;

/// The backend of a build that refuses injection; it can never be constructed.
#[cfg(not(all(windows, panic = "unwind")))]
pub(crate) enum Refused {}

#[cfg(not(all(windows, panic = "unwind")))]
impl Backend for Refused {
    fn send(&mut self, _: &[Input]) -> usize {
        match *self {}
    }

    fn place_cursor(&mut self, _: i32, _: i32) -> bool {
        match *self {}
    }

    fn left_button_down(&mut self) -> bool {
        match *self {}
    }

    fn sleep(&mut self, _: Duration) {
        match *self {}
    }

    fn now(&mut self) -> Instant {
        match *self {}
    }
}

#[cfg(not(all(windows, panic = "unwind")))]
pub(crate) type LiveBackend = Refused;

/// A left button pressed through the live injector.
#[cfg(windows)]
pub(crate) type LiveLeftButton = LeftButton<'static, LiveBackend>;

/// Why the live injector stopped, if it did; a build without one never stops.
pub(crate) fn stop_reason() -> Option<String> {
    live().ok().and_then(Injection::stop_reason)
}

/// Fails in a build without a live injector or once it stopped (see
/// `Injection::require_running`).
#[cfg(windows)]
pub(crate) fn require_running() -> Result<(), String> {
    live()?.require_running()
}

/// The process's injector; a build that cannot unwind, or is not for Windows, has none.
pub(crate) fn live() -> Result<&'static Injection<LiveBackend>, String> {
    #[cfg(all(windows, panic = "unwind"))]
    {
        static LIVE: std::sync::OnceLock<Injection<LiveBackend>> = std::sync::OnceLock::new();
        Ok(LIVE.get_or_init(|| Injection::new(win32::SendInputBackend)))
    }
    #[cfg(not(all(windows, panic = "unwind")))]
    {
        Err(
            "input injection is refused in this build: it cannot unwind (panic = \"abort\") \
             or is not for Windows, so an injected press could not be released on failure"
                .to_owned(),
        )
    }
}

#[cfg(test)]
mod tests;

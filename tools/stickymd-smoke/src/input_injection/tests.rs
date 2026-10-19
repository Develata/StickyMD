use std::collections::VecDeque;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use super::*;

const A: Key = Key::new(0x41, 0x1E);

#[derive(Default)]
struct Log {
    inputs: Vec<Input>,
    cursor: Vec<(i32, i32)>,
    sends: usize,
    /// Zero-based `send` calls that insert nothing.
    short: Vec<usize>,
    /// Zero-based `send` calls that panic.
    panic_at: Vec<usize>,
    /// Zero-based `send` calls that panic with a payload whose destructor panics.
    bomb_at: Vec<usize>,
    /// Left-button reads to return first, in order; after them the injected state is read.
    reads: VecDeque<bool>,
    /// The left button reads down whatever was injected.
    stuck_down: bool,
    /// The left button never reads down.
    never_down: bool,
    /// The zero-based left-button read that panics.
    panic_on_read: Option<usize>,
    button_reads: usize,
    /// A zero-based left-button read that takes this long, by the scripted clock.
    slow_read: Option<(usize, Duration)>,
    refuse_cursor: bool,
    left_down: bool,
    slept: Duration,
}

#[derive(Clone, Default)]
struct Scripted(Arc<Mutex<Log>>);

impl Scripted {
    fn with(script: impl FnOnce(&mut Log)) -> Self {
        let scripted = Self::default();
        script(&mut scripted.log());
        scripted
    }

    fn log(&self) -> std::sync::MutexGuard<'_, Log> {
        self.0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    fn inputs(&self) -> Vec<Input> {
        self.log().inputs.clone()
    }
}

impl Backend for Scripted {
    fn send(&mut self, inputs: &[Input]) -> usize {
        let mut log = self.log();
        let call = log.sends;
        log.sends += 1;
        if log.panic_at.contains(&call) {
            drop(log);
            panic!("the backend failed while sending {inputs:?}");
        }
        if log.bomb_at.contains(&call) {
            drop(log);
            std::panic::panic_any(Bomb);
        }
        if log.short.contains(&call) {
            return 0;
        }
        for input in inputs {
            if let Input::Button {
                button: Button::Left,
                up,
            } = *input
            {
                log.left_down = !up;
            }
            log.inputs.push(*input);
        }
        inputs.len()
    }

    fn place_cursor(&mut self, x: i32, y: i32) -> bool {
        let mut log = self.log();
        if log.refuse_cursor {
            return false;
        }
        log.cursor.push((x, y));
        true
    }

    fn left_button_down(&mut self) -> bool {
        let mut log = self.log();
        let read = log.button_reads;
        log.button_reads += 1;
        if let Some((slow, duration)) = log.slow_read
            && slow == read
        {
            log.slept += duration;
        }
        if log.panic_on_read == Some(read) {
            drop(log);
            panic!("the button state could not be read");
        }
        if let Some(read) = log.reads.pop_front() {
            return read;
        }
        log.stuck_down || (log.left_down && !log.never_down)
    }

    fn sleep(&mut self, duration: Duration) {
        self.log().slept += duration;
    }

    /// A clock that only sleeps and slow reads advance.
    fn now(&mut self) -> Instant {
        static ORIGIN: OnceLock<Instant> = OnceLock::new();
        *ORIGIN.get_or_init(Instant::now) + self.log().slept
    }
}

/// A panic payload whose destructor panics.
struct Bomb;

impl Drop for Bomb {
    fn drop(&mut self) {
        panic!("a panic payload's destructor panicked");
    }
}

const fn key(key: Key, up: bool) -> Input {
    Input::Key { key, up }
}

const fn left(up: bool) -> Input {
    Input::Button {
        button: Button::Left,
        up,
    }
}

const fn right(up: bool) -> Input {
    Input::Button {
        button: Button::Right,
        up,
    }
}

/// Asserts every kind of new injection is refused with `reason`, and nothing reaches the
/// stream or the cursor.
fn assert_refused(injection: &Injection<Scripted>, backend: &Scripted, reason: &str) {
    let (inputs, cursor) = (backend.inputs().len(), backend.log().cursor.len());
    for refused in [
        injection.tap(A),
        injection.chord(&[SHIFT], A),
        injection.click_right(),
        injection.move_absolute(1, 2),
        injection.wheel(120),
        injection.place_cursor(3, 4),
        injection.release_left(),
        injection.require_running(),
        injection.press_left().map(drop),
    ] {
        let error = refused.unwrap_err();
        assert!(
            error.contains("stopped") && error.contains(reason),
            "{error}"
        );
    }
    assert_eq!(backend.inputs().len(), inputs, "nothing reaches the stream");
    assert_eq!(
        backend.log().cursor.len(),
        cursor,
        "the cursor is not placed"
    );
}

/// A chord presses its modifiers in order and the key, then releases them in reverse.
#[test]
fn a_chord_releases_in_reverse_press_order() {
    let backend = Scripted::default();
    let injection = Injection::new(backend.clone());
    injection.chord(&[CONTROL, SHIFT], A).unwrap();
    assert_eq!(
        backend.inputs(),
        [
            key(CONTROL, false),
            key(SHIFT, false),
            key(A, false),
            key(A, true),
            key(SHIFT, true),
            key(CONTROL, true),
        ]
    );
    injection.tap(ESCAPE).unwrap();
    assert_eq!(
        backend.inputs()[6..],
        [key(ESCAPE, false), key(ESCAPE, true)]
    );
    injection.click_right().unwrap();
    assert_eq!(backend.inputs()[8..], [right(false), right(true)]);
    assert_eq!(injection.stop_reason(), None);
}

/// A press that inserts nothing fails, and every key the operation registered before it is
/// still released; a press failure alone does not stop injection.
#[test]
fn a_short_press_fails_and_releases_what_was_registered() {
    // Calls: Ctrl down (0), Shift down (1, short).
    let backend = Scripted::with(|log| log.short = vec![1]);
    let injection = Injection::new(backend.clone());
    let error = injection.chord(&[CONTROL, SHIFT], A).unwrap_err();
    assert!(error.contains("cannot press"), "{error}");
    assert_eq!(
        backend.inputs(),
        [key(CONTROL, false), key(SHIFT, true), key(CONTROL, true)]
    );
    assert_eq!(injection.stop_reason(), None);
    injection.tap(A).unwrap();
}

/// A release that inserts nothing stops every later injection in the process, of every kind.
#[test]
fn a_short_release_stops_all_later_injection() {
    // Calls: Ctrl down (0), A down (1), A up (2, short), Ctrl up (3).
    let backend = Scripted::with(|log| log.short = vec![2]);
    let injection = Injection::new(backend.clone());
    let error = injection.chord(&[CONTROL], A).unwrap_err();
    assert!(error.contains("release of"), "{error}");
    // The remaining modifier is still released.
    assert_eq!(backend.inputs().last(), Some(&key(CONTROL, true)));
    let reason = injection.stop_reason().expect("stopped");
    assert_refused(&injection, &backend, &reason);
}

/// A backend that panics while an operation has keys pressed: the unwind releases what that
/// operation pressed and stops all later injection, and the poisoned lock stays stopped.
#[test]
fn an_unwinding_operation_releases_its_presses_and_stops() {
    // Calls: Ctrl down (0), A down (1, panics) -> unwind: A up (2), Ctrl up (3).
    let backend = Scripted::with(|log| log.panic_at = vec![1]);
    let injection = Injection::new(backend.clone());
    assert!(catch_unwind(AssertUnwindSafe(|| injection.chord(&[CONTROL], A))).is_err());
    assert_eq!(
        backend.inputs(),
        [key(CONTROL, false), key(A, true), key(CONTROL, true)]
    );
    let reason = injection.stop_reason().expect("stopped");
    assert!(reason.contains("unwound"), "{reason}");
    assert_refused(&injection, &backend, &reason);

    // A panic while releasing keeps the registration, so the unwind releases it again.
    // Calls: Ctrl down (0), A down (1), A up (2, panics) -> unwind: A up (3), Ctrl up (4).
    let backend = Scripted::with(|log| log.panic_at = vec![2]);
    let injection = Injection::new(backend.clone());
    assert!(catch_unwind(AssertUnwindSafe(|| injection.chord(&[CONTROL], A))).is_err());
    assert_eq!(
        backend.inputs(),
        [
            key(CONTROL, false),
            key(A, false),
            key(A, true),
            key(CONTROL, true)
        ]
    );
    assert!(injection.stop_reason().is_some());

    // A panic while sampling a fresh left press releases the button too.
    let backend = Scripted::with(|log| log.panic_on_read = Some(0));
    let injection = Injection::new(backend.clone());
    assert!(catch_unwind(AssertUnwindSafe(|| injection.press_left().map(drop))).is_err());
    assert_eq!(backend.inputs(), [left(false), left(true)]);
    assert!(injection.stop_reason().is_some());
}

/// A release that panics again while unwinding: it is caught, so the remaining releases still
/// run and the process does not abort.
#[test]
fn a_release_panicking_again_while_unwinding_is_isolated() {
    // Calls: Ctrl down (0), A down (1, panics) -> unwind: A up (2, panics), Ctrl up (3).
    let backend = Scripted::with(|log| log.panic_at = vec![1, 2]);
    let injection = Injection::new(backend.clone());
    assert!(catch_unwind(AssertUnwindSafe(|| injection.chord(&[CONTROL], A))).is_err());
    assert_eq!(backend.inputs(), [key(CONTROL, false), key(CONTROL, true)]);
    assert!(injection.stop_reason().is_some());

    // A held left button dropped while a case unwinds, its release panicking: one isolated
    // attempt, no registration left behind, injection stopped.
    // Calls: left down (0), left up (1, panics).
    let backend = Scripted::with(|log| log.panic_at = vec![1]);
    let injection = Injection::new(backend.clone());
    assert!(
        catch_unwind(AssertUnwindSafe(|| {
            let _button = injection.press_left().unwrap();
            panic!("the case failed while the button was held");
        }))
        .is_err()
    );
    assert_eq!(backend.inputs(), [left(false)]);
    assert!(injection.lock().held.is_empty());
    assert!(injection.stop_reason().is_some());

    // The same drop outside an unwind retries under the release's guard, then reports.
    // Calls: left down (0), left up (1, panics) -> guard: left up (2).
    let backend = Scripted::with(|log| log.panic_at = vec![1]);
    let injection = Injection::new(backend.clone());
    assert!(catch_unwind(AssertUnwindSafe(|| drop(injection.press_left().unwrap()))).is_err());
    assert_eq!(backend.inputs(), [left(false), left(true)]);
    assert!(injection.lock().held.is_empty());
    assert!(injection.stop_reason().is_some());
}

/// A panic payload that panics when dropped: it is never dropped, so the remaining releases
/// still run and the process does not abort.
#[test]
fn a_panic_payload_is_not_dropped_while_unwinding() {
    // Calls: Ctrl down (0), A down (1, panics) -> unwind: A up (2, a bomb), Ctrl up (3).
    let backend = Scripted::with(|log| {
        log.panic_at = vec![1];
        log.bomb_at = vec![2];
    });
    let injection = Injection::new(backend.clone());
    assert!(catch_unwind(AssertUnwindSafe(|| injection.chord(&[CONTROL], A))).is_err());
    assert_eq!(backend.inputs(), [key(CONTROL, false), key(CONTROL, true)]);
    assert!(injection.lock().held.is_empty());
    assert!(injection.stop_reason().is_some());
}

/// The release wait keeps its deadline by the backend's clock: a reading taken after it fails
/// the release even when it reads up, one taken before it counts.
#[test]
fn a_left_release_read_past_its_deadline_fails() {
    // Reads: the press (0), the release's first sample (1).
    for (delay, settles) in [
        (Duration::from_millis(600), false),
        (Duration::from_millis(400), true),
    ] {
        let backend = Scripted::with(|log| log.slow_read = Some((1, delay)));
        let injection = Injection::new(backend.clone());
        let released = injection.press_left().unwrap().release();
        assert_eq!(released.is_ok(), settles, "{delay:?}: {released:?}");
        assert_eq!(injection.stop_reason().is_none(), settles, "{delay:?}");
    }

    // A reading at the deadline counts: reading down there fails at once, even if the next
    // reading, at the same instant, would read up; reading up there succeeds. Reads: the press
    // (0), the release's first sample (1, slow), its second (2).
    for (delay, settles) in [
        (LEFT_BUTTON_SETTLE, false),
        (LEFT_BUTTON_SETTLE - Duration::from_millis(1), true),
    ] {
        let backend = Scripted::with(|log| {
            log.reads = VecDeque::from([true, true, false]);
            log.slow_read = Some((1, delay));
        });
        let injection = Injection::new(backend.clone());
        let released = injection.press_left().unwrap().release();
        assert_eq!(released.is_ok(), settles, "{delay:?}: {released:?}");
        assert_eq!(injection.stop_reason().is_none(), settles, "{delay:?}");
        if settles {
            // Down at 499 ms sleeps only the 1 ms left, then reads up at the deadline.
            assert_eq!(backend.log().slept, LEFT_BUTTON_SETTLE);
        }
    }

    // A stuck button stops sampling at the deadline, sleeping no further than it.
    let backend = Scripted::with(|log| log.stuck_down = true);
    let injection = Injection::new(backend.clone());
    drop(injection.press_left().unwrap());
    assert_eq!(backend.log().slept, LEFT_BUTTON_SETTLE);
}

/// The left-button guard's own release unwinding: the release's guard stops injection and
/// retries it once. Both the send and the sampling of the release are covered.
#[test]
fn an_unwinding_left_release_is_retried_by_its_guard() {
    // Calls: left down (0), left up (1, panics) -> guard: left up (2).
    let backend = Scripted::with(|log| log.panic_at = vec![1]);
    let injection = Injection::new(backend.clone());
    assert!(
        catch_unwind(AssertUnwindSafe(|| injection
            .press_left()
            .unwrap()
            .release()))
        .is_err()
    );
    assert_eq!(backend.inputs(), [left(false), left(true)]);
    assert!(injection.stop_reason().is_some());
    assert!(injection.tap(A).is_err());

    // Reads: the press (0), the release's first sample (1, panics), the guard's sample (2).
    let backend = Scripted::with(|log| log.panic_on_read = Some(1));
    let injection = Injection::new(backend.clone());
    assert!(
        catch_unwind(AssertUnwindSafe(|| injection
            .press_left()
            .unwrap()
            .release()))
        .is_err()
    );
    assert_eq!(backend.inputs(), [left(false), left(true), left(true)]);
    assert!(injection.stop_reason().is_some());
}

/// A key or button is held by one operation at a time: a second hold of the left button, or a
/// recovery release while it is held, is refused and leaves the first hold intact.
#[test]
fn a_held_button_is_neither_held_again_nor_recovered_away() {
    let backend = Scripted::default();
    let injection = Injection::new(backend.clone());
    let first = injection.press_left().unwrap();
    let error = injection.press_left().map(drop).unwrap_err();
    assert!(error.contains("already held"), "{error}");
    let error = injection.release_left().unwrap_err();
    assert!(error.contains("holds the left button"), "{error}");
    assert_eq!(backend.inputs(), [left(false)], "the hold is untouched");
    assert_eq!(injection.stop_reason(), None);
    first.release().unwrap();
    injection.release_left().unwrap();
}

/// A left press waits until the button reads down, its release until it reads up: a button
/// that settles up within the window passes, one still down at the deadline stops injection.
#[test]
fn a_left_button_still_down_after_the_window_stops_injection() {
    let backend = Scripted::default();
    let injection = Injection::new(backend.clone());
    injection.press_left().unwrap().release().unwrap();
    assert_eq!(backend.inputs(), [left(false), left(true)]);

    // The first read serves the press; the next three still read down after the release,
    // then it reads up: the release settled within the window.
    let settling = Scripted::with(|log| log.reads = VecDeque::from([true, true, true, true]));
    let injection = Injection::new(settling.clone());
    injection.press_left().unwrap().release().unwrap();
    assert_eq!(injection.stop_reason(), None);
    assert_eq!(settling.log().slept, Duration::from_millis(15));

    let stuck = Scripted::with(|log| log.stuck_down = true);
    let injection = Injection::new(stuck.clone());
    let error = injection.release_left().unwrap_err();
    assert!(error.contains("did not read up"), "{error}");
    assert!(injection.stop_reason().is_some());
    assert!(stuck.log().slept >= Duration::from_millis(400));
}

/// A press that never reads down fails and is released; the press failure does not stop.
#[test]
fn a_left_press_never_seen_down_is_released() {
    let backend = Scripted::with(|log| log.never_down = true);
    let injection = Injection::new(backend.clone());
    let error = injection.press_left().map(drop).unwrap_err();
    assert!(error.contains("did not read down"), "{error}");
    assert_eq!(backend.inputs(), [left(false), left(true)]);
    assert_eq!(injection.stop_reason(), None);
}

/// The guard releases on drop and on unwind, and an explicit release is not repeated by drop.
#[test]
fn the_left_button_is_released_on_drop_and_on_unwind() {
    let backend = Scripted::default();
    let injection = Injection::new(backend.clone());
    drop(injection.press_left().unwrap());
    assert_eq!(backend.inputs(), [left(false), left(true)]);

    let result = catch_unwind(AssertUnwindSafe(|| {
        let _button = injection.press_left().unwrap();
        panic!("a case failed while the button was down");
    }));
    assert!(result.is_err());
    assert_eq!(backend.inputs()[2..], [left(false), left(true)]);
    // The case's outcome is unknown, so injection stops before the release.
    let reason = injection.stop_reason().expect("stopped");
    assert!(reason.contains("held the left button"), "{reason}");
    assert_refused(&injection, &backend, &reason);
}

/// Each operation releases only what it pressed: keys tapped while the left button is held
/// leave the button down, and the button's own release sends exactly one release.
#[test]
fn an_operation_releases_only_its_own_presses() {
    let backend = Scripted::default();
    let injection = Injection::new(backend.clone());
    let button = injection.press_left().unwrap();
    injection.tap(ESCAPE).unwrap();
    injection.chord(&[SHIFT], A).unwrap();
    assert!(
        !backend.inputs().contains(&left(true)),
        "a chord released the held button"
    );
    button.release().unwrap();
    assert_eq!(
        backend
            .inputs()
            .iter()
            .filter(|input| **input == left(true))
            .count(),
        1
    );
}

/// After the shared stop, a held button's guard still releases what it registered (and only
/// that), while new injection is refused.
#[test]
fn a_registered_press_is_still_released_after_the_stop() {
    // Calls: left down (0), Ctrl down (1), A down (2), A up (3, short), Ctrl up (4), left up (5).
    let backend = Scripted::with(|log| log.short = vec![3]);
    let injection = Injection::new(backend.clone());
    let button = injection.press_left().unwrap();
    assert!(injection.chord(&[CONTROL], A).is_err());
    assert!(injection.stop_reason().is_some());
    assert!(injection.tap(A).is_err());
    button.release().unwrap();
    assert_eq!(backend.inputs().last(), Some(&left(true)));
}

/// A drop whose release fails cannot report it; the shared stop does.
#[test]
fn a_failed_release_on_drop_sets_the_shared_stop() {
    // Calls: left down (0), left up (1, short).
    let backend = Scripted::with(|log| log.short = vec![1]);
    let injection = Injection::new(backend.clone());
    drop(injection.press_left().unwrap());
    assert!(injection.stop_reason().is_some());
    assert!(injection.tap(A).is_err());
}

/// Moves, wheel turns and cursor placements hold nothing: a refusal fails the call without
/// stopping.
#[test]
fn a_short_move_or_refused_placement_fails_without_stopping() {
    let backend = Scripted::with(|log| log.short = vec![0, 1]);
    let injection = Injection::new(backend.clone());
    assert!(injection.move_absolute(10, 20).is_err());
    assert!(injection.wheel(-120).is_err());
    assert_eq!(injection.stop_reason(), None);
    injection.move_absolute(10, 20).unwrap();
    assert_eq!(backend.inputs(), [Input::Move { x: 10, y: 20 }]);
    injection.place_cursor(5, 6).unwrap();
    assert_eq!(backend.log().cursor, [(5, 6)]);
    backend.log().refuse_cursor = true;
    assert!(injection.place_cursor(7, 8).is_err());
    assert_eq!(injection.stop_reason(), None);
    injection.require_running().unwrap();
}

/// A build that cannot unwind, or is not for Windows, has no live injector.
#[test]
fn only_an_unwinding_windows_build_injects() {
    assert_eq!(
        live().is_ok(),
        cfg!(all(windows, panic = "unwind")),
        "{:?}",
        live().err()
    );
    // Nothing has failed to release in this process.
    assert_eq!(stop_reason(), None);
}

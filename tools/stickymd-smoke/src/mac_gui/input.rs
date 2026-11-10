//! Single-owner input stream: register before pressing, release in reverse on error/unwind.
//! A failed release permanently stops later operations, including later scenarios.
//! plan_ref: docs/plan/11_testing_and_release.md#phase-16-macos-host
use super::model::Point;
use std::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};
pub(super) const CMD: u16 = 55;
pub(super) const SHIFT: u16 = 56;
pub(super) const OPTION: u16 = 58;
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum Held {
    Key(u16, Option<u16>),
    Left,
}
#[derive(Clone, Copy, Debug)]
pub(super) enum Event {
    Key {
        code: u16,
        text: Option<u16>,
        down: bool,
        flags: u64,
    },
    Button {
        at: Point,
        down: bool,
    },
    Move {
        at: Point,
        dragging: bool,
    },
}
pub(super) trait Sender {
    fn send(&mut self, event: Event) -> Result<(), String>;
    fn pause(&mut self, ms: u64);
}
pub(super) struct Injection<B: Sender> {
    pub backend: B,
    held: Vec<Held>,
    at: Point,
    pub stopped: Option<String>,
}
impl<B: Sender> Injection<B> {
    pub fn new(backend: B) -> Self {
        Self {
            backend,
            held: Vec::new(),
            at: Point::default(),
            stopped: None,
        }
    }
    pub fn require_running(&self) -> Result<(), String> {
        self.stopped
            .as_ref()
            .map_or(Ok(()), |s| Err(format!("input stopped: {s}")))
    }
    fn flags(&self) -> u64 {
        self.held.iter().fold(0, |f, k| {
            f | match k {
                Held::Key(CMD, _) => 1 << 20,
                Held::Key(SHIFT, _) => 1 << 17,
                Held::Key(OPTION, _) => 1 << 19,
                _ => 0,
            }
        })
    }
    fn event(&self, held: Held, down: bool) -> Event {
        match held {
            Held::Key(code, text) => Event::Key {
                code,
                text,
                down,
                flags: self.flags(),
            },
            Held::Left => Event::Button { at: self.at, down },
        }
    }
    fn press(&mut self, held: Held) -> Result<(), String> {
        self.require_running()?;
        if self.held.contains(&held) {
            return Err("duplicate held input".into());
        }
        self.held.push(held); // Register before send: a failing API may already have posted.
        self.backend.send(self.event(held, true))
    }
    fn release_all(&mut self) -> Result<(), String> {
        let mut errors = Vec::new();
        while let Some(held) = self.held.pop() {
            // Catch each release separately so a panicking fake/backend cannot prevent the rest.
            let event = self.event(held, false);
            match catch_unwind(AssertUnwindSafe(|| self.backend.send(event))) {
                Ok(Ok(())) => {}
                Ok(Err(e)) => {
                    errors.push(e);
                    // A second best-effort release is safe: only this stream owns this press.
                    if let Err(payload) =
                        catch_unwind(AssertUnwindSafe(|| self.backend.send(event)))
                    {
                        std::mem::forget(payload);
                    }
                }
                Err(payload) => {
                    std::mem::forget(payload);
                    errors.push("release panicked".into());
                }
            }
        }
        if errors.is_empty() {
            Ok(())
        } else {
            let error = errors.join("; ");
            self.stopped.get_or_insert(error.clone());
            Err(error)
        }
    }
    fn operation(&mut self, f: impl FnOnce(&mut Self) -> Result<(), String>) -> Result<(), String> {
        self.require_running()?;
        let outcome = catch_unwind(AssertUnwindSafe(|| f(self)));
        if !matches!(&outcome, Ok(Ok(()))) {
            self.stopped
                .get_or_insert("input operation failed or unwound".into());
        }
        let release = self.release_all();
        match outcome {
            Ok(result) => result.and(release),
            Err(payload) => resume_unwind(payload),
        }
    }
    pub fn chord(&mut self, keys: &[u16]) -> Result<(), String> {
        self.operation(|s| {
            for key in keys {
                s.press(Held::Key(*key, None))?;
                s.backend.pause(12);
            }
            Ok(())
        })
    }
    pub fn text(&mut self, text: &str) -> Result<(), String> {
        // Unicode payloads still travel through real key-down/up pairs; no pasteboard shortcut.
        for unit in text.encode_utf16() {
            if unit == 10 {
                self.chord(&[36])?;
                continue;
            }
            if unit == 9 {
                self.chord(&[48])?;
                continue;
            }
            self.operation(|s| s.press(Held::Key(0, Some(unit))))?;
            self.backend.pause(20);
        }
        Ok(())
    }
    pub fn move_to(&mut self, at: Point) -> Result<(), String> {
        self.operation(|s| {
            s.at = at;
            s.backend.send(Event::Move {
                at,
                dragging: false,
            })
        })
    }
    pub fn click(&mut self, at: Point) -> Result<(), String> {
        self.operation(|s| {
            s.at = at;
            s.backend.send(Event::Move {
                at,
                dragging: false,
            })?;
            s.press(Held::Left)?;
            s.backend.pause(40);
            Ok(())
        })
    }
    pub fn drag(&mut self, from: Point, to: Point) -> Result<(), String> {
        self.operation(|s| {
            s.at = from;
            s.backend.send(Event::Move {
                at: from,
                dragging: false,
            })?;
            s.press(Held::Left)?;
            for step in 1..=20 {
                s.at = Point {
                    x: from.x + (to.x - from.x) * f64::from(step) / 20.0,
                    y: from.y + (to.y - from.y) * f64::from(step) / 20.0,
                };
                s.backend.send(Event::Move {
                    at: s.at,
                    dragging: true,
                })?;
                s.backend.pause(15);
            }
            Ok(())
        })
    }
    pub fn release(&mut self) -> Result<(), String> {
        self.release_all()
    }
}
impl<B: Sender> Drop for Injection<B> {
    fn drop(&mut self) {
        let _ = self.release_all();
    }
}

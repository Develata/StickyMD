//! Headless tests for the W06, W11, W12 and F01 orchestration against simulated products; no GUI
//! process is launched and no native event is built.
//! plan_ref: docs/plan/11_testing_and_release.md#phase-16-macos-host
use super::{
    input::{CMD, SHIFT},
    lifecycle::{CANCEL_MS, DURABLE, QUIT_CANCELLED},
    model::*,
    scenarios::{self, Backend, Driver, Op, Value},
};

fn verdict(row: &Row, name: &str) -> Option<Verdict> {
    row.checks
        .iter()
        .find(|c| c.name == name)
        .map(|c| c.verdict)
}
fn run(backend: &mut impl Backend, id: &str) -> Row {
    let mut row = Row::new(id);
    scenarios::run(
        &mut Driver {
            backend,
            row: &mut row,
            pid: 42,
        },
        id,
    )
    .unwrap();
    row
}

/// A product whose note save fails while the fault is on, unless `fault_bites` is false.
struct Quitting {
    now: u64,
    fault: bool,
    fault_bites: bool,
    titles: bool,
    /// How long the failed save takes to reach the title.
    cancel_after: u64,
    cancelled_at: Option<u64>,
    exit_at: Option<u64>,
    /// How long the quit takes once the export dialog has been ended.
    modal_quit_after: u64,
    memory: String,
    disk: String,
    panel: bool,
    /// AX reads of the dialog that end as CannotComplete while it is being presented.
    busy_panel_reads: u32,
    /// The request waits until the dialog is closed by hand instead of ending it.
    deferred_in_dialog: bool,
    deferred: bool,
    /// A failing save that truncates the note before it fails.
    truncates_on_failure: bool,
    /// AX stops reporting titles once the save succeeds.
    titles_vanish_after_save: bool,
    /// An autosave writes the text as soon as the directory is writable again.
    autosave_on_restore: bool,
    /// Title reads fail (CannotComplete) once the save succeeded.
    title_fails_after_save: bool,
    saved: bool,
    /// Cancelling the dialog by hand does not close it.
    cancel_ignored: bool,
    /// Title reads fail once the quit was cancelled, though the product did cancel it.
    title_fails_after_quit: bool,
    /// The dialog cannot be aimed at the session's export file.
    aim_fails: bool,
    aimed: bool,
    /// The defect: the dialog ended by the quit exports anyway.
    exports_on_end: bool,
    export: Option<String>,
}
impl Quitting {
    fn new() -> Self {
        Self {
            now: 0,
            fault: false,
            fault_bites: true,
            titles: true,
            cancel_after: 300,
            cancelled_at: None,
            exit_at: None,
            modal_quit_after: 900,
            memory: DURABLE.into(),
            disk: DURABLE.into(),
            panel: false,
            busy_panel_reads: 0,
            deferred_in_dialog: false,
            deferred: false,
            truncates_on_failure: false,
            titles_vanish_after_save: false,
            autosave_on_restore: false,
            title_fails_after_save: false,
            saved: false,
            cancel_ignored: false,
            title_fails_after_quit: false,
            aim_fails: false,
            aimed: false,
            exports_on_end: false,
            export: None,
        }
    }
    fn modal_quit(&mut self) {
        if self.exports_on_end && self.aimed {
            self.export = Some(self.memory.clone());
        }
        self.panel = false;
        self.disk = self.memory.clone();
        self.exit_at = Some(self.now + self.modal_quit_after);
    }
    fn quit(&mut self) {
        if self.fault && self.fault_bites {
            self.cancelled_at = Some(self.now + self.cancel_after);
            if self.truncates_on_failure {
                self.disk.clear();
            }
        } else {
            self.disk = self.memory.clone();
            self.exit_at = Some(self.now + 100);
        }
    }
    fn alive(&self) -> bool {
        self.exit_at.is_none_or(|at| self.now < at)
    }
}
impl Backend for Quitting {
    fn now(&self) -> u64 {
        self.now
    }
    fn perform(&mut self, op: Op) -> Result<Value, String> {
        self.now += 5;
        Ok(match op {
            Op::Wait(ms) => {
                self.now += ms;
                Value::Unit
            }
            Op::Activate => Value::Unit,
            Op::Fault(on) => {
                self.fault = on;
                if !on && self.autosave_on_restore {
                    self.disk = self.memory.clone();
                }
                Value::Unit
            }
            Op::Chord(keys) if keys == [CMD, 0] => {
                self.memory.clear();
                Value::Unit
            }
            Op::Text(s) => {
                self.memory.push_str(&s);
                Value::Time(self.now)
            }
            Op::Chord(keys) if keys == [CMD, 12] => {
                self.quit();
                Value::Unit
            }
            Op::Chord(keys) if keys == [CMD, 1] => {
                if !self.fault {
                    self.disk = self.memory.clone();
                    self.cancelled_at = None;
                    self.titles &= !self.titles_vanish_after_save;
                    self.saved = true;
                }
                Value::Unit
            }
            Op::Chord(keys) if keys == [CMD, SHIFT, 1] => {
                self.panel = true;
                Value::Unit
            }
            Op::Terminate => {
                if self.panel && self.deferred_in_dialog {
                    self.deferred = true;
                } else if self.panel {
                    self.modal_quit();
                } else {
                    self.quit();
                }
                Value::Text("true\n".into())
            }
            Op::Panel(false) => {
                if !self.cancel_ignored {
                    self.panel = false;
                    if self.deferred {
                        self.modal_quit();
                    }
                }
                Value::Text("cancelled".into())
            }
            Op::PanelTarget if self.aim_fails => {
                return Err("Go to Folder focus is AXButton".into());
            }
            Op::PanelTarget => {
                self.aimed = true;
                Value::Text("aimed".into())
            }
            Op::Title if self.title_fails_after_quit && self.cancelled_at.is_some() => {
                return Err("AX attribute error -25204; inspection incomplete".into());
            }
            Op::Title if self.title_fails_after_save && self.saved => {
                return Err("AX attribute error -25204; inspection incomplete".into());
            }
            Op::Title => Value::Text(if !self.titles {
                String::new()
            } else if self.cancelled_at.is_some_and(|at| self.now >= at) {
                format!("StickyMD * — {QUIT_CANCELLED}")
            } else {
                "StickyMD *".into()
            }),
            Op::Alive => Value::Bool(self.alive()),
            Op::Note | Op::NoteNow => Value::Text(self.disk.clone()),
            Op::PanelOpen if self.busy_panel_reads > 0 => {
                self.busy_panel_reads -= 1;
                return Err("AX attribute error -25204; inspection incomplete".into());
            }
            Op::PanelOpen => Value::Bool(self.panel),
            Op::Export => Value::Text(self.export.clone().unwrap_or_else(|| "<absent>".into())),
            op => return Err(format!("unexpected fake operation {op:?}")),
        })
    }
}

#[test]
fn w11_passes_only_when_the_failed_save_cancels_the_quit() {
    let row = run(&mut Quitting::new(), "W11");
    assert_eq!(aggregate(&row.checks), Verdict::Pass, "{}", row.json());

    let mut harmless = Quitting::new();
    harmless.fault_bites = false;
    let row = run(&mut harmless, "W11");
    assert_eq!(
        verdict(&row, "Cmd+Q: process keeps running"),
        Some(Verdict::Fail)
    );
    assert_eq!(
        verdict(&row, "Cmd+Q: old bytes still on disk"),
        Some(Verdict::Fail)
    );

    // A save that truncates before failing destroys the old bytes even though it stays up.
    let mut truncating = Quitting::new();
    truncating.truncates_on_failure = true;
    let row = run(&mut truncating, "W11");
    assert_eq!(
        verdict(&row, "Cmd+Q: old bytes still on disk"),
        Some(Verdict::Fail)
    );
}

#[test]
fn a_title_that_can_no_longer_be_read_is_not_a_cleared_one() {
    let name = "title clears once the save succeeds";
    let mut vanishing = Quitting::new();
    vanishing.titles_vanish_after_save = true;
    let row = run(&mut vanishing, "W11");
    assert_eq!(
        verdict(&row, name),
        Some(Verdict::NotTested),
        "{}",
        row.json()
    );
    // A failed read is unobserved too, and the scenario still reaches the clean quit.
    let mut failing = Quitting::new();
    failing.title_fails_after_save = true;
    let row = run(&mut failing, "W11");
    assert_eq!(
        verdict(&row, name),
        Some(Verdict::NotTested),
        "{}",
        row.json()
    );
    assert_eq!(verdict(&row, "process exited"), Some(Verdict::Pass));
}

#[test]
fn a_cancelled_title_after_the_deadline_is_late() {
    let mut slow = Quitting::new();
    slow.cancel_after = CANCEL_MS + 200;
    let row = run(&mut slow, "W11");
    assert_eq!(
        verdict(&row, "Cmd+Q: title shows the cancelled quit"),
        Some(Verdict::Fail),
        "{}",
        row.json()
    );
}

#[test]
fn without_ax_titles_the_title_checks_are_not_tested_not_passed() {
    let mut untitled = Quitting::new();
    untitled.titles = false;
    let row = run(&mut untitled, "W11");
    assert_eq!(aggregate(&row.checks), Verdict::NotTested, "{}", row.json());
    assert_eq!(
        verdict(&row, "Cmd+Q: title shows the cancelled quit"),
        Some(Verdict::NotTested)
    );
    assert_eq!(
        verdict(&row, "Cmd+Q: process keeps running"),
        Some(Verdict::Pass)
    );
}

#[test]
fn w12_covers_the_refused_and_the_modal_termination_but_not_logout() {
    let row = run(&mut Quitting::new(), "W12");
    assert!(
        row.checks.iter().all(|c| c.verdict != Verdict::Fail),
        "{}",
        row.json()
    );
    assert_eq!(verdict(&row, "logout"), Some(Verdict::NotTested));
    assert_eq!(
        verdict(&row, "the quit exits once the dialog is gone"),
        Some(Verdict::Pass)
    );

    // A dialog being presented holds the main thread: unanswered reads are not a closed dialog.
    let mut presenting = Quitting::new();
    presenting.busy_panel_reads = 3;
    let row = run(&mut presenting, "W12");
    assert_eq!(
        verdict(&row, "export dialog open"),
        Some(Verdict::Pass),
        "{}",
        row.json()
    );
    let mut unanswered = Quitting::new();
    unanswered.busy_panel_reads = u32::MAX;
    let row = run(&mut unanswered, "W12");
    assert_eq!(verdict(&row, "export dialog open"), Some(Verdict::Fail));

    // A product that defers the request until the dialog closes passes the exit check only. (On
    // macOS 27.2 the VM showed the request lost; see RISK-macos-modal-termination.)
    let mut deferred = Quitting::new();
    deferred.deferred_in_dialog = true;
    let row = run(&mut deferred, "W12");
    assert_eq!(
        verdict(&row, "the request ends the export dialog"),
        Some(Verdict::Fail)
    );
    assert_eq!(
        verdict(&row, "the quit exits once the dialog is gone"),
        Some(Verdict::Pass),
        "{}",
        row.json()
    );

    // A dialog that is not seen closed even after cancelling leaves the exit untimed.
    let mut sticky = Quitting::new();
    sticky.deferred_in_dialog = true;
    sticky.cancel_ignored = true;
    let row = run(&mut sticky, "W12");
    assert_eq!(
        verdict(&row, "the quit exits once the dialog is gone"),
        Some(Verdict::NotTested),
        "{}",
        row.json()
    );

    let mut stuck = Quitting::new();
    stuck.modal_quit_after = 60_000;
    let row = run(&mut stuck, "W12");
    assert_eq!(
        verdict(&row, "the quit exits once the dialog is gone"),
        Some(Verdict::Fail)
    );

    let mut harmless = Quitting::new();
    harmless.fault_bites = false;
    let row = run(&mut harmless, "W12");
    assert_eq!(aggregate(&row.checks), Verdict::Fail, "{}", row.json());

    // An autosave that saved first leaves nothing for the quit to be credited with.
    let mut autosaved = Quitting::new();
    autosaved.autosave_on_restore = true;
    let row = run(&mut autosaved, "W12");
    assert_eq!(
        verdict(
            &row,
            "text unsaved before the request is on disk after the exit"
        ),
        Some(Verdict::NotTested),
        "{}",
        row.json()
    );
    let row = run(&mut Quitting::new(), "W12");
    assert_eq!(
        verdict(
            &row,
            "text unsaved before the request is on disk after the exit"
        ),
        Some(Verdict::Pass)
    );
}

/// A window docked on the right of a 1024×768 screen.
struct Docking {
    now: u64,
    expanded: bool,
    focused: bool,
    layer: i32,
    /// The defect S7 forbids: focus returning expands the collapsed window.
    expand_on_focus: bool,
    /// The other defect: focus returning revokes the strip's raised layer.
    lower_on_focus: bool,
    /// How long after focus returns a brief expansion shows before it settles back (ms).
    flash_on_focus: u64,
    flash_until: u64,
    /// Extra time each window read takes, to model a stalled sampler (ms).
    read_cost: u64,
}
impl Docking {
    fn new() -> Self {
        Self {
            now: 0,
            expanded: true,
            focused: true,
            layer: 0,
            expand_on_focus: false,
            lower_on_focus: false,
            flash_on_focus: 0,
            flash_until: 0,
            read_cost: 0,
        }
    }
}
fn screen() -> Rect {
    Rect {
        origin: Point::default(),
        size: Size {
            width: 1024.0,
            height: 768.0,
        },
    }
}
impl Backend for Docking {
    fn now(&self) -> u64 {
        self.now
    }
    fn perform(&mut self, op: Op) -> Result<Value, String> {
        self.now += 5;
        Ok(match op {
            Op::Wait(ms) => {
                self.now += ms;
                Value::Unit
            }
            Op::Activate => {
                (self.expanded, self.focused, self.layer) = (true, true, 0);
                Value::Unit
            }
            Op::Drag(..) => Value::Unit,
            Op::Move(p) => {
                if p.x >= 1022.0 {
                    self.expanded = true;
                }
                Value::Unit
            }
            Op::Finder => {
                self.focused = false;
                if self.expanded {
                    (self.expanded, self.layer) = (false, 3);
                }
                Value::Pid(7)
            }
            Op::Chord(keys) if keys == [53] => {
                (self.expanded, self.layer) = (false, 3);
                Value::Unit
            }
            Op::Focus => {
                self.focused = true;
                if self.expand_on_focus {
                    self.expanded = true;
                }
                if self.lower_on_focus {
                    self.layer = 0;
                }
                self.flash_until = self.now + self.flash_on_focus;
                Value::Unit
            }
            Op::Front => Value::Pid(if self.focused { 42 } else { 7 }),
            Op::Screen => Value::Rect(screen()),
            Op::Config => Value::Text("[window]\ndock_edge = \"right\"\n".into()),
            Op::Window => {
                let expanded = self.expanded || self.now < self.flash_until;
                self.now += self.read_cost;
                Value::Window(Window {
                    id: 9,
                    bounds: Rect {
                        origin: Point {
                            x: if expanded { 400.0 } else { 1022.0 },
                            y: 144.0,
                        },
                        size: Size {
                            width: 520.0,
                            height: 480.0,
                        },
                    },
                    alpha: 1.0,
                    layer: if expanded { 0 } else { self.layer },
                    onscreen: true,
                })
            }
            op => return Err(format!("unexpected fake operation {op:?}")),
        })
    }
}

#[test]
fn w06_fails_when_focus_return_expands_or_lowers_the_strip() {
    let row = run(&mut Docking::new(), "W06");
    assert!(
        row.checks.iter().all(|c| c.verdict != Verdict::Fail),
        "{}",
        row.json()
    );
    assert_eq!(verdict(&row, "Space switch"), Some(Verdict::NotTested));
    let name = "focus return neither expands nor lowers the strip";
    assert_eq!(verdict(&row, name), Some(Verdict::Pass));

    let mut expanding = Docking::new();
    expanding.expand_on_focus = true;
    assert_eq!(
        verdict(&run(&mut expanding, "W06"), name),
        Some(Verdict::Fail)
    );
    let mut lowering = Docking::new();
    lowering.lower_on_focus = true;
    assert_eq!(
        verdict(&run(&mut lowering, "W06"), name),
        Some(Verdict::Fail)
    );
    // A 60 ms expansion right after focus returns, settled long before the window ends.
    let mut flashing = Docking::new();
    flashing.flash_on_focus = 60;
    assert_eq!(
        verdict(&run(&mut flashing, "W06"), name),
        Some(Verdict::Fail)
    );
    // A sampler that stalls cannot rule a brief change out: not tested, not passed.
    let mut stalled = Docking::new();
    stalled.read_cost = 80;
    assert_eq!(
        verdict(&run(&mut stalled, "W06"), name),
        Some(Verdict::NotTested)
    );
}

/// The product's mapped files: Menlo appears in `mapped_source` and/or once Preview shapes code.
struct Fonts {
    preview: bool,
    menlo_in_source: bool,
    menlo_in_preview: bool,
}
impl Backend for Fonts {
    fn now(&self) -> u64 {
        0
    }
    fn perform(&mut self, op: Op) -> Result<Value, String> {
        let menlo = if self.preview {
            self.menlo_in_preview
        } else {
            self.menlo_in_source
        };
        Ok(match op {
            Op::Fonts => Value::Text(
                "font selection: CJK=PingFang SC found=true Latin=Times New Roman found=true"
                    .into(),
            ),
            Op::Mapped => Value::Text(format!(
                "p42\nn/System/Library/Fonts/Times.ttc\n{}",
                if menlo {
                    "n/System/Library/Fonts/Menlo.ttc\n"
                } else {
                    ""
                }
            )),
            Op::Activate | Op::Wait(_) => Value::Unit,
            Op::Chord(keys) => {
                assert_eq!(keys, [CMD, 20]);
                self.preview = true;
                Value::Unit
            }
            Op::Config => Value::Text(format!(
                "view_mode = \"{}\"\n",
                if self.preview { "preview" } else { "source" }
            )),
            Op::Screenshot => Value::Text("NOT_TESTED screenshot".into()),
            op => return Err(format!("unexpected fake operation {op:?}")),
        })
    }
}

#[test]
fn f01_ties_menlo_to_the_preview_code_span_with_a_negative_control() {
    let name = "Menlo loaded once Preview shapes the code span (not proof of selection)";
    for (in_source, in_preview, expected) in [
        (false, true, Verdict::Pass),
        (false, false, Verdict::Fail),
        (true, true, Verdict::NotTested),
    ] {
        let mut fonts = Fonts {
            preview: false,
            menlo_in_source: in_source,
            menlo_in_preview: in_preview,
        };
        let row = run(&mut fonts, "F01");
        assert_eq!(verdict(&row, name), Some(expected), "{}", row.json());
        assert_eq!(
            verdict(&row, "glyph visual review"),
            Some(Verdict::NotTested)
        );
    }
}

#[test]
fn keys_go_only_to_the_focused_target_window() {
    use super::observation::{KeyTarget, key_target};
    let note = || Ok("StickyMD *".to_string());
    let dialog = || Ok("存储".to_string());
    assert!(key_target(Ok(42), note(), 42, KeyTarget::Note).is_ok());
    assert!(key_target(Ok(42), dialog(), 42, KeyTarget::Dialog).is_ok());
    // Focus taken by another application after activation: nothing is injected.
    assert!(key_target(Ok(7), note(), 42, KeyTarget::Note).unwrap_err().contains("PID 7"));
    // The product is frontmost but another of its windows has focus.
    assert!(key_target(Ok(42), dialog(), 42, KeyTarget::Note).is_err());
    assert!(key_target(Ok(42), note(), 42, KeyTarget::Dialog).is_err());
    // Unreadable focus injects nothing.
    assert!(key_target(Err("AX error -25204".into()), note(), 42, KeyTarget::Note).is_err());
    assert!(key_target(Ok(42), Err("AXFocusedWindow absent".into()), 42, KeyTarget::Note).is_err());
}

#[test]
fn an_unreadable_title_at_the_deadline_is_not_a_failed_cancel() {
    let mut unreadable = Quitting::new();
    unreadable.title_fails_after_quit = true;
    let row = run(&mut unreadable, "W11");
    assert_eq!(
        verdict(&row, "Cmd+Q: title shows the cancelled quit"),
        Some(Verdict::NotTested),
        "{}",
        row.json()
    );
    assert_eq!(verdict(&row, "Cmd+Q: process keeps running"), Some(Verdict::Pass));
}

#[test]
fn the_export_check_is_about_the_file_the_dialog_was_aimed_at() {
    let name = "the ended dialog exports nothing to the file it was aimed at";
    let row = run(&mut Quitting::new(), "W12");
    assert_eq!(verdict(&row, name), Some(Verdict::Pass), "{}", row.json());
    let mut exporting = Quitting::new();
    exporting.exports_on_end = true;
    assert_eq!(verdict(&run(&mut exporting, "W12"), name), Some(Verdict::Fail));
    let mut unaimed = Quitting::new();
    unaimed.aim_fails = true;
    assert_eq!(verdict(&run(&mut unaimed, "W12"), name), Some(Verdict::NotTested));
}

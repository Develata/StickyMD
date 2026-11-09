//! Window-domain values. No UI handle or platform type is authoritative here.
//!
//! plan_ref: docs/plan/09_windows_shell.md#windows-shell-purpose

use super::geometry::{MonitorGeometry, PhysicalRect, WindowPlacement};
use super::placement::PendingPlacement;
use super::topmost::TopmostState;
use crate::ports::{FailureKind, FrameFact};

pub const HOVER_REVEAL_DELAY_MS: u64 = 100;
pub const HOVER_LEAVE_COLLAPSE_DELAY_MS: u64 = 500;
pub const AUTO_COLLAPSE_DELAY_MS: u64 = 700;
pub const ANIMATION_DURATION_MS: u64 = 140;
pub(crate) const ANIMATION_FRAME_MS: u64 = 16;
/// How far an observed frame may be from its target on each edge and still be that target. A
/// measurement on a single display at 100%: 20 of 20 settled targets matched exactly. Mixed DPI is
/// unmeasured (P15-D10), so this may change.
pub const PLACEMENT_TOLERANCE_PX: u32 = 0;
/// How long a placement may take to be confirmed. In the same measurement, which looked 250 ms
/// out, 19 of 20 confirmations arrived within 100 ms; 500 ms leaves room for a loaded machine
/// without leaving a failed placement unreported for long.
pub const PLACEMENT_CONFIRM_TIMEOUT_MS: u64 = 500;
/// How long a show, hide, focus or topmost request may go without its receipt before it counts as
/// failed. Today's hosts answer these within the same dispatch, so this only ends a wait for a
/// host that does not: long enough not to cut off a slow compositor, short enough that a window
/// that never answers does not hold input off for long.
pub const WINDOW_ACTION_RECEIPT_TIMEOUT_MS: u64 = 2_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DockEdge {
    Left,
    Right,
    Top,
}

impl DockEdge {
    pub(crate) const fn tie_break_order(self) -> u8 {
        match self {
            Self::Top => 0,
            Self::Left => 1,
            Self::Right => 2,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StableVisibility {
    Floating,
    DockedExpanded(DockEdge),
    DockedCollapsed(DockEdge),
}

impl StableVisibility {
    pub const fn dock_edge(self) -> Option<DockEdge> {
        match self {
            Self::Floating => None,
            Self::DockedExpanded(edge) | Self::DockedCollapsed(edge) => Some(edge),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AnimationState {
    pub from: PhysicalRect,
    pub to: PhysicalRect,
    pub started_ms: u64,
    pub end_ms: u64,
    pub next_frame_ms: u64,
    pub final_visibility: StableVisibility,
}

impl AnimationState {
    pub fn frame_at(self, now_ms: u64) -> PhysicalRect {
        if now_ms >= self.end_ms {
            return self.to;
        }
        let elapsed = now_ms.saturating_sub(self.started_ms);
        let duration = self.end_ms.saturating_sub(self.started_ms);
        interpolate_rect(self.from, self.to, cubic_ease_out(elapsed, duration))
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum VisibilityState {
    Presented(StableVisibility),
    AwaitingHideSave {
        restore: StableVisibility,
    },
    /// The host was asked to hide the window (action `id`) and has not reported back; nothing that
    /// depends on the window being hidden is committed yet (plan 04 window-action-receipts).
    HidePending {
        id: u64,
        restore: StableVisibility,
        /// The edge sensor's topmost state to return to if the hide is rejected.
        sensor_topmost: bool,
        deadline_ms: u64,
    },
    HiddenToTray {
        restore: StableVisibility,
    },
    /// The host was asked to show the window (action `id`) and has not reported back; nothing that
    /// depends on the window being shown is committed yet (plan 04 window-action-receipts).
    /// `restore` is what the window was hidden from; a rejection goes back to it unchanged.
    ShowPending {
        id: u64,
        restore: StableVisibility,
        deadline_ms: u64,
    },
    Animating(AnimationState),
}

#[derive(Debug, Clone, PartialEq)]
pub struct DockState {
    pub edge: Option<DockEdge>,
    pub monitor: MonitorGeometry,
    pub offset_ratio: f64,
    pub manually_hidden: bool,
    pub hover_revealed: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct WindowGuardSnapshot {
    pub window_focused: bool,
    pub ime_composing: bool,
    pub dragging: bool,
    pub popup_open: bool,
    pub conflict_or_recovery: bool,
    pub paste_pending: bool,
    pub asset_transaction_pending: bool,
    pub note_save_required: bool,
    pub note_save_in_flight: bool,
}

impl WindowGuardSnapshot {
    pub const fn blocks_auto_collapse(self) -> bool {
        self.window_focused
            || self.ime_composing
            || self.dragging
            || self.popup_open
            || self.conflict_or_recovery
    }

    pub const fn blocks_quit(self) -> bool {
        self.conflict_or_recovery
    }

    pub const fn requires_note_barrier(self) -> bool {
        self.note_save_required || self.note_save_in_flight
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct WindowDeadlines {
    pub hover_reveal_ms: Option<u64>,
    pub auto_collapse_ms: Option<u64>,
    pub hover_leave_collapse_ms: Option<u64>,
}

impl WindowDeadlines {
    pub fn next(self) -> Option<u64> {
        [
            self.hover_reveal_ms,
            self.auto_collapse_ms,
            self.hover_leave_collapse_ms,
        ]
        .into_iter()
        .flatten()
        .min()
    }

    pub(crate) fn clear(&mut self) {
        *self = Self::default();
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuitStage {
    AwaitingPaste,
    AwaitingNoteSave,
    AwaitingAssetGc,
    AwaitingConfig,
    ReadyToExit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LifecycleState {
    Running,
    Quitting(QuitStage),
}

impl LifecycleState {
    pub const fn is_quitting(self) -> bool {
        matches!(self, Self::Quitting(_))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuitBarrier {
    Paste,
    NoteSave,
    AssetGc,
    Config,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShowReason {
    Tray,
    SecondInstance,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowSaveReason {
    HideToTray,
    Shutdown,
}

/// Who wants the window on top. The window's bit is the two together (`effective_topmost`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TopmostSource {
    User,
    Sensor,
    /// Asking for the confirmed values again after a request failed left the bit unknown.
    Reconcile,
}

/// A request for the window's topmost bit, whose receipt carries `id`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TopmostRequest {
    pub id: u64,
    pub desired_effective: bool,
    pub source: TopmostSource,
}

#[derive(Debug, Clone, PartialEq)]
pub enum WindowIntent {
    CloseRequested {
        now_ms: u64,
        guards: WindowGuardSnapshot,
    },
    ShowRequested {
        reason: ShowReason,
        now_ms: u64,
    },
    TrayToggleRequested {
        now_ms: u64,
        guards: WindowGuardSnapshot,
    },
    TrayQuitRequested {
        guards: WindowGuardSnapshot,
    },
    ManualCollapse {
        now_ms: u64,
    },
    EscapePressed {
        now_ms: u64,
    },
    SensorEntered {
        now_ms: u64,
    },
    PointerLeft {
        now_ms: u64,
    },
    GuardsChanged {
        guards: WindowGuardSnapshot,
        now_ms: u64,
    },
    /// The user asked to change the always-on-top preference to `on`.
    UserTopmostRequested {
        on: bool,
        now_ms: u64,
    },
    DragEnded {
        frame: PhysicalRect,
        monitor: MonitorGeometry,
        now_ms: u64,
    },
    DisplayTopologyChanged {
        monitors: Vec<MonitorGeometry>,
        now_ms: u64,
    },
    /// A native drag or resize began: the window is under the user's hand from now.
    MoveResizeStarted,
    /// That native drag or resize is over, whether or not a `DragEnded` follows.
    MoveResizeEnded,
    /// What the host saw of the window while placement `id` was waiting.
    PlacementObserved {
        id: u64,
        observed: Result<FrameFact, FailureKind>,
        now_ms: u64,
    },
    SplitModeChanged {
        split: bool,
    },
    Tick {
        now_ms: u64,
    },
    PasteSettled {
        guards: WindowGuardSnapshot,
        now_ms: u64,
    },
    HideSaveCompleted {
        succeeded: bool,
        guards: WindowGuardSnapshot,
        now_ms: u64,
    },
    QuitBarrierCompleted {
        barrier: QuitBarrier,
        succeeded: bool,
        guards: WindowGuardSnapshot,
    },
    /// The Execution Receipt of the window action `id`.
    ActionCompleted {
        id: u64,
        outcome: Result<(), FailureKind>,
        now_ms: u64,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum WindowEffect {
    /// Move the window to `target`. With `last` it is a placement that gets a verdict, under
    /// `id`; otherwise it is an animation frame, a presentation request with no receipt.
    ApplyFrame {
        id: u64,
        target: PhysicalRect,
        last: bool,
    },
    /// Report where the window is now, for placement `id`.
    ObservePlacement {
        id: u64,
    },
    PlacementFailed(FailureKind),
    /// Apply `desired_effective` to the window and report `WindowIntent::ActionCompleted`.
    ApplyTopmost(TopmostRequest),
    /// The window took the user's value: it is the configuration's now.
    CommitAlwaysOnTop(bool),
    TopmostRejected {
        source: TopmostSource,
        kind: FailureKind,
    },
    /// Show the window and report `WindowIntent::ActionCompleted` with this `id`.
    ShowWindow {
        id: u64,
    },
    /// The show committed: the tray says so and the view comes back.
    WindowShown,
    /// Hide the window and report `WindowIntent::ActionCompleted` with this `id`.
    HideWindow {
        id: u64,
    },
    /// Ask for focus and report whether the window is reachable. A success is not focus: only a
    /// real focus event proves that.
    FocusWindow {
        id: u64,
    },
    CancelImePreedit,
    SetImeAllowed(bool),
    SetEditorInputEnabled(bool),
    RequestNoteSave(WindowSaveReason),
    RequestSafeAssetGc,
    RequestConfigFlush,
    ExitProcess,
    CommitPlacement {
        placement: WindowPlacement,
        dock_edge: Option<DockEdge>,
    },
    ReleaseHiddenCaches,
    ReportTrayUnavailable,
    HideBlocked,
    HideCancelled,
    HideRejected(FailureKind),
    ShowRejected(FailureKind),
    QuitBlocked,
    QuitCancelled(QuitBarrier),
    QuitWarning(QuitBarrier),
    RequestRedraw,
    WakeAt(u64),
}

/// A focus request not yet reported on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct PendingFocus {
    pub(super) id: u64,
    pub(super) deadline_ms: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct WindowShellState {
    pub(super) visibility: VisibilityState,
    pub(super) dock: DockState,
    pub(super) placement: WindowPlacement,
    pub(super) frame: PhysicalRect,
    pub(super) guards: WindowGuardSnapshot,
    pub(super) deadlines: WindowDeadlines,
    pub(super) lifecycle: LifecycleState,
    pub(super) tray_available: bool,
    pub(super) hide_save_pending: bool,
    pub(super) topmost: TopmostState,
    /// The identity of the newest window action request.
    pub(super) last_window_action: u64,
    /// The newest focus request not yet reported on. A focus has no state of its own to commit,
    /// so this is what tells its receipt from a stale one.
    pub(super) focus_pending: Option<PendingFocus>,
    /// The placement and dock edge whose window frames were last confirmed. Only these reach the
    /// configuration when no frame can be confirmed (a topology change while hidden).
    pub(super) confirmed_placement: WindowPlacement,
    pub(super) confirmed_dock_edge: Option<DockEdge>,
    /// The placement waiting for its verdict.
    pub(super) placement_pending: Option<PendingPlacement>,
    /// A native drag or resize is running: no frame can be judged until it ends.
    pub(super) native_move_active: bool,
}

impl WindowShellState {
    pub fn visibility(&self) -> VisibilityState {
        self.visibility
    }

    #[cfg(test)]
    pub fn dock(&self) -> &DockState {
        &self.dock
    }

    /// The guards the coordinator last received, as opposed to the shell's current facts.
    pub fn guards(&self) -> WindowGuardSnapshot {
        self.guards
    }

    pub fn frame(&self) -> PhysicalRect {
        self.frame
    }

    /// Whether the edge sensor wants the effective topmost projection, as last requested, without
    /// changing the user's configured always-on-top preference. The value a receipt has proved is
    /// not this one until that receipt comes in.
    pub fn temporary_sensor_topmost(&self) -> bool {
        self.topmost.sensor_target()
    }

    /// The placement still waiting for its verdict, which is when the window is observed.
    pub fn pending_placement_id(&self) -> Option<u64> {
        self.placement_pending.as_ref().map(PendingPlacement::id)
    }

    pub fn lifecycle(&self) -> LifecycleState {
        self.lifecycle
    }

    pub fn next_deadline_ms(&self) -> Option<u64> {
        let animation = match self.visibility {
            VisibilityState::Animating(animation) => {
                Some(animation.next_frame_ms.min(animation.end_ms))
            }
            _ => None,
        };
        let placement = self
            .placement_pending
            .as_ref()
            .map(PendingPlacement::deadline_ms);
        let visibility_action = match self.visibility {
            VisibilityState::HidePending { deadline_ms, .. }
            | VisibilityState::ShowPending { deadline_ms, .. } => Some(deadline_ms),
            _ => None,
        };
        let focus = self.focus_pending.map(|focus| focus.deadline_ms);
        [
            self.deadlines.next(),
            animation,
            placement,
            visibility_action,
            focus,
            self.topmost.deadline_ms(),
        ]
        .into_iter()
        .flatten()
        .min()
    }

    pub fn accepts_editor_mutation(&self) -> bool {
        self.lifecycle == LifecycleState::Running
            && matches!(
                self.visibility,
                VisibilityState::Presented(
                    StableVisibility::Floating | StableVisibility::DockedExpanded(_)
                )
            )
            && !(self.dock.hover_revealed && !self.guards.window_focused)
    }
}

pub fn cubic_ease_out(elapsed_ms: u64, duration_ms: u64) -> f64 {
    if duration_ms == 0 {
        return 1.0;
    }
    let t = (elapsed_ms as f64 / duration_ms as f64).clamp(0.0, 1.0);
    1.0 - (1.0 - t).powi(3)
}

/// Resolves the actual z-order projection without mutating the configured
/// always-on-top preference.
pub const fn effective_topmost(configured: bool, temporary_sensor: bool) -> bool {
    configured || temporary_sensor
}

fn interpolate_rect(from: PhysicalRect, to: PhysicalRect, progress: f64) -> PhysicalRect {
    PhysicalRect::new(
        interpolate_i32(from.x, to.x, progress),
        interpolate_i32(from.y, to.y, progress),
        interpolate_u32(from.width, to.width, progress),
        interpolate_u32(from.height, to.height, progress),
    )
}

fn interpolate_i32(from: i32, to: i32, progress: f64) -> i32 {
    (f64::from(from) + (f64::from(to) - f64::from(from)) * progress)
        .round()
        .clamp(f64::from(i32::MIN), f64::from(i32::MAX)) as i32
}

fn interpolate_u32(from: u32, to: u32, progress: f64) -> u32 {
    (f64::from(from) + (f64::from(to) - f64::from(from)) * progress)
        .round()
        .clamp(0.0, f64::from(u32::MAX)) as u32
}

#[cfg(test)]
mod phase8_window_tests {
    use super::*;

    #[test]
    fn cubic_ease_out_has_exact_endpoints_and_advances_quickly() {
        assert_eq!(cubic_ease_out(0, 140), 0.0);
        assert_eq!(cubic_ease_out(140, 140), 1.0);
        assert_eq!(cubic_ease_out(200, 140), 1.0);
        assert!(cubic_ease_out(70, 140) > 0.5);
    }

    #[test]
    fn effective_topmost_keeps_configured_and_sensor_authorities_separate() {
        assert!(!effective_topmost(false, false));
        assert!(effective_topmost(true, false));
        assert!(effective_topmost(false, true));
        assert!(effective_topmost(true, true));
    }

    #[test]
    fn all_auto_collapse_guards_are_explicit() {
        for blocked in [
            WindowGuardSnapshot {
                window_focused: true,
                ..Default::default()
            },
            WindowGuardSnapshot {
                ime_composing: true,
                ..Default::default()
            },
            WindowGuardSnapshot {
                dragging: true,
                ..Default::default()
            },
            WindowGuardSnapshot {
                popup_open: true,
                ..Default::default()
            },
            WindowGuardSnapshot {
                conflict_or_recovery: true,
                ..Default::default()
            },
        ] {
            assert!(blocked.blocks_auto_collapse());
        }
        assert!(!WindowGuardSnapshot::default().blocks_auto_collapse());
    }
}

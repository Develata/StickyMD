//! Window-domain values. No UI handle or platform type is authoritative here.
//!
//! plan_ref: docs/plan/09_windows_shell.md#windows-shell-purpose

use super::geometry::{MonitorGeometry, PhysicalRect, WindowPlacement};

pub const HOVER_REVEAL_DELAY_MS: u64 = 100;
pub const HOVER_LEAVE_COLLAPSE_DELAY_MS: u64 = 500;
pub const AUTO_COLLAPSE_DELAY_MS: u64 = 700;
pub const ANIMATION_DURATION_MS: u64 = 140;
pub(crate) const ANIMATION_FRAME_MS: u64 = 16;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DockEdge {
    Left,
    Right,
    Top,
}

impl DockEdge {
    pub(crate) const fn tie_break_order(self) -> u8 {
        match self {
            Self::Left => 0,
            Self::Right => 1,
            Self::Top => 2,
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
    AwaitingHideSave { restore: StableVisibility },
    HiddenToTray { restore: StableVisibility },
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
    DragEnded {
        frame: PhysicalRect,
        monitor: MonitorGeometry,
        now_ms: u64,
    },
    DisplayTopologyChanged {
        monitors: Vec<MonitorGeometry>,
    },
    SplitModeChanged {
        split: bool,
    },
    Tick {
        now_ms: u64,
    },
    PasteSettled {
        guards: WindowGuardSnapshot,
    },
    HideSaveCompleted {
        succeeded: bool,
        guards: WindowGuardSnapshot,
    },
    QuitBarrierCompleted {
        barrier: QuitBarrier,
        succeeded: bool,
        guards: WindowGuardSnapshot,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum WindowEffect {
    ApplyFrame(PhysicalRect),
    SetTemporarySensorTopmost(bool),
    SetVisible(bool),
    FocusWindow,
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
    QuitBlocked,
    QuitCancelled(QuitBarrier),
    QuitWarning(QuitBarrier),
    RequestRedraw,
    WakeAt(u64),
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
    pub(super) pre_split_width_dip: Option<f64>,
    pub(super) temporary_sensor_topmost: bool,
}

impl WindowShellState {
    pub fn visibility(&self) -> VisibilityState {
        self.visibility
    }

    #[cfg(test)]
    pub fn dock(&self) -> &DockState {
        &self.dock
    }

    pub fn frame(&self) -> PhysicalRect {
        self.frame
    }

    /// Whether the edge sensor needs an effective topmost projection without
    /// changing the user's configured always-on-top preference.
    pub fn temporary_sensor_topmost(&self) -> bool {
        self.temporary_sensor_topmost
    }

    #[cfg(test)]
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
        [self.deadlines.next(), animation]
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

//! Always-on-top as request, receipt and commit. The window has one bit, the effective value of the
//! user's preference and the edge sensor's temporary state, so requests are settled in one order:
//! only the newest native request can be judged, and a success confirms both sources at once. The
//! user's value is what the configuration ends up holding; the sensor's never reaches it (plan 04
//! window-action-receipts).
//!
//! plan_ref: docs/plan/04_runtime_state_model.md#window-action-receipts

use super::reducer::WindowShellCoordinator;
use super::state::{
    TopmostRequest, TopmostSource, WINDOW_ACTION_RECEIPT_TIMEOUT_MS, WindowEffect,
    effective_topmost,
};
use crate::ports::FailureKind;

/// The newest native request: what both sources would be if it succeeds. An older request is
/// stale from the moment this replaced it, and a newer one always carries the current targets of
/// both sources, so a replaced request needs no reconciling of its own.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct PendingTopmost {
    id: u64,
    user: bool,
    sensor: bool,
    deadline_ms: u64,
    source: TopmostSource,
}

/// Confirmed values are the ones the last successful receipt proved together.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TopmostState {
    pub(super) user_confirmed: bool,
    pub(super) sensor_confirmed: bool,
    pub(super) in_flight: Option<PendingTopmost>,
    /// A request failed and nothing has proved the native bit since, so it may not match the
    /// confirmed values and a repeat of them must still reach the host.
    pub(super) native_bit_unknown: bool,
}

impl TopmostState {
    pub(super) fn user_target(self) -> bool {
        self.in_flight.map_or(self.user_confirmed, |p| p.user)
    }

    pub(super) fn sensor_target(self) -> bool {
        self.in_flight.map_or(self.sensor_confirmed, |p| p.sensor)
    }

    /// Whether asking for a value that is already the target would add nothing: the same request
    /// is out, or the native bit is known to match it.
    fn repeat_is_redundant(self) -> bool {
        self.in_flight.is_some() || !self.native_bit_unknown
    }

    pub(super) fn deadline_ms(self) -> Option<u64> {
        self.in_flight.map(|p| p.deadline_ms)
    }

    #[cfg(test)]
    pub(super) fn in_flight_id(self) -> Option<u64> {
        self.in_flight.map(|p| p.id)
    }

    pub(super) fn overdue(self, now_ms: u64) -> Option<u64> {
        self.in_flight
            .filter(|p| now_ms >= p.deadline_ms)
            .map(|p| p.id)
    }
}

impl WindowShellCoordinator {
    /// The user's preference, read back before it reaches the configuration. A value that is
    /// already the target asks for nothing unless the native bit may not match it.
    pub(super) fn request_user_topmost(
        &mut self,
        on: bool,
        now_ms: u64,
        effects: &mut Vec<WindowEffect>,
    ) {
        if self.state.topmost.user_target() == on && self.state.topmost.repeat_is_redundant() {
            return;
        }
        let sensor = self.state.topmost.sensor_target();
        self.request_topmost(TopmostSource::User, on, sensor, now_ms, effects);
    }

    /// The edge sensor's temporary z-order. The configuration is not touched in this path.
    pub(super) fn set_temporary_sensor_topmost(
        &mut self,
        enabled: bool,
        now_ms: u64,
        effects: &mut Vec<WindowEffect>,
    ) {
        if self.state.topmost.sensor_target() == enabled && self.state.topmost.repeat_is_redundant()
        {
            return;
        }
        let user = self.state.topmost.user_target();
        self.request_topmost(TopmostSource::Sensor, user, enabled, now_ms, effects);
    }

    /// The startup applies the configured value outside this flow. When that failed, nothing
    /// proves the native bit, so asking for the configured value again must still reach the host.
    pub fn mark_native_topmost_unknown(&mut self) {
        self.state.topmost.native_bit_unknown = true;
    }

    fn request_topmost(
        &mut self,
        source: TopmostSource,
        user: bool,
        sensor: bool,
        now_ms: u64,
        effects: &mut Vec<WindowEffect>,
    ) {
        let Some(id) = self.next_window_action() else {
            effects.push(WindowEffect::TopmostRejected {
                source,
                kind: FailureKind::Failed,
            });
            return;
        };
        self.state.topmost.in_flight = Some(PendingTopmost {
            id,
            user,
            sensor,
            deadline_ms: now_ms.saturating_add(WINDOW_ACTION_RECEIPT_TIMEOUT_MS),
            source,
        });
        effects.push(WindowEffect::ApplyTopmost(TopmostRequest {
            id,
            desired_effective: effective_topmost(user, sensor),
            source,
        }));
    }

    /// Whether action `id` was the newest topmost request. It proves the bit only, not the z-order
    /// against other windows or focus. A failure leaves the native bit unknown, so the confirmed
    /// values are asked for again, once.
    pub(super) fn complete_topmost(
        &mut self,
        id: u64,
        outcome: Result<(), FailureKind>,
        now_ms: u64,
        effects: &mut Vec<WindowEffect>,
    ) -> bool {
        let Some(pending) = self.state.topmost.in_flight.filter(|p| p.id == id) else {
            return false;
        };
        self.state.topmost.in_flight = None;
        match outcome {
            Ok(()) => {
                self.state.topmost.native_bit_unknown = false;
                let user_changed = pending.user != self.state.topmost.user_confirmed;
                self.state.topmost.user_confirmed = pending.user;
                self.state.topmost.sensor_confirmed = pending.sensor;
                if user_changed {
                    effects.push(WindowEffect::CommitAlwaysOnTop(pending.user));
                }
            }
            Err(kind) if pending.source == TopmostSource::Reconcile => {
                self.state.topmost.native_bit_unknown = true;
                effects.push(WindowEffect::TopmostRejected {
                    source: TopmostSource::Reconcile,
                    kind,
                });
            }
            Err(kind) => {
                self.state.topmost.native_bit_unknown = true;
                let source = if pending.user == self.state.topmost.user_confirmed {
                    pending.source
                } else {
                    TopmostSource::User
                };
                effects.push(WindowEffect::TopmostRejected { source, kind });
                let user = self.state.topmost.user_confirmed;
                let sensor = self.state.topmost.sensor_confirmed;
                self.request_topmost(TopmostSource::Reconcile, user, sensor, now_ms, effects);
            }
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::super::geometry::{MonitorGeometry, MonitorIdentity, PhysicalRect, WindowPlacement};
    use super::super::state::{
        DockEdge, StableVisibility, VisibilityState, WindowGuardSnapshot, WindowIntent,
    };
    use super::*;

    const REJECTIONS: [FailureKind; 3] = [
        FailureKind::Failed,
        FailureKind::Unavailable,
        FailureKind::Unsupported,
    ];

    fn coordinator(visibility: StableVisibility) -> WindowShellCoordinator {
        WindowShellCoordinator::new(
            WindowPlacement::new(520.0, 680.0, None, 0.5, 0.5, 0.25),
            visibility,
            MonitorGeometry::new(
                MonitorIdentity::new("primary"),
                PhysicalRect::new(0, 0, 1920, 1080),
                1.0,
                true,
            ),
            true,
        )
    }

    fn user(coordinator: &mut WindowShellCoordinator, on: bool) -> Vec<WindowEffect> {
        coordinator.dispatch(WindowIntent::UserTopmostRequested { on, now_ms: 5 })
    }

    fn sensor(coordinator: &mut WindowShellCoordinator, on: bool) -> Vec<WindowEffect> {
        let mut effects = Vec::new();
        coordinator.set_temporary_sensor_topmost(on, 5, &mut effects);
        effects
    }

    fn request(effects: &[WindowEffect], source: TopmostSource) -> Option<TopmostRequest> {
        effects.iter().find_map(|effect| match effect {
            WindowEffect::ApplyTopmost(request) if request.source == source => Some(*request),
            _ => None,
        })
    }

    fn user_id(effects: &[WindowEffect]) -> u64 {
        request(effects, TopmostSource::User)
            .expect("a user request")
            .id
    }

    fn sensor_id(effects: &[WindowEffect]) -> u64 {
        request(effects, TopmostSource::Sensor)
            .expect("a sensor request")
            .id
    }

    fn receipt(
        coordinator: &mut WindowShellCoordinator,
        id: u64,
        outcome: Result<(), FailureKind>,
    ) -> Vec<WindowEffect> {
        coordinator.dispatch(WindowIntent::ActionCompleted {
            id,
            outcome,
            now_ms: 5,
        })
    }

    #[test]
    fn a_user_request_carries_the_effective_value_and_commits_on_success() {
        let mut coordinator = coordinator(StableVisibility::Floating);
        let effects = user(&mut coordinator, true);
        assert_eq!(
            request(&effects, TopmostSource::User),
            Some(TopmostRequest {
                id: 1,
                desired_effective: true,
                source: TopmostSource::User
            })
        );
        assert!(!coordinator.state.topmost.user_confirmed);
        assert_eq!(
            receipt(&mut coordinator, 1, Ok(())),
            [WindowEffect::CommitAlwaysOnTop(true)]
        );
        assert!(coordinator.state.topmost.user_confirmed);
        assert_eq!(coordinator.state.topmost.in_flight, None);
    }

    /// A failure leaves the native bit unknown: the value that was confirmed is asked for again,
    /// and a failure of that request ends it there.
    #[test]
    fn a_rejected_user_request_keeps_the_value_reports_and_reconciles_once() {
        for kind in REJECTIONS {
            let mut coordinator = coordinator(StableVisibility::Floating);
            let id = user_id(&user(&mut coordinator, true));
            let effects = receipt(&mut coordinator, id, Err(kind));
            assert_eq!(
                effects[0],
                WindowEffect::TopmostRejected {
                    source: TopmostSource::User,
                    kind
                }
            );
            let reconcile = request(&effects, TopmostSource::Reconcile).expect("a reconcile");
            assert!(!reconcile.desired_effective);
            assert!(!coordinator.state.topmost.user_confirmed);
            assert!(!coordinator.state.topmost.user_target());

            let effects = receipt(&mut coordinator, reconcile.id, Err(kind));
            assert_eq!(
                effects,
                [WindowEffect::TopmostRejected {
                    source: TopmostSource::Reconcile,
                    kind
                }]
            );
            assert_eq!(coordinator.state.topmost.in_flight, None);
        }
    }

    #[test]
    fn a_stale_topmost_receipt_commits_nothing() {
        let mut coordinator = coordinator(StableVisibility::Floating);
        let id = user_id(&user(&mut coordinator, true));
        let before = coordinator.state.topmost;
        assert!(receipt(&mut coordinator, id + 100, Ok(())).is_empty());
        assert_eq!(coordinator.state.topmost, before);
    }

    /// The newer request carries both current targets, so the older receipt, whatever it says,
    /// has nothing left to settle.
    #[test]
    fn a_newer_request_supersedes_the_older_receipt() {
        let mut coordinator = coordinator(StableVisibility::Floating);
        let first = user_id(&user(&mut coordinator, true));
        let second = user_id(&user(&mut coordinator, false));
        assert_ne!(first, second);
        assert!(receipt(&mut coordinator, first, Ok(())).is_empty());
        assert!(receipt(&mut coordinator, first, Err(FailureKind::Failed)).is_empty());
        assert!(!coordinator.state.topmost.user_confirmed);
        assert!(receipt(&mut coordinator, second, Ok(())).is_empty());
        assert!(!coordinator.state.topmost.user_confirmed);
    }

    /// The reviewed sequence: a sensor request from a collapse is replaced by the user turning the
    /// preference off. The user's receipt settles the one native bit for both; the sensor's late
    /// failure cannot make the confirmed values disagree with the bit that was proven.
    #[test]
    fn a_composed_bit_settles_in_one_order() {
        let edge = StableVisibility::DockedExpanded(DockEdge::Left);
        let mut coordinator = coordinator(edge).with_user_topmost(true);
        let collapse = coordinator.dispatch(WindowIntent::ManualCollapse { now_ms: 5 });
        let s = request(&collapse, TopmostSource::Sensor).expect("the sensor's request");
        assert!(s.desired_effective);
        let u = request(&user(&mut coordinator, false), TopmostSource::User).expect("a request");
        assert!(u.desired_effective, "the sensor still holds the window up");
        assert_eq!(
            receipt(&mut coordinator, u.id, Ok(())),
            [WindowEffect::CommitAlwaysOnTop(false)]
        );
        assert!(receipt(&mut coordinator, s.id, Err(FailureKind::Failed)).is_empty());
        let topmost = coordinator.state.topmost;
        assert!(!topmost.user_confirmed);
        assert!(topmost.sensor_confirmed);
        assert_eq!(topmost.in_flight, None);
        assert_eq!(
            effective_topmost(topmost.user_confirmed, topmost.sensor_confirmed),
            u.desired_effective,
            "the confirmed values agree with the last proven bit"
        );
    }

    /// A newer sensor request confirms the user's still-unconfirmed value with it.
    #[test]
    fn a_sensor_success_carries_the_users_pending_value() {
        let mut coordinator = coordinator(StableVisibility::Floating);
        user(&mut coordinator, true);
        let id = sensor_id(&sensor(&mut coordinator, true));
        assert_eq!(
            receipt(&mut coordinator, id, Ok(())),
            [WindowEffect::CommitAlwaysOnTop(true)]
        );
        let topmost = coordinator.state.topmost;
        assert!(topmost.user_confirmed);
        assert!(topmost.sensor_confirmed);
    }

    /// What was rejected stays unconfirmed, so the same value can be asked for again, including
    /// while the reconciling request is still out.
    #[test]
    fn a_rejected_user_request_can_be_retried() {
        let mut coordinator = coordinator(StableVisibility::Floating);
        let id = user_id(&user(&mut coordinator, true));
        receipt(&mut coordinator, id, Err(FailureKind::Failed));
        let id = user_id(&user(&mut coordinator, true));
        assert_eq!(
            receipt(&mut coordinator, id, Ok(())),
            [WindowEffect::CommitAlwaysOnTop(true)]
        );
    }

    /// A failed request that carries an unconfirmed user change is the user's failure, whichever
    /// source sent it; otherwise it is the failure of the source that sent it.
    #[test]
    fn a_failed_sensor_request_carrying_an_unconfirmed_user_value_is_reported_as_user() {
        let mut coordinator = coordinator(StableVisibility::Floating);
        user(&mut coordinator, true);
        let id = sensor_id(&sensor(&mut coordinator, true));
        let effects = receipt(&mut coordinator, id, Err(FailureKind::Failed));
        assert_eq!(
            effects[0],
            WindowEffect::TopmostRejected {
                source: TopmostSource::User,
                kind: FailureKind::Failed
            }
        );
    }

    /// The request and its reconcile failed, so the native bit may not match the confirmed value:
    /// nothing retries by itself, but an explicit repeat of that value reaches the host, and a
    /// success makes repeats redundant again.
    #[test]
    fn an_explicit_retry_reaches_the_host_after_a_failed_reconcile() {
        let failed_reconcile = |coordinator: &mut WindowShellCoordinator,
                                effects: &[WindowEffect]| {
            let id = effects
                .iter()
                .find_map(|effect| match effect {
                    WindowEffect::ApplyTopmost(request) => Some(request.id),
                    _ => None,
                })
                .expect("a request");
            let effects = receipt(coordinator, id, Err(FailureKind::Failed));
            let reconcile = request(&effects, TopmostSource::Reconcile).unwrap();
            let effects = receipt(coordinator, reconcile.id, Err(FailureKind::Failed));
            assert_eq!(
                effects,
                [WindowEffect::TopmostRejected {
                    source: TopmostSource::Reconcile,
                    kind: FailureKind::Failed
                }],
                "nothing retries by itself"
            );
            assert_eq!(coordinator.state.topmost.in_flight, None);
        };

        let mut coordinator = coordinator(StableVisibility::Floating);
        let requested = user(&mut coordinator, true);
        failed_reconcile(&mut coordinator, &requested);
        let retry = user(&mut coordinator, false);
        let retried = request(&retry, TopmostSource::User).expect("the retry reaches the host");
        assert!(!retried.desired_effective);
        assert!(user(&mut coordinator, false).is_empty(), "already out");
        assert!(receipt(&mut coordinator, retried.id, Ok(())).is_empty());
        assert!(
            user(&mut coordinator, false).is_empty(),
            "known to match now"
        );

        let requested = sensor(&mut coordinator, true);
        failed_reconcile(&mut coordinator, &requested);
        let retry = sensor(&mut coordinator, false);
        let retried = request(&retry, TopmostSource::Sensor).expect("the retry reaches the host");
        assert!(!retried.desired_effective);
        assert!(sensor(&mut coordinator, false).is_empty(), "already out");
        assert!(receipt(&mut coordinator, retried.id, Ok(())).is_empty());
        assert!(
            sensor(&mut coordinator, false).is_empty(),
            "known to match now"
        );
    }

    #[test]
    fn a_value_already_held_or_pending_asks_for_nothing() {
        let mut coordinator = coordinator(StableVisibility::Floating);
        assert!(user(&mut coordinator, false).is_empty());
        user(&mut coordinator, true);
        assert!(user(&mut coordinator, true).is_empty());
    }

    /// The sensor's request carries the configured value too, and the receipt never reaches it.
    #[test]
    fn a_sensor_request_stays_separate_from_the_user_value() {
        let mut coordinator = coordinator(StableVisibility::Floating).with_user_topmost(true);
        let effects = sensor(&mut coordinator, true);
        assert_eq!(
            request(&effects, TopmostSource::Sensor).unwrap(),
            TopmostRequest {
                id: 1,
                desired_effective: true,
                source: TopmostSource::Sensor
            }
        );
        assert!(!coordinator.state.topmost.sensor_confirmed);
        assert!(coordinator.state().temporary_sensor_topmost());
        let effects = receipt(&mut coordinator, 1, Ok(()));
        assert!(effects.is_empty(), "the sensor writes no configuration");
        assert!(coordinator.state.topmost.sensor_confirmed);
        assert!(coordinator.state.topmost.user_confirmed);
    }

    /// The effective value is the two sources together: turning the user's off while the sensor
    /// holds the window up still leaves the bit on, and that is what the receipt must prove.
    #[test]
    fn the_request_is_the_effective_of_both_sources() {
        let mut first = coordinator(StableVisibility::Floating).with_user_topmost(true);
        first.state.topmost.sensor_confirmed = true;
        let effects = user(&mut first, false);
        assert!(
            request(&effects, TopmostSource::User)
                .unwrap()
                .desired_effective
        );

        let mut second = coordinator(StableVisibility::Floating);
        user(&mut second, true);
        sensor(&mut second, true);
        let off = sensor(&mut second, false);
        assert!(
            request(&off, TopmostSource::Sensor)
                .unwrap()
                .desired_effective,
            "the pending user value counts"
        );
    }

    #[test]
    fn a_rejected_sensor_request_keeps_its_confirmed_value_and_reports() {
        for kind in REJECTIONS {
            let mut coordinator = coordinator(StableVisibility::Floating);
            let id = sensor_id(&sensor(&mut coordinator, true));
            let effects = receipt(&mut coordinator, id, Err(kind));
            assert_eq!(
                effects[0],
                WindowEffect::TopmostRejected {
                    source: TopmostSource::Sensor,
                    kind
                }
            );
            assert!(request(&effects, TopmostSource::Reconcile).is_some());
            assert!(!coordinator.state.topmost.sensor_confirmed);
            assert!(!coordinator.state().temporary_sensor_topmost());
            let retried = sensor(&mut coordinator, true);
            assert!(
                request(&retried, TopmostSource::Sensor).is_some(),
                "retried"
            );
        }
    }

    /// A topmost receipt is not a window receipt: it reaches its own pending state only.
    #[test]
    fn a_topmost_receipt_never_settles_a_visibility_action() {
        let mut coordinator = coordinator(StableVisibility::Floating);
        let id = user_id(&user(&mut coordinator, true));
        let close = coordinator.dispatch(WindowIntent::CloseRequested {
            now_ms: 0,
            guards: WindowGuardSnapshot::default(),
        });
        assert!(close.contains(&WindowEffect::HideWindow { id: id + 1 }));
        assert_eq!(
            receipt(&mut coordinator, id, Ok(())),
            [WindowEffect::CommitAlwaysOnTop(true)]
        );
        assert!(matches!(
            coordinator.state().visibility(),
            VisibilityState::HidePending { .. }
        ));
    }

    /// The hide records what the sensor last proved, not what it last asked for.
    #[test]
    fn a_hide_saves_the_confirmed_sensor_value() {
        let edge = StableVisibility::DockedExpanded(DockEdge::Left);
        let mut coordinator = coordinator(edge);
        sensor(&mut coordinator, true);
        assert!(!coordinator.state.topmost.sensor_confirmed);
        let effects = coordinator.dispatch(WindowIntent::CloseRequested {
            now_ms: 0,
            guards: WindowGuardSnapshot::default(),
        });
        let VisibilityState::HidePending {
            sensor_topmost, id, ..
        } = coordinator.state().visibility()
        else {
            panic!("the close hides at once");
        };
        assert!(!sensor_topmost);
        assert!(effects.contains(&WindowEffect::HideWindow { id }));
    }

    #[test]
    fn a_rejected_hide_restores_the_sensor_value_it_saved() {
        let edge = StableVisibility::DockedExpanded(DockEdge::Left);
        let mut coordinator = coordinator(edge);
        coordinator.state.topmost.sensor_confirmed = true;
        let close = coordinator.dispatch(WindowIntent::CloseRequested {
            now_ms: 0,
            guards: WindowGuardSnapshot::default(),
        });
        let hide = close
            .iter()
            .find_map(|effect| match effect {
                WindowEffect::HideWindow { id } => Some(*id),
                _ => None,
            })
            .expect("a hide");
        // The hide took the sensor off the top: answering that first leaves it confirmed false.
        let off = sensor_id(&close);
        receipt(&mut coordinator, off, Ok(()));
        assert!(!coordinator.state.topmost.sensor_confirmed);
        let effects = receipt(&mut coordinator, hide, Err(FailureKind::Failed));
        assert_eq!(
            request(&effects, TopmostSource::Sensor).map(|r| r.desired_effective),
            Some(true)
        );
    }

    /// With no receipt the deadline settles it like a failure: the id dies, the value is reported
    /// and reconciled, the same value can be asked for again, and a late receipt commits nothing.
    #[test]
    fn a_topmost_request_with_no_receipt_ends_at_its_deadline() {
        let mut coordinator = coordinator(StableVisibility::Floating);
        let id = user_id(&user(&mut coordinator, true));
        let deadline = 5 + WINDOW_ACTION_RECEIPT_TIMEOUT_MS;
        assert_eq!(coordinator.state().next_deadline_ms(), Some(deadline));
        assert!(
            coordinator
                .dispatch(WindowIntent::Tick {
                    now_ms: deadline - 1,
                })
                .is_empty()
        );
        let effects = coordinator.dispatch(WindowIntent::Tick { now_ms: deadline });
        assert_eq!(
            effects[0],
            WindowEffect::TopmostRejected {
                source: TopmostSource::User,
                kind: FailureKind::Failed
            }
        );
        let reconcile = request(&effects, TopmostSource::Reconcile).unwrap();
        assert!(receipt(&mut coordinator, id, Ok(())).is_empty());
        assert!(!coordinator.state.topmost.user_confirmed);
        // The reconcile is out and has no answer yet: the same value can still be asked for.
        let retry = user(&mut coordinator, true);
        let retried = user_id(&retry);
        assert_ne!(retried, reconcile.id);
        assert_eq!(
            receipt(&mut coordinator, retried, Ok(())),
            [WindowEffect::CommitAlwaysOnTop(true)]
        );
    }

    #[test]
    fn the_configured_value_seeds_the_flow() {
        let coordinator = coordinator(StableVisibility::Floating).with_user_topmost(true);
        assert!(coordinator.state.topmost.user_confirmed);
        assert!(coordinator.state.topmost.user_target());
    }
}

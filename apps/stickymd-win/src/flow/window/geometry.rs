//! Pure window geometry in signed physical coordinates with per-monitor DIP scaling.
//!
//! plan_ref: docs/plan/09_windows_shell.md#platform-adapter-boundary

use super::state::DockEdge;

pub const SNAP_DISTANCE_DIP: f64 = 24.0;
pub const SNAP_TIE_EPSILON_DIP: f64 = 1.0;
pub const UNDOCK_DISTANCE_DIP: f64 = 16.0;
pub const SENSOR_THICKNESS_DIP: f64 = 3.0;
pub const MIN_WINDOW_WIDTH_DIP: f64 = 220.0;
pub const MIN_WINDOW_HEIGHT_DIP: f64 = 120.0;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct MonitorIdentity(String);

impl MonitorIdentity {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PhysicalRect {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

impl PhysicalRect {
    pub const fn new(x: i32, y: i32, width: u32, height: u32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    pub fn right(self) -> i64 {
        i64::from(self.x) + i64::from(self.width)
    }

    #[cfg(test)]
    pub fn bottom(self) -> i64 {
        i64::from(self.y) + i64::from(self.height)
    }

    #[cfg(test)]
    pub fn contains(self, other: Self) -> bool {
        i64::from(other.x) >= i64::from(self.x)
            && i64::from(other.y) >= i64::from(self.y)
            && other.right() <= self.right()
            && other.bottom() <= self.bottom()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct MonitorGeometry {
    pub identity: MonitorIdentity,
    pub work_area: PhysicalRect,
    pub scale_factor: f64,
    pub primary: bool,
}

impl MonitorGeometry {
    pub fn new(
        identity: MonitorIdentity,
        work_area: PhysicalRect,
        scale_factor: f64,
        primary: bool,
    ) -> Self {
        Self {
            identity,
            work_area,
            scale_factor: valid_scale(scale_factor),
            primary,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct WindowPlacement {
    pub width_dip: f64,
    pub height_dip: f64,
    pub monitor_identity: Option<MonitorIdentity>,
    pub dock_offset_ratio: f64,
    pub floating_x_ratio: f64,
    pub floating_y_ratio: f64,
}

impl WindowPlacement {
    pub fn new(
        width_dip: f64,
        height_dip: f64,
        monitor_identity: Option<MonitorIdentity>,
        dock_offset_ratio: f64,
        floating_x_ratio: f64,
        floating_y_ratio: f64,
    ) -> Self {
        Self {
            width_dip: finite_with_minimum(width_dip, MIN_WINDOW_WIDTH_DIP),
            height_dip: finite_with_minimum(height_dip, MIN_WINDOW_HEIGHT_DIP),
            monitor_identity,
            dock_offset_ratio: ratio(dock_offset_ratio),
            floating_x_ratio: ratio(floating_x_ratio),
            floating_y_ratio: ratio(floating_y_ratio),
        }
    }
}

pub fn sensor_thickness_px(monitor: &MonitorGeometry) -> u32 {
    dip_to_px(SENSOR_THICKNESS_DIP, monitor.scale_factor).max(1)
}

pub fn snap_edge(frame: PhysicalRect, monitor: &MonitorGeometry) -> Option<DockEdge> {
    let scale = valid_scale(monitor.scale_factor);
    let work = monitor.work_area;
    let candidates = [
        (
            DockEdge::Left,
            (i64::from(frame.x) - i64::from(work.x)).unsigned_abs() as f64 / scale,
        ),
        (
            DockEdge::Right,
            (frame.right() - work.right()).unsigned_abs() as f64 / scale,
        ),
        (
            DockEdge::Top,
            (i64::from(frame.y) - i64::from(work.y)).unsigned_abs() as f64 / scale,
        ),
    ];
    let nearest = candidates
        .iter()
        .map(|(_, distance)| *distance)
        .filter(|distance| *distance <= SNAP_DISTANCE_DIP)
        .min_by(f64::total_cmp)?;
    candidates
        .into_iter()
        .filter(|(_, distance)| {
            *distance <= SNAP_DISTANCE_DIP && *distance <= nearest + SNAP_TIE_EPSILON_DIP
        })
        .min_by_key(|(edge, _)| edge.tie_break_order())
        .map(|(edge, _)| edge)
}

pub fn should_undock(edge: DockEdge, frame: PhysicalRect, monitor: &MonitorGeometry) -> bool {
    let threshold = i64::from(dip_to_px(UNDOCK_DISTANCE_DIP, monitor.scale_factor));
    let work = monitor.work_area;
    let inward = match edge {
        DockEdge::Left => i64::from(frame.x) - i64::from(work.x),
        DockEdge::Right => work.right() - frame.right(),
        DockEdge::Top => i64::from(frame.y) - i64::from(work.y),
    };
    inward > threshold
}

pub fn expanded_frame(
    placement: &WindowPlacement,
    edge: Option<DockEdge>,
    monitor: &MonitorGeometry,
) -> PhysicalRect {
    let work = monitor.work_area;
    let width = dip_to_px(placement.width_dip, monitor.scale_factor).min(work.width);
    let height = dip_to_px(placement.height_dip, monitor.scale_factor).min(work.height);
    let available_x = work.width.saturating_sub(width);
    let available_y = work.height.saturating_sub(height);
    let floating_x = i64::from(work.x) + ratio_offset(available_x, placement.floating_x_ratio);
    let floating_y = i64::from(work.y) + ratio_offset(available_y, placement.floating_y_ratio);
    let dock_offset_x = i64::from(work.x) + ratio_offset(available_x, placement.dock_offset_ratio);
    let dock_offset_y = i64::from(work.y) + ratio_offset(available_y, placement.dock_offset_ratio);
    let (x, y) = match edge {
        None => (floating_x, floating_y),
        Some(DockEdge::Left) => (i64::from(work.x), dock_offset_y),
        Some(DockEdge::Right) => (work.right() - i64::from(width), dock_offset_y),
        Some(DockEdge::Top) => (dock_offset_x, i64::from(work.y)),
    };
    PhysicalRect::new(saturating_i32(x), saturating_i32(y), width, height)
}

pub fn collapsed_frame(
    expanded: PhysicalRect,
    edge: DockEdge,
    monitor: &MonitorGeometry,
) -> PhysicalRect {
    let work = monitor.work_area;
    let sensor = i64::from(sensor_thickness_px(monitor));
    let (x, y) = match edge {
        DockEdge::Left => (
            i64::from(work.x) - i64::from(expanded.width) + sensor,
            i64::from(expanded.y),
        ),
        DockEdge::Right => (work.right() - sensor, i64::from(expanded.y)),
        DockEdge::Top => (
            i64::from(expanded.x),
            i64::from(work.y) - i64::from(expanded.height) + sensor,
        ),
    };
    PhysicalRect::new(
        saturating_i32(x),
        saturating_i32(y),
        expanded.width,
        expanded.height,
    )
}

pub fn recover_placement<'a>(
    placement: &WindowPlacement,
    edge: Option<DockEdge>,
    monitors: &'a [MonitorGeometry],
) -> Option<(&'a MonitorGeometry, PhysicalRect)> {
    let monitor = placement
        .monitor_identity
        .as_ref()
        .and_then(|identity| {
            monitors
                .iter()
                .find(|monitor| &monitor.identity == identity)
        })
        .or_else(|| monitors.iter().find(|monitor| monitor.primary))
        .or_else(|| monitors.first())?;
    Some((monitor, expanded_frame(placement, edge, monitor)))
}

pub(crate) fn placement_from_frame(
    frame: PhysicalRect,
    monitor: &MonitorGeometry,
    previous: &WindowPlacement,
    dock_edge: Option<DockEdge>,
) -> WindowPlacement {
    let work = monitor.work_area;
    let available_x = work.width.saturating_sub(frame.width);
    let available_y = work.height.saturating_sub(frame.height);
    let x_ratio = offset_ratio(i64::from(frame.x) - i64::from(work.x), available_x);
    let y_ratio = offset_ratio(i64::from(frame.y) - i64::from(work.y), available_y);
    let dock_ratio = match dock_edge {
        Some(DockEdge::Left | DockEdge::Right) => y_ratio,
        Some(DockEdge::Top) => x_ratio,
        None => previous.dock_offset_ratio,
    };
    WindowPlacement::new(
        f64::from(frame.width) / monitor.scale_factor,
        f64::from(frame.height) / monitor.scale_factor,
        Some(monitor.identity.clone()),
        dock_ratio,
        x_ratio,
        y_ratio,
    )
}

fn valid_scale(scale: f64) -> f64 {
    if scale.is_finite() && scale > 0.0 {
        scale
    } else {
        1.0
    }
}

fn finite_non_negative(value: f64) -> f64 {
    if value.is_finite() {
        value.max(0.0)
    } else {
        0.0
    }
}

fn finite_with_minimum(value: f64, minimum: f64) -> f64 {
    if value.is_finite() {
        value.max(minimum)
    } else {
        minimum
    }
}

fn ratio(value: f64) -> f64 {
    if value.is_finite() {
        value.clamp(0.0, 1.0)
    } else {
        0.5
    }
}

fn dip_to_px(dip: f64, scale: f64) -> u32 {
    let pixels = finite_non_negative(dip) * valid_scale(scale);
    pixels.round().clamp(0.0, f64::from(u32::MAX)) as u32
}

fn ratio_offset(available: u32, ratio: f64) -> i64 {
    (f64::from(available) * ratio.clamp(0.0, 1.0)).round() as i64
}

fn offset_ratio(offset: i64, available: u32) -> f64 {
    if available == 0 {
        0.5
    } else {
        (offset as f64 / f64::from(available)).clamp(0.0, 1.0)
    }
}

fn saturating_i32(value: i64) -> i32 {
    value.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32
}

#[cfg(test)]
mod phase8_window_tests {
    use super::*;

    fn monitor(identity: &str, x: i32, scale: f64, primary: bool) -> MonitorGeometry {
        MonitorGeometry::new(
            MonitorIdentity::new(identity),
            PhysicalRect::new(x, -900, 1920, 1080),
            scale,
            primary,
        )
    }

    fn placement(identity: Option<&str>) -> WindowPlacement {
        WindowPlacement::new(
            520.0,
            680.0,
            identity.map(MonitorIdentity::new),
            0.5,
            0.5,
            0.25,
        )
    }

    #[test]
    fn snap_and_undock_thresholds_are_dip_scaled() {
        let monitor = monitor("mixed", -1920, 1.5, true);
        let snap = (SNAP_DISTANCE_DIP * 1.5) as i32;
        let frame = PhysicalRect::new(monitor.work_area.x + snap, -800, 600, 700);
        assert_eq!(snap_edge(frame, &monitor), Some(DockEdge::Left));
        let just_inside = PhysicalRect::new(
            monitor.work_area.x + (UNDOCK_DISTANCE_DIP * 1.5) as i32,
            -800,
            600,
            700,
        );
        assert!(!should_undock(DockEdge::Left, just_inside, &monitor));
        let beyond = PhysicalRect::new(just_inside.x + 1, -800, 600, 700);
        assert!(should_undock(DockEdge::Left, beyond, &monitor));
        let beyond_snap = PhysicalRect::new(monitor.work_area.x + snap + 1, -800, 600, 700);
        assert_eq!(snap_edge(beyond_snap, &monitor), None);
        let near_bottom_only = PhysicalRect::new(
            monitor.work_area.x + 500,
            monitor.work_area.y + monitor.work_area.height as i32 - 5,
            300,
            100,
        );
        assert_eq!(snap_edge(near_bottom_only, &monitor), None);
    }

    #[test]
    fn phase10_snap_uses_nearest_edge_and_only_tie_breaks_within_one_dip() {
        for scale in [1.0, 1.25, 1.5, 2.0] {
            let monitor = MonitorGeometry::new(
                MonitorIdentity::new(format!("scale-{scale}")),
                PhysicalRect::new(-1920, -1080, 1920, 1080),
                scale,
                true,
            );
            let capture = dip_to_px(SNAP_DISTANCE_DIP, scale) as i32;
            let inside = PhysicalRect::new(
                monitor.work_area.x + capture,
                monitor.work_area.y + 400,
                400,
                500,
            );
            assert_eq!(snap_edge(inside, &monitor), Some(DockEdge::Left));
            let outside = PhysicalRect::new(inside.x + 1, inside.y, inside.width, inside.height);
            assert_eq!(snap_edge(outside, &monitor), None);

            let top_distance = dip_to_px(10.0, scale) as i32;
            let left_distance = dip_to_px(11.0, scale) as i32;
            let tied = PhysicalRect::new(
                monitor.work_area.x + left_distance,
                monitor.work_area.y + top_distance,
                400,
                500,
            );
            assert_eq!(snap_edge(tied, &monitor), Some(DockEdge::Top));

            let clearly_nearer_left = PhysicalRect::new(
                monitor.work_area.x + dip_to_px(8.0, scale) as i32,
                monitor.work_area.y + dip_to_px(11.0, scale) as i32,
                400,
                500,
            );
            assert_eq!(
                snap_edge(clearly_nearer_left, &monitor),
                Some(DockEdge::Left)
            );
        }
    }

    #[test]
    fn collapsed_frames_leave_exact_scaled_sensor_strip() {
        let monitor = monitor("primary", -1920, 2.0, true);
        let expanded = expanded_frame(&placement(None), Some(DockEdge::Left), &monitor);
        let collapsed = collapsed_frame(expanded, DockEdge::Left, &monitor);
        assert_eq!(collapsed.right() - i64::from(monitor.work_area.x), 6);
        assert_eq!(collapsed.height, expanded.height);
        let right = collapsed_frame(expanded, DockEdge::Right, &monitor);
        assert_eq!(monitor.work_area.right() - i64::from(right.x), 6);
        assert_eq!(right.height, expanded.height);
        let top = collapsed_frame(expanded, DockEdge::Top, &monitor);
        assert_eq!(top.bottom() - i64::from(monitor.work_area.y), 6);
        assert_eq!(top.width, expanded.width);
    }

    #[test]
    fn missing_monitor_falls_back_to_primary_and_stays_visible() {
        let monitors = [
            monitor("secondary", -1920, 1.5, false),
            monitor("primary", 0, 1.0, true),
        ];
        let (chosen, frame) = recover_placement(
            &placement(Some("missing")),
            Some(DockEdge::Right),
            &monitors,
        )
        .unwrap();
        assert_eq!(chosen.identity.as_str(), "primary");
        assert!(chosen.work_area.contains(frame));
        assert_eq!(frame.right(), chosen.work_area.right());
    }

    #[test]
    fn deterministic_geometry_property_keeps_recovered_frame_inside_work_area() {
        let mut seed = 0x5eed_cafe_u64;
        for index in 0..1_000 {
            seed = seed.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1);
            let x = ((seed >> 16) as i32 % 8_000).saturating_sub(4_000);
            let y = ((seed >> 32) as i32 % 4_000).saturating_sub(2_000);
            let scale = [1.0, 1.25, 1.5, 2.0][index % 4];
            let monitor = MonitorGeometry::new(
                MonitorIdentity::new(format!("m-{index}")),
                PhysicalRect::new(
                    x,
                    y,
                    640 + (index % 5) as u32 * 320,
                    480 + (index % 3) as u32 * 240,
                ),
                scale,
                true,
            );
            let candidate = WindowPlacement::new(
                100.0 + (index % 20) as f64 * 90.0,
                100.0 + (index % 15) as f64 * 80.0,
                None,
                (index % 11) as f64 / 10.0,
                (index % 13) as f64 / 12.0,
                (index % 17) as f64 / 16.0,
            );
            for edge in [
                None,
                Some(DockEdge::Left),
                Some(DockEdge::Right),
                Some(DockEdge::Top),
            ] {
                let frame = expanded_frame(&candidate, edge, &monitor);
                assert!(
                    monitor.work_area.contains(frame),
                    "index={index} edge={edge:?}"
                );
            }
        }
    }

    #[test]
    fn placement_sanitizes_non_finite_ratios_size_and_scale() {
        let placement = WindowPlacement::new(
            f64::NAN,
            f64::NEG_INFINITY,
            None,
            f64::NAN,
            f64::INFINITY,
            -1.0,
        );
        let monitor = MonitorGeometry::new(
            MonitorIdentity::new("invalid-scale"),
            PhysicalRect::new(-500, -500, 800, 600),
            f64::NAN,
            true,
        );
        assert_eq!(placement.width_dip, MIN_WINDOW_WIDTH_DIP);
        assert_eq!(placement.height_dip, MIN_WINDOW_HEIGHT_DIP);
        assert_eq!(placement.dock_offset_ratio, 0.5);
        assert_eq!(placement.floating_x_ratio, 0.5);
        assert_eq!(placement.floating_y_ratio, 0.0);
        assert_eq!(monitor.scale_factor, 1.0);
        assert!(
            monitor
                .work_area
                .contains(expanded_frame(&placement, None, &monitor))
        );
    }
}

//! The byte counters of the frame path.
//!
//! plan_ref: docs/plan/06_markdown_math_rendering.md#frame-presentation

use std::fmt;

/// Bytes written on the frame path, so a migration can be compared with its baseline (plan 10).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FrameLedger {
    pub fill_bytes: u64,
    pub pane_blit_source_bytes: u64,
    pub pane_blit_preview_bytes: u64,
    pub convert_full_bytes: u64,
    pub convert_damage_bytes: u64,
    pub damage_blit_bytes: u64,
    pub full_presents: u64,
    pub damage_presents: u64,
    pub pixmap_allocs: u64,
}

impl fmt::Display for FrameLedger {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "fill={} blit_source={} blit_preview={} convert_full={} convert_damage={} \
             damage_blit={} full_presents={} damage_presents={} pixmap_allocs={}",
            self.fill_bytes,
            self.pane_blit_source_bytes,
            self.pane_blit_preview_bytes,
            self.convert_full_bytes,
            self.convert_damage_bytes,
            self.damage_blit_bytes,
            self.full_presents,
            self.damage_presents,
            self.pixmap_allocs,
        )
    }
}

impl FrameLedger {
    pub(super) fn record_full(&mut self, bytes: usize) {
        self.convert_full_bytes += bytes as u64;
        self.full_presents += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::FrameLedger;

    #[test]
    fn the_ledger_prints_every_counter() {
        let ledger = FrameLedger {
            fill_bytes: 1,
            pane_blit_source_bytes: 2,
            pane_blit_preview_bytes: 3,
            convert_full_bytes: 4,
            convert_damage_bytes: 5,
            damage_blit_bytes: 6,
            full_presents: 7,
            damage_presents: 8,
            pixmap_allocs: 9,
        };
        assert_eq!(
            ledger.to_string(),
            "fill=1 blit_source=2 blit_preview=3 convert_full=4 convert_damage=5 damage_blit=6 \
             full_presents=7 damage_presents=8 pixmap_allocs=9"
        );
    }
}

//! Native adapters only; no scenario or acceptance policy.
//! plan_ref: docs/plan/11_testing_and_release.md#phase-16-macos-host
pub(in crate::mac_gui) mod accessibility;
mod cf;
pub(in crate::mac_gui) mod events;
pub(in crate::mac_gui) mod permissions;
pub(in crate::mac_gui) mod windows;

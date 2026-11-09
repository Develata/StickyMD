//! StickyMD Shared Application.
//!
//! plan_ref: docs/plan/03_system_architecture.md#shared-crates
//!
//! This crate holds the Instruction Interface and Flow Coordination that every Platform Host
//! shares: typed intents, coordinators, the application session that owns them, configuration
//! state and the editor and Search sessions.
//! It must never depend on a window system or a platform API and it contains no `unsafe`; a host
//! supplies native facts and executes the capabilities requested of it. The module layout keeps
//! the plan 03 layers (`instruction`, `flow`); a crate boundary does not replace them.
#![forbid(unsafe_code)]

pub mod config;
pub mod execution;
pub mod flow;
pub mod instruction;
pub mod interaction;
pub mod ports;
pub mod session;
mod source_search;

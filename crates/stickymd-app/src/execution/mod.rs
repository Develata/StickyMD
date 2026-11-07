//! Execution every host shares: worker scheduling and the result protocol. A worker owns its
//! thread and mailbox; a host injects the capabilities it needs and the completion callback that
//! wakes its event loop.
//!
//! plan_ref: docs/plan/03_system_architecture.md#shared-crates

mod preview_worker;

pub use preview_worker::PreviewWorker;

//! StickyMD rendering projection crate.
//!
//! plan_ref: docs/plan/06_markdown_math_rendering.md#owned-ast-投影
//!
//! Phase 1 status: package skeleton only. Future responsibility:
//!
//! - Markdown projection (Comrak Arena -> owned AST conversion happens here)
//! - Owned AST (`preview::owned_ast`)
//! - RenderTree (`preview::render_tree`)
//! - Block/inline layout
//! - Math integration (RaTeX `math::display_list` stays a projection; RaTeX
//!   types must never leak into `stickymd-core`)
//! - Preview selection mapping
//!
//! Constraints carried over from the contracts:
//!
//! - Comrak defines Markdown semantics; RaTeX defines math semantics.
//!   StickyMD owns only projection/layout integration.
//! - The Comrak Arena must never be stored long-term or cross-thread;
//!   it is converted to the owned tree and dropped.
//! - This crate must stay platform-independent (Linux CI builds it).
#![forbid(unsafe_code)]

//! Lua script engine for Mamu.
//!
//! Provides a sandboxed Lua 5.4 runtime that exposes memory manipulation APIs,
//! allowing users to automate search, read/write, freeze, and other operations
//! via Lua scripts.
//!
//! # Architecture
//!
//! ```text
//! ┌──────────────────────────────────────────┐
//! │  engine.rs   — Lua VM lifecycle          │
//! │  runtime.rs  — Execution & cancellation  │
//! │  api/        — mamu.* Lua bindings       │
//! │    memory.rs   — read/write primitives   │
//! │    search.rs   — search/refine/results   │
//! │    freeze.rs   — freeze/unfreeze values  │
//! │    process.rs  — process info queries    │
//! │    utility.rs  — sleep/toast/log/input   │
//! └──────────────────────────────────────────┘
//! ```

pub mod api;
pub mod engine;
pub mod runtime;

// Re-export the primary public interface
pub use engine::ScriptEngine;
pub use runtime::{ScriptRuntime, ScriptStatus};

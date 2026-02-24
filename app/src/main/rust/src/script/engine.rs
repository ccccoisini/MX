//! Core Lua engine lifecycle management.
//!
//! [`ScriptEngine`] owns the Lua VM instance and is responsible for:
//! - Creating a sandboxed Lua 5.4 environment
//! - Registering the `mamu.*` API table
//! - Compiling and executing user scripts
//! - Providing a clean shutdown path

use anyhow::{Context, Result};
use log::{error, info};
use mlua::{Lua, StdLib};
use std::sync::Arc;

use crate::script::api;
use crate::script::runtime::ScriptCallback;

/// Core Lua engine that manages the VM and API registration.
///
/// Each `ScriptEngine` holds one Lua VM. It is created fresh for every
/// script execution so that global state never leaks between runs.
pub struct ScriptEngine {
    lua: Lua,
}

impl ScriptEngine {
    /// Create a new Lua engine with the `mamu` API table registered.
    ///
    /// Only safe standard libraries are loaded (no `io`, `os`, `debug`, `ffi`).
    /// The `mamu` global table is populated with all API functions.
    pub fn new(callback: Arc<dyn ScriptCallback>) -> Result<Self> {
        // Load safe standard libraries — no file I/O, OS, debug, or FFI access.
        // Base functions (tostring, tonumber, type, pairs, pcall, etc.) are
        // always available in mlua regardless of StdLib flags.
        let safe_libs = StdLib::TABLE
            | StdLib::STRING
            | StdLib::MATH
            | StdLib::COROUTINE
            | StdLib::UTF8;

        let lua = Lua::new_with(safe_libs, mlua::LuaOptions::default())
            .context("Failed to create Lua VM")?;

        // Remove dangerous base functions for sandboxing
        let globals = lua.globals();
        globals.set("dofile", mlua::Value::Nil)?;
        globals.set("loadfile", mlua::Value::Nil)?;

        // Set reasonable memory limit (64 MB) to prevent runaway scripts
        lua.set_memory_limit(64 * 1024 * 1024)?;

        // Register the mamu API table
        api::register_all(&lua, callback.clone())
            .context("Failed to register mamu API")?;

        // Register a sandboxed `print` that redirects to the log panel
        let cb = callback.clone();
        let print_fn = lua.create_function(move |lua, args: mlua::MultiValue| {
            let tostring: mlua::Function = lua.globals().get("tostring")?;
            let parts: Vec<String> = args
                .iter()
                .map(|v| {
                    tostring
                        .call::<mlua::String>(v.clone())
                        .and_then(|s| Ok(s.to_str()?.to_owned()))
                        .unwrap_or_else(|_| format!("{:?}", v))
                })
                .collect();
            let message = parts.join("\t");
            cb.on_log(&message);
            Ok(())
        })?;
        lua.globals().set("print", print_fn)?;

        info!("ScriptEngine created successfully");
        Ok(Self { lua })
    }

    /// Execute a Lua script from source code.
    ///
    /// Returns `Ok(())` on successful completion, or an error describing
    /// what went wrong (syntax error, runtime error, etc.).
    pub fn execute(&self, source: &str, script_name: &str) -> Result<()> {
        self.lua
            .load(source)
            .set_name(script_name)
            .exec()
            .map_err(|e| {
                error!("Script '{}' failed: {}", script_name, e);
                anyhow::anyhow!("{}", e)
            })
    }
}

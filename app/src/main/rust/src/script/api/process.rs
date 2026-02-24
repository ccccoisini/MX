//! Process information API bindings for Lua.
//!
//! Exposes `mamu.get_pid`, `mamu.get_process_info`, and
//! `mamu.list_memory_regions` for querying the bound process state.

use anyhow::Result;
use mlua::{Lua, Table};
use std::sync::Arc;

use crate::core::DRIVER_MANAGER;
use crate::script::runtime::ScriptCallback;

/// Register process-related functions into the `mamu` table.
pub fn register(lua: &Lua, mamu: &Table, _callback: Arc<dyn ScriptCallback>) -> Result<()> {
    // -----------------------------------------------------------------------
    // mamu.get_pid() -> integer | nil
    //   Returns the PID of the currently bound process, or nil if none.
    // -----------------------------------------------------------------------
    mamu.set(
        "get_pid",
        lua.create_function(|_, ()| {
            let dm = DRIVER_MANAGER.read().map_err(|e| mlua::Error::RuntimeError(e.to_string()))?;
            if dm.is_process_bound() {
                Ok(Some(dm.get_bound_pid() as i64))
            } else {
                Ok(None)
            }
        })?,
    )?;

    // -----------------------------------------------------------------------
    // mamu.is_process_bound() -> boolean
    //   Returns whether a process is currently bound.
    // -----------------------------------------------------------------------
    mamu.set(
        "is_process_bound",
        lua.create_function(|_, ()| {
            let dm = DRIVER_MANAGER.read().map_err(|e| mlua::Error::RuntimeError(e.to_string()))?;
            Ok(dm.is_process_bound())
        })?,
    )?;

    // -----------------------------------------------------------------------
    // mamu.is_driver_loaded() -> boolean
    //   Returns whether the kernel driver is loaded.
    // -----------------------------------------------------------------------
    mamu.set(
        "is_driver_loaded",
        lua.create_function(|_, ()| {
            let dm = DRIVER_MANAGER.read().map_err(|e| mlua::Error::RuntimeError(e.to_string()))?;
            Ok(dm.is_driver_loaded())
        })?,
    )?;

    Ok(())
}

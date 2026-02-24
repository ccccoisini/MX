//! Lua API bindings for the `mamu` global table.
//!
//! Each sub-module registers its functions into the `mamu` table that is
//! available to user scripts. The modules are:
//!
//! - [`memory`]  — `mamu.read_*` / `mamu.write_*`
//! - [`search`]  — `mamu.search` / `mamu.refine` / `mamu.get_results`
//! - [`freeze`]  — `mamu.freeze` / `mamu.unfreeze`
//! - [`process`] — `mamu.get_pid` / `mamu.get_process_info`
//! - [`utility`] — `mamu.sleep` / `mamu.toast` / `mamu.log`

pub mod freeze;
pub mod memory;
pub mod process;
pub mod regions;
pub mod search;
pub mod utility;

use anyhow::Result;
use mlua::Lua;
use std::sync::Arc;

use crate::script::runtime::ScriptCallback;

/// Register all `mamu.*` API functions into the Lua global table.
///
/// This creates a single `mamu` table and delegates to each sub-module
/// to populate it with their respective functions.
pub fn register_all(lua: &Lua, callback: Arc<dyn ScriptCallback>) -> Result<()> {
    let mamu = lua.create_table()?;

    memory::register(lua, &mamu, callback.clone())?;
    search::register(lua, &mamu, callback.clone())?;
    freeze::register(lua, &mamu, callback.clone())?;
    process::register(lua, &mamu, callback.clone())?;
    utility::register(lua, &mamu, callback)?;

    lua.globals().set("mamu", mamu)?;
    Ok(())
}

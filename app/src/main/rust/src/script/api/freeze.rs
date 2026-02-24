//! Freeze/unfreeze API bindings for Lua.
//!
//! Exposes `mamu.freeze` and `mamu.unfreeze` functions that delegate
//! to the existing Rust `FreezeManager`.

use anyhow::Result;
use mlua::{Lua, Table};
use std::sync::Arc;

use crate::core::globals::FREEZE_MANAGER;
use crate::script::runtime::ScriptCallback;

/// Register freeze-related functions into the `mamu` table.
pub fn register(lua: &Lua, mamu: &Table, callback: Arc<dyn ScriptCallback>) -> Result<()> {
    // -----------------------------------------------------------------------
    // mamu.freeze(address, value, type_string) -> boolean
    //   Freezes a memory address to a specific value.
    //   The value is continuously written at the freeze interval.
    // -----------------------------------------------------------------------
    let cb = callback.clone();
    mamu.set(
        "freeze",
        lua.create_function(move |_, (addr, value, typ): (u64, String, String)| {
            let bytes = parse_value_to_bytes(&value, &typ)?;
            let type_id = type_string_to_id(&typ);

            let fm = FREEZE_MANAGER.read().map_err(|e| mlua::Error::RuntimeError(e.to_string()))?;
            fm.add_frozen(addr, bytes, type_id);
            cb.on_log(&format!("[冻结] 冻结地址 0x{:X} = {}", addr, value));
            Ok(true)
        })?,
    )?;

    // -----------------------------------------------------------------------
    // mamu.unfreeze(address) -> boolean
    //   Removes the freeze on a specific address.
    // -----------------------------------------------------------------------
    let cb = callback.clone();
    mamu.set(
        "unfreeze",
        lua.create_function(move |_, addr: u64| {
            let fm = FREEZE_MANAGER.read().map_err(|e| mlua::Error::RuntimeError(e.to_string()))?;
            let removed = fm.remove_frozen(addr);
            if removed {
                cb.on_log(&format!("[冻结] 解冻地址 0x{:X}", addr));
            }
            Ok(removed)
        })?,
    )?;

    // -----------------------------------------------------------------------
    // mamu.unfreeze_all() -> nil
    //   Removes all active freezes.
    // -----------------------------------------------------------------------
    let cb = callback.clone();
    mamu.set(
        "unfreeze_all",
        lua.create_function(move |_, ()| {
            let fm = FREEZE_MANAGER.read().map_err(|e| mlua::Error::RuntimeError(e.to_string()))?;
            fm.clear_all();
            cb.on_log("[冻结] 已解冻所有地址");
            Ok(())
        })?,
    )?;

    // -----------------------------------------------------------------------
    // mamu.get_freeze_count() -> integer
    //   Returns the number of currently frozen addresses.
    // -----------------------------------------------------------------------
    mamu.set(
        "get_freeze_count",
        lua.create_function(|_, ()| {
            let fm = FREEZE_MANAGER.read().map_err(|e| mlua::Error::RuntimeError(e.to_string()))?;
            Ok(fm.get_frozen_count() as i64)
        })?,
    )?;

    // -----------------------------------------------------------------------
    // mamu.is_frozen(address) -> boolean
    //   Checks whether a specific address is currently frozen.
    // -----------------------------------------------------------------------
    mamu.set(
        "is_frozen",
        lua.create_function(|_, addr: u64| {
            let fm = FREEZE_MANAGER.read().map_err(|e| mlua::Error::RuntimeError(e.to_string()))?;
            Ok(fm.is_frozen(addr))
        })?,
    )?;

    Ok(())
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Map a type string to a numeric type ID for the FreezeManager.
fn type_string_to_id(typ: &str) -> i32 {
    match typ.to_lowercase().as_str() {
        "byte" | "i8" | "u8" => 0,
        "word" | "short" | "i16" | "u16" => 1,
        "dword" | "int" | "i32" | "u32" => 2,
        "qword" | "long" | "i64" | "u64" => 3,
        "float" | "f32" => 4,
        "double" | "f64" => 5,
        _ => -1,
    }
}

/// Parse a string value + type into raw bytes for freezing.
fn parse_value_to_bytes(value: &str, typ: &str) -> mlua::Result<Vec<u8>> {
    match typ.to_lowercase().as_str() {
        "byte" | "i8" | "u8" => {
            let v: i64 = value.parse().map_err(mlua::Error::external)?;
            Ok((v as u8).to_le_bytes().to_vec())
        }
        "word" | "short" | "i16" | "u16" => {
            let v: i64 = value.parse().map_err(mlua::Error::external)?;
            Ok((v as i16).to_le_bytes().to_vec())
        }
        "dword" | "int" | "i32" | "u32" => {
            let v: i64 = value.parse().map_err(mlua::Error::external)?;
            Ok((v as i32).to_le_bytes().to_vec())
        }
        "qword" | "long" | "i64" | "u64" => {
            let v: i64 = value.parse().map_err(mlua::Error::external)?;
            Ok(v.to_le_bytes().to_vec())
        }
        "float" | "f32" => {
            let v: f32 = value.parse().map_err(mlua::Error::external)?;
            Ok(v.to_le_bytes().to_vec())
        }
        "double" | "f64" => {
            let v: f64 = value.parse().map_err(mlua::Error::external)?;
            Ok(v.to_le_bytes().to_vec())
        }
        other => Err(mlua::Error::external(anyhow::anyhow!(
            "freeze: 未知的值类型 '{}'",
            other
        ))),
    }
}

//! Memory read/write API bindings for Lua.
//!
//! Exposes `mamu.read_*` and `mamu.write_*` functions that operate on
//! the currently bound process through the kernel driver.

use anyhow::Result;
use log::error;
use mlua::{Lua, Table};
use std::sync::Arc;

use crate::core::DRIVER_MANAGER;
use crate::script::runtime::ScriptCallback;

/// Register memory read/write functions into the `mamu` table.
pub fn register(lua: &Lua, mamu: &Table, _callback: Arc<dyn ScriptCallback>) -> Result<()> {
    // -----------------------------------------------------------------------
    // mamu.read_byte(address) -> integer | nil
    // -----------------------------------------------------------------------
    mamu.set(
        "read_byte",
        lua.create_function(|_, addr: u64| {
            read_value::<u8>(addr).map(|v| v.map(|n| n as i64))
        })?,
    )?;

    // -----------------------------------------------------------------------
    // mamu.read_short(address) -> integer | nil
    // -----------------------------------------------------------------------
    mamu.set(
        "read_short",
        lua.create_function(|_, addr: u64| {
            read_value::<i16>(addr).map(|v| v.map(|n| n as i64))
        })?,
    )?;

    // -----------------------------------------------------------------------
    // mamu.read_ushort(address) -> integer | nil
    // -----------------------------------------------------------------------
    mamu.set(
        "read_ushort",
        lua.create_function(|_, addr: u64| {
            read_value::<u16>(addr).map(|v| v.map(|n| n as i64))
        })?,
    )?;

    // -----------------------------------------------------------------------
    // mamu.read_int(address) -> integer | nil
    // -----------------------------------------------------------------------
    mamu.set(
        "read_int",
        lua.create_function(|_, addr: u64| {
            read_value::<i32>(addr).map(|v| v.map(|n| n as i64))
        })?,
    )?;

    // -----------------------------------------------------------------------
    // mamu.read_uint(address) -> integer | nil
    // -----------------------------------------------------------------------
    mamu.set(
        "read_uint",
        lua.create_function(|_, addr: u64| {
            read_value::<u32>(addr).map(|v| v.map(|n| n as i64))
        })?,
    )?;

    // -----------------------------------------------------------------------
    // mamu.read_long(address) -> integer | nil
    // -----------------------------------------------------------------------
    mamu.set(
        "read_long",
        lua.create_function(|_, addr: u64| {
            read_value::<i64>(addr)
        })?,
    )?;

    // -----------------------------------------------------------------------
    // mamu.read_float(address) -> number | nil
    // -----------------------------------------------------------------------
    mamu.set(
        "read_float",
        lua.create_function(|_, addr: u64| {
            read_value::<f32>(addr).map(|v| v.map(|n| n as f64))
        })?,
    )?;

    // -----------------------------------------------------------------------
    // mamu.read_double(address) -> number | nil
    // -----------------------------------------------------------------------
    mamu.set(
        "read_double",
        lua.create_function(|_, addr: u64| {
            read_value::<f64>(addr)
        })?,
    )?;

    // -----------------------------------------------------------------------
    // mamu.read_bytes(address, size) -> string | nil
    //   Returns raw bytes as a Lua string (binary safe).
    // -----------------------------------------------------------------------
    mamu.set(
        "read_bytes",
        lua.create_function(|lua, (addr, size): (u64, usize)| {
            let mut buf = vec![0u8; size];
            let dm = DRIVER_MANAGER.read().map_err(|e| mlua::Error::RuntimeError(e.to_string()))?;
            match dm.read_memory_unified(addr, &mut buf, None) {
                Ok(()) => Ok(Some(lua.create_string(&buf)?)),
                Err(e) => {
                    error!("read_bytes(0x{:X}, {}) failed: {}", addr, size, e);
                    Ok(None)
                }
            }
        })?,
    )?;

    // -----------------------------------------------------------------------
    // mamu.write_byte(address, value) -> boolean
    // -----------------------------------------------------------------------
    mamu.set(
        "write_byte",
        lua.create_function(|_, (addr, val): (u64, i64)| {
            write_value::<u8>(addr, val as u8)
        })?,
    )?;

    // -----------------------------------------------------------------------
    // mamu.write_short(address, value) -> boolean
    // -----------------------------------------------------------------------
    mamu.set(
        "write_short",
        lua.create_function(|_, (addr, val): (u64, i64)| {
            write_value::<i16>(addr, val as i16)
        })?,
    )?;

    // -----------------------------------------------------------------------
    // mamu.write_int(address, value) -> boolean
    // -----------------------------------------------------------------------
    mamu.set(
        "write_int",
        lua.create_function(|_, (addr, val): (u64, i64)| {
            write_value::<i32>(addr, val as i32)
        })?,
    )?;

    // -----------------------------------------------------------------------
    // mamu.write_long(address, value) -> boolean
    // -----------------------------------------------------------------------
    mamu.set(
        "write_long",
        lua.create_function(|_, (addr, val): (u64, i64)| {
            write_value::<i64>(addr, val)
        })?,
    )?;

    // -----------------------------------------------------------------------
    // mamu.write_float(address, value) -> boolean
    // -----------------------------------------------------------------------
    mamu.set(
        "write_float",
        lua.create_function(|_, (addr, val): (u64, f64)| {
            write_value::<f32>(addr, val as f32)
        })?,
    )?;

    // -----------------------------------------------------------------------
    // mamu.write_double(address, value) -> boolean
    // -----------------------------------------------------------------------
    mamu.set(
        "write_double",
        lua.create_function(|_, (addr, val): (u64, f64)| {
            write_value::<f64>(addr, val)
        })?,
    )?;

    // -----------------------------------------------------------------------
    // mamu.write_bytes(address, data_string) -> boolean
    //   Writes raw bytes from a Lua string.
    // -----------------------------------------------------------------------
    mamu.set(
        "write_bytes",
        lua.create_function(|_, (addr, data): (u64, mlua::String)| {
            let bytes = data.as_bytes();
            let dm = DRIVER_MANAGER.read().map_err(|e| mlua::Error::RuntimeError(e.to_string()))?;
            match dm.write_memory_unified(addr, &bytes[..]) {
                Ok(()) => Ok(true),
                Err(e) => {
                    error!("write_bytes(0x{:X}) failed: {}", addr, e);
                    Ok(false)
                }
            }
        })?,
    )?;

    Ok(())
}

// ---------------------------------------------------------------------------
// Generic helpers
// ---------------------------------------------------------------------------

/// Read a typed value from process memory via `read_memory_unified`.
fn read_value<T: Copy>(addr: u64) -> mlua::Result<Option<T>> {
    let size = std::mem::size_of::<T>();
    let mut buf = vec![0u8; size];
    let dm = DRIVER_MANAGER.read().map_err(|e| mlua::Error::RuntimeError(e.to_string()))?;

    match dm.read_memory_unified(addr, &mut buf, None) {
        Ok(()) => {
            let value = unsafe { std::ptr::read_unaligned(buf.as_ptr() as *const T) };
            Ok(Some(value))
        }
        Err(e) => {
            error!("read_value<{}>(0x{:X}) failed: {}", std::any::type_name::<T>(), addr, e);
            Ok(None)
        }
    }
}

/// Write a typed value to process memory via `write_memory_unified`.
fn write_value<T: Copy>(addr: u64, value: T) -> mlua::Result<bool> {
    let size = std::mem::size_of::<T>();
    let bytes = unsafe {
        std::slice::from_raw_parts(&value as *const T as *const u8, size)
    };

    let dm = DRIVER_MANAGER.read().map_err(|e| mlua::Error::RuntimeError(e.to_string()))?;
    match dm.write_memory_unified(addr, bytes) {
        Ok(()) => Ok(true),
        Err(e) => {
            error!("write_value<{}>(0x{:X}) failed: {}", std::any::type_name::<T>(), addr, e);
            Ok(false)
        }
    }
}

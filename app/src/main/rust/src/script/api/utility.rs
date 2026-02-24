//! Utility API bindings for Lua.
//!
//! Exposes helper functions like `mamu.sleep`, `mamu.toast`, `mamu.log`,
//! `mamu.hex`, and other convenience utilities.

use anyhow::Result;
use mlua::{Lua, Table};
use std::sync::Arc;

use crate::script::runtime::{ScriptCallback, CANCEL_FLAG};

/// Register utility functions into the `mamu` table.
pub fn register(lua: &Lua, mamu: &Table, callback: Arc<dyn ScriptCallback>) -> Result<()> {
    // -----------------------------------------------------------------------
    // mamu.sleep(milliseconds)
    //   Pauses script execution. Checks the cancel flag every 50 ms so
    //   cancellation remains responsive even during long sleeps.
    // -----------------------------------------------------------------------
    mamu.set(
        "sleep",
        lua.create_function(|_, ms: u64| {
            let start = std::time::Instant::now();
            let duration = std::time::Duration::from_millis(ms);
            let check_interval = std::time::Duration::from_millis(50);

            while start.elapsed() < duration {
                if CANCEL_FLAG.is_cancelled() {
                    return Err(mlua::Error::external(anyhow::anyhow!("脚本已取消")));
                }
                let remaining = duration.saturating_sub(start.elapsed());
                std::thread::sleep(remaining.min(check_interval));
            }
            Ok(())
        })?,
    )?;

    // -----------------------------------------------------------------------
    // mamu.toast(message)
    //   Shows a short toast notification on the device screen.
    // -----------------------------------------------------------------------
    let cb = callback.clone();
    mamu.set(
        "toast",
        lua.create_function(move |_, msg: String| {
            cb.on_toast(&msg);
            Ok(())
        })?,
    )?;

    // -----------------------------------------------------------------------
    // mamu.log(message)
    //   Appends a message to the script output log panel.
    // -----------------------------------------------------------------------
    let cb = callback.clone();
    mamu.set(
        "log",
        lua.create_function(move |_, msg: String| {
            cb.on_log(&msg);
            Ok(())
        })?,
    )?;

    // -----------------------------------------------------------------------
    // mamu.hex(number) -> string
    //   Formats an integer as a hexadecimal string (e.g. "0x1A2B").
    // -----------------------------------------------------------------------
    mamu.set(
        "hex",
        lua.create_function(|_, n: i64| {
            Ok(format!("0x{:X}", n))
        })?,
    )?;

    // -----------------------------------------------------------------------
    // mamu.parse_hex(hex_string) -> integer
    //   Parses a hexadecimal string (with or without "0x" prefix) to integer.
    // -----------------------------------------------------------------------
    mamu.set(
        "parse_hex",
        lua.create_function(|_, s: String| {
            let s = s.trim().trim_start_matches("0x").trim_start_matches("0X");
            let value = u64::from_str_radix(s, 16)
                .map_err(|e| mlua::Error::external(anyhow::anyhow!("无效的十六进制: {}", e)))?;
            Ok(value as i64)
        })?,
    )?;

    // -----------------------------------------------------------------------
    // mamu.is_cancelled() -> boolean
    //   Returns true if the user has requested script cancellation.
    //   Useful for checking inside long loops.
    // -----------------------------------------------------------------------
    mamu.set(
        "is_cancelled",
        lua.create_function(|_, ()| {
            Ok(CANCEL_FLAG.is_cancelled())
        })?,
    )?;

    // -----------------------------------------------------------------------
    // mamu.check_cancel()
    //   Throws an error if cancellation was requested.
    //   Insert this in loops to make scripts responsive to Stop button.
    // -----------------------------------------------------------------------
    mamu.set(
        "check_cancel",
        lua.create_function(|_, ()| {
            if CANCEL_FLAG.is_cancelled() {
                Err(mlua::Error::external(anyhow::anyhow!("脚本已取消")))
            } else {
                Ok(())
            }
        })?,
    )?;

    // -----------------------------------------------------------------------
    // mamu.time() -> number (seconds since epoch as float)
    //   Returns the current time for profiling or timing operations.
    // -----------------------------------------------------------------------
    mamu.set(
        "time",
        lua.create_function(|_, ()| {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default();
            Ok(now.as_secs_f64())
        })?,
    )?;

    // -----------------------------------------------------------------------
    // mamu.bytes_to_int(byte_string, offset?, type?) -> integer
    //   Reads a typed integer from a byte string at the given offset.
    //   type: "byte","short","int"(default),"long"
    // -----------------------------------------------------------------------
    mamu.set(
        "bytes_to_int",
        lua.create_function(|_, (data, offset, typ): (mlua::String, Option<usize>, Option<String>)| {
            let bytes = data.as_bytes();
            let offset = offset.unwrap_or(0);
            let typ = typ.unwrap_or_else(|| "int".to_string());

            match typ.to_lowercase().as_str() {
                "byte" | "u8" => {
                    if offset >= bytes.len() {
                        return Ok(None);
                    }
                    Ok(Some(bytes[offset] as i64))
                }
                "short" | "word" | "i16" => {
                    if offset + 2 > bytes.len() {
                        return Ok(None);
                    }
                    let v = i16::from_le_bytes([bytes[offset], bytes[offset + 1]]);
                    Ok(Some(v as i64))
                }
                "int" | "dword" | "i32" => {
                    if offset + 4 > bytes.len() {
                        return Ok(None);
                    }
                    let v = i32::from_le_bytes([
                        bytes[offset], bytes[offset + 1],
                        bytes[offset + 2], bytes[offset + 3],
                    ]);
                    Ok(Some(v as i64))
                }
                "long" | "qword" | "i64" => {
                    if offset + 8 > bytes.len() {
                        return Ok(None);
                    }
                    let v = i64::from_le_bytes([
                        bytes[offset], bytes[offset + 1],
                        bytes[offset + 2], bytes[offset + 3],
                        bytes[offset + 4], bytes[offset + 5],
                        bytes[offset + 6], bytes[offset + 7],
                    ]);
                    Ok(Some(v))
                }
                _ => Err(mlua::Error::external(anyhow::anyhow!("未知类型: {}", typ))),
            }
        })?,
    )?;

    // -----------------------------------------------------------------------
    // mamu.bytes_to_float(byte_string, offset?, type?) -> number
    //   Reads a float/double from a byte string at the given offset.
    //   type: "float"(default), "double"
    // -----------------------------------------------------------------------
    mamu.set(
        "bytes_to_float",
        lua.create_function(|_, (data, offset, typ): (mlua::String, Option<usize>, Option<String>)| {
            let bytes = data.as_bytes();
            let offset = offset.unwrap_or(0);
            let typ = typ.unwrap_or_else(|| "float".to_string());

            match typ.to_lowercase().as_str() {
                "float" | "f32" => {
                    if offset + 4 > bytes.len() {
                        return Ok(None);
                    }
                    let v = f32::from_le_bytes([
                        bytes[offset], bytes[offset + 1],
                        bytes[offset + 2], bytes[offset + 3],
                    ]);
                    Ok(Some(v as f64))
                }
                "double" | "f64" => {
                    if offset + 8 > bytes.len() {
                        return Ok(None);
                    }
                    let v = f64::from_le_bytes([
                        bytes[offset], bytes[offset + 1],
                        bytes[offset + 2], bytes[offset + 3],
                        bytes[offset + 4], bytes[offset + 5],
                        bytes[offset + 6], bytes[offset + 7],
                    ]);
                    Ok(Some(v))
                }
                _ => Err(mlua::Error::external(anyhow::anyhow!("未知类型: {}", typ))),
            }
        })?,
    )?;

    Ok(())
}

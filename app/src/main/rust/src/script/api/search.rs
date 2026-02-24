//! Search API bindings for Lua scripts.
//!
//! Exposes the full search engine functionality to Lua, matching the UI capabilities:
//! - New search, refine, fuzzy search, fuzzy refine, pattern search
//! - All value types: byte, word, dword, qword, float, double, auto, xor
//! - All memory ranges: Jh, Ch, Ca, Cd, Cb, Ps, An, J, S, As, V, O, etc.
//! - Result querying, batch writing, and clearing

use anyhow::Result;
use mlua::{Lua, Table};
use std::sync::Arc;

use crate::core::DRIVER_MANAGER;
use crate::core::memory_mode::MemoryAccessMode;
use crate::script::runtime::ScriptCallback;
use crate::search::engine::SEARCH_ENGINE_MANAGER;
use crate::search::result_manager::SearchResultItem;
use crate::search::types::ValueType;

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

fn poll_search_completion() -> mlua::Result<usize> {
    loop {
        std::thread::sleep(std::time::Duration::from_millis(50));
        let manager = SEARCH_ENGINE_MANAGER.read()
            .map_err(|e| mlua::Error::RuntimeError(e.to_string()))?;
        if !manager.is_searching() {
            break;
        }
    }
    let manager = SEARCH_ENGINE_MANAGER.read()
        .map_err(|e| mlua::Error::RuntimeError(e.to_string()))?;
    let count = manager.get_total_count().map_err(mlua::Error::external)?;
    Ok(count)
}

fn switch_access_mode_for_search() -> mlua::Result<(MemoryAccessMode, bool)> {
    let original_mode = {
        let dm = DRIVER_MANAGER.read()
            .map_err(|e| mlua::Error::RuntimeError(e.to_string()))?;
        dm.get_access_mode()
    };
    let switched = if original_mode == MemoryAccessMode::None {
        let mut dm = DRIVER_MANAGER.write()
            .map_err(|e| mlua::Error::RuntimeError(e.to_string()))?;
        dm.set_access_mode(MemoryAccessMode::PageFault).is_ok()
    } else {
        false
    };
    Ok((original_mode, switched))
}

fn restore_access_mode(original_mode: MemoryAccessMode, was_switched: bool) {
    if was_switched {
        if let Ok(mut dm) = DRIVER_MANAGER.write() {
            let _ = dm.set_access_mode(original_mode);
        }
    }
}

fn prepare_regions(ranges_str: &str) -> mlua::Result<Vec<(u64, u64)>> {
    use crate::script::api::regions::{MemRange, get_filtered_regions};

    let requested_ranges: Vec<MemRange> = ranges_str
        .split(',')
        .filter_map(|s| {
            let s = s.trim();
            if s.is_empty() { return None; }
            MemRange::from_code(s)
        })
        .collect();

    if requested_ranges.is_empty() {
        return Err(mlua::Error::external(anyhow::anyhow!(
            "无效的内存范围. 支持: Jh,Ch,Ca,Cd,Cb,Ps,An,J,S,As,V,O,B,Xa,Xs,Dx,Jc,Oa,Vx,Ts,Xx"
        )));
    }

    get_filtered_regions(&requested_ranges).map_err(mlua::Error::external)
}

fn value_type_to_string(vt: ValueType) -> &'static str {
    match vt {
        ValueType::Byte => "byte",
        ValueType::Word => "word",
        ValueType::Dword => "dword",
        ValueType::Qword => "qword",
        ValueType::Float => "float",
        ValueType::Double => "double",
        ValueType::Auto => "auto",
        ValueType::Xor => "xor",
        ValueType::Pattern => "pattern",
    }
}

fn string_to_value_type(typ: &str) -> Option<ValueType> {
    match typ.to_lowercase().as_str() {
        "byte" | "i8" | "u8" => Some(ValueType::Byte),
        "word" | "short" | "i16" | "u16" => Some(ValueType::Word),
        "dword" | "int" | "i32" | "u32" | "d" => Some(ValueType::Dword),
        "qword" | "long" | "i64" | "u64" => Some(ValueType::Qword),
        "float" | "f32" | "f" => Some(ValueType::Float),
        "double" | "f64" => Some(ValueType::Double),
        "auto" => Some(ValueType::Auto),
        "xor" => Some(ValueType::Xor),
        _ => None,
    }
}

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
            "未知的值类型: '{}'. 支持: byte, word, dword, qword, float, double", other
        ))),
    }
}

// ---------------------------------------------------------------------------
// Public API registration
// ---------------------------------------------------------------------------

pub fn register(lua: &Lua, mamu: &Table, callback: Arc<dyn ScriptCallback>) -> Result<()> {
    // mamu.get_result_count() -> integer
    mamu.set(
        "get_result_count",
        lua.create_function(|_, ()| {
            let manager = SEARCH_ENGINE_MANAGER.read()
                .map_err(|e| mlua::Error::RuntimeError(e.to_string()))?;
            let count = manager.get_total_count().map_err(mlua::Error::external)?;
            Ok(count as i64)
        })?,
    )?;

    // mamu.get_results(offset?, count?) -> table of {address, type}
    mamu.set(
        "get_results",
        lua.create_function(|lua, (offset, count): (Option<i64>, Option<i64>)| {
            let offset = offset.unwrap_or(0).max(0) as usize;
            let count = count.unwrap_or(100).max(1).min(10000) as usize;

            let manager = SEARCH_ENGINE_MANAGER.read()
                .map_err(|e| mlua::Error::RuntimeError(e.to_string()))?;
            let results = manager.get_results(offset, count)
                .map_err(mlua::Error::external)?;

            let table = lua.create_table()?;
            for (i, item) in results.iter().enumerate() {
                let entry = lua.create_table()?;
                match item {
                    SearchResultItem::Exact(e) => {
                        entry.set("address", e.address)?;
                        entry.set("type", value_type_to_string(e.typ))?;
                    }
                    SearchResultItem::Fuzzy(f) => {
                        entry.set("address", f.address)?;
                        entry.set("type", value_type_to_string(f.value_type))?;
                    }
                }
                table.set(i + 1, entry)?;
            }
            Ok(table)
        })?,
    )?;

    // mamu.get_result_addresses(offset?, count?) -> table of integers
    mamu.set(
        "get_result_addresses",
        lua.create_function(|lua, (offset, count): (Option<i64>, Option<i64>)| {
            let offset = offset.unwrap_or(0).max(0) as usize;
            let count = count.unwrap_or(100).max(1).min(10000) as usize;

            let manager = SEARCH_ENGINE_MANAGER.read()
                .map_err(|e| mlua::Error::RuntimeError(e.to_string()))?;
            let results = manager.get_results(offset, count)
                .map_err(mlua::Error::external)?;

            let table = lua.create_table()?;
            for (i, item) in results.iter().enumerate() {
                let addr = match item {
                    SearchResultItem::Exact(e) => e.address,
                    SearchResultItem::Fuzzy(f) => f.address,
                };
                table.set(i + 1, addr)?;
            }
            Ok(table)
        })?,
    )?;

    // mamu.clear_results() -> nil
    let cb = callback.clone();
    mamu.set(
        "clear_results",
        lua.create_function(move |_, ()| {
            let mut manager = SEARCH_ENGINE_MANAGER.write()
                .map_err(|e| mlua::Error::RuntimeError(e.to_string()))?;
            manager.clear_results().map_err(mlua::Error::external)?;
            cb.on_search_completed(0);
            Ok(())
        })?,
    )?;

    // mamu.is_searching() -> boolean
    mamu.set(
        "is_searching",
        lua.create_function(|_, ()| {
            let manager = SEARCH_ENGINE_MANAGER.read()
                .map_err(|e| mlua::Error::RuntimeError(e.to_string()))?;
            Ok(manager.is_searching())
        })?,
    )?;

    // mamu.write_results(value, type_string) -> integer (success count)
    let cb = callback.clone();
    mamu.set(
        "write_results",
        lua.create_function(move |_, (value, typ): (String, String)| {
            let bytes = parse_value_to_bytes(&value, &typ)?;

            let manager = SEARCH_ENGINE_MANAGER.read()
                .map_err(|e| mlua::Error::RuntimeError(e.to_string()))?;
            let total = manager.get_total_count().unwrap_or(0);
            if total == 0 {
                return Ok(0i64);
            }

            let dm = DRIVER_MANAGER.read()
                .map_err(|e| mlua::Error::RuntimeError(e.to_string()))?;
            let mut success_count = 0i64;
            let batch_size = 500;
            let mut offset = 0;

            while offset < total {
                let results = manager.get_results(offset, batch_size)
                    .map_err(mlua::Error::external)?;
                if results.is_empty() { break; }

                for item in &results {
                    let addr = match item {
                        SearchResultItem::Exact(e) => e.address,
                        SearchResultItem::Fuzzy(f) => f.address,
                    };
                    if dm.write_memory_unified(addr, &bytes).is_ok() {
                        success_count += 1;
                    }
                }
                offset += results.len();
            }

            cb.on_log(&format!("[搜索] 批量写入: {}/{} 成功", success_count, total));
            Ok(success_count)
        })?,
    )?;

    // mamu.search(value, type, ranges) -> integer
    let cb = callback.clone();
    mamu.set(
        "search",
        lua.create_function(move |_, (value, typ, ranges_str): (String, String, String)| {
            use crate::search::parser::parse_search_query;

            let value_type = string_to_value_type(&typ).ok_or_else(|| {
                mlua::Error::external(anyhow::anyhow!(
                    "未知的值类型: '{}'. 支持: byte, word, dword, qword, float, double, auto, xor", typ
                ))
            })?;

            let regions = prepare_regions(&ranges_str)?;
            if regions.is_empty() {
                cb.on_toast("没有匹配的内存区域");
                return Ok(0i64);
            }

            let total_mb: u64 = regions.iter().map(|(s, e)| e - s).sum::<u64>() / 1024 / 1024;
            log::info!("[script] search: value='{}', type={:?}, regions={}, {}MB", value, value_type, regions.len(), total_mb);
            cb.on_toast(&format!("搜索 {} 个区域 ({}MB)...", regions.len(), total_mb));

            let query = parse_search_query(&value, value_type)
                .map_err(|e| mlua::Error::external(anyhow::anyhow!("搜索解析错误: {}", e)))?;

            let (orig_mode, switched) = switch_access_mode_for_search()?;

            {
                let mut manager = SEARCH_ENGINE_MANAGER.write()
                    .map_err(|e| mlua::Error::RuntimeError(e.to_string()))?;
                manager.start_search_async(query, regions, false, false)
                    .map_err(|e| mlua::Error::external(e))?;
            }

            let count = poll_search_completion()?;
            restore_access_mode(orig_mode, switched);
            log::info!("[script] search completed: {} results", count);
            cb.on_search_completed(count as i64);
            Ok(count as i64)
        })?,
    )?;

    // mamu.refine(value, type) -> integer
    let cb = callback.clone();
    mamu.set(
        "refine",
        lua.create_function(move |_, (value, typ): (String, String)| {
            use crate::search::parser::parse_search_query;

            let value_type = string_to_value_type(&typ).ok_or_else(|| {
                mlua::Error::external(anyhow::anyhow!(
                    "未知的值类型: '{}'. 支持: byte, word, dword, qword, float, double, auto, xor", typ
                ))
            })?;

            let query = parse_search_query(&value, value_type)
                .map_err(|e| mlua::Error::external(anyhow::anyhow!("改善搜索解析错误: {}", e)))?;

            let (orig_mode, switched) = switch_access_mode_for_search()?;

            {
                let mut manager = SEARCH_ENGINE_MANAGER.write()
                    .map_err(|e| mlua::Error::RuntimeError(e.to_string()))?;
                manager.start_refine_async(query)
                    .map_err(|e| mlua::Error::external(e))?;
            }

            let count = poll_search_completion()?;
            restore_access_mode(orig_mode, switched);
            log::info!("[script] refine completed: {} results", count);
            cb.on_search_completed(count as i64);
            Ok(count as i64)
        })?,
    )?;

    // mamu.fuzzy_search(type, ranges) -> integer
    let cb = callback.clone();
    mamu.set(
        "fuzzy_search",
        lua.create_function(move |_, (typ, ranges_str): (String, String)| {
            let value_type = string_to_value_type(&typ).ok_or_else(|| {
                mlua::Error::external(anyhow::anyhow!(
                    "未知的值类型: '{}'. 支持: byte, word, dword, qword, float, double", typ
                ))
            })?;

            let regions = prepare_regions(&ranges_str)?;
            if regions.is_empty() {
                cb.on_toast("没有匹配的内存区域");
                return Ok(0i64);
            }

            let total_mb: u64 = regions.iter().map(|(s, e)| e - s).sum::<u64>() / 1024 / 1024;
            log::info!("[script] fuzzy_search: type={:?}, regions={}, {}MB", value_type, regions.len(), total_mb);
            cb.on_toast(&format!("未知值搜索 {} 个区域 ({}MB)...", regions.len(), total_mb));

            let (orig_mode, switched) = switch_access_mode_for_search()?;

            {
                let mut manager = SEARCH_ENGINE_MANAGER.write()
                    .map_err(|e| mlua::Error::RuntimeError(e.to_string()))?;
                manager.start_fuzzy_search_async(value_type, regions, false)
                    .map_err(|e| mlua::Error::external(e))?;
            }

            let count = poll_search_completion()?;
            restore_access_mode(orig_mode, switched);
            log::info!("[script] fuzzy_search completed: {} results", count);
            cb.on_search_completed(count as i64);
            Ok(count as i64)
        })?,
    )?;

    // mamu.fuzzy_refine(condition, param1?, param2?) -> integer
    let cb = callback.clone();
    mamu.set(
        "fuzzy_refine",
        lua.create_function(move |_, (condition, param1, param2): (String, Option<i64>, Option<i64>)| {
            use crate::search::types::FuzzyCondition;

            let p1 = param1.unwrap_or(0);
            let p2 = param2.unwrap_or(0);

            let cond = match condition.to_lowercase().as_str() {
                "unchanged" => FuzzyCondition::Unchanged,
                "changed" => FuzzyCondition::Changed,
                "increased" => FuzzyCondition::Increased,
                "decreased" => FuzzyCondition::Decreased,
                "increased_by" => FuzzyCondition::IncreasedBy(p1),
                "decreased_by" => FuzzyCondition::DecreasedBy(p1),
                "increased_range" => FuzzyCondition::IncreasedByRange(p1, p2),
                "decreased_range" => FuzzyCondition::DecreasedByRange(p1, p2),
                "increased_percent" => FuzzyCondition::IncreasedByPercent(p1 as f32),
                "decreased_percent" => FuzzyCondition::DecreasedByPercent(p1 as f32),
                other => {
                    return Err(mlua::Error::external(anyhow::anyhow!(
                        "未知的模糊条件: '{}'. 支持: unchanged, changed, increased, decreased, \
                         increased_by, decreased_by, increased_range, decreased_range, \
                         increased_percent, decreased_percent", other
                    )));
                }
            };

            let (orig_mode, switched) = switch_access_mode_for_search()?;

            {
                let mut manager = SEARCH_ENGINE_MANAGER.write()
                    .map_err(|e| mlua::Error::RuntimeError(e.to_string()))?;
                manager.start_fuzzy_refine_async(cond)
                    .map_err(|e| mlua::Error::external(e))?;
            }

            let count = poll_search_completion()?;
            restore_access_mode(orig_mode, switched);
            log::info!("[script] fuzzy_refine '{}': {} results", condition, count);
            cb.on_search_completed(count as i64);
            Ok(count as i64)
        })?,
    )?;

    // mamu.pattern_search(pattern, ranges) -> integer
    let cb = callback.clone();
    mamu.set(
        "pattern_search",
        lua.create_function(move |_, (pattern_str, ranges_str): (String, String)| {
            use crate::search::parse_pattern;

            let pattern = parse_pattern(&pattern_str)
                .map_err(|e| mlua::Error::external(anyhow::anyhow!("特征码解析错误: {}", e)))?;

            let regions = prepare_regions(&ranges_str)?;
            if regions.is_empty() {
                cb.on_toast("没有匹配的内存区域");
                return Ok(0i64);
            }

            let total_mb: u64 = regions.iter().map(|(s, e)| e - s).sum::<u64>() / 1024 / 1024;
            log::info!("[script] pattern_search: '{}', regions={}, {}MB", pattern_str, regions.len(), total_mb);
            cb.on_toast(&format!("特征码搜索 {} 个区域 ({}MB)...", regions.len(), total_mb));

            let (orig_mode, switched) = switch_access_mode_for_search()?;

            {
                let mut manager = SEARCH_ENGINE_MANAGER.write()
                    .map_err(|e| mlua::Error::RuntimeError(e.to_string()))?;
                manager.start_pattern_search_async(pattern, regions)
                    .map_err(|e| mlua::Error::external(e))?;
            }

            let count = poll_search_completion()?;
            restore_access_mode(orig_mode, switched);
            log::info!("[script] pattern_search completed: {} results", count);
            cb.on_search_completed(count as i64);
            Ok(count as i64)
        })?,
    )?;

    Ok(())
}

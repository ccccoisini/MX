//! Memory region query and classification helper for Lua scripts.
//!
//! Ports the Kotlin `DevideMemRange.kt` classification logic to Rust
//! so that Lua scripts can query filtered memory regions directly.

use crate::core::DRIVER_MANAGER;
use crate::wuwa::{MEM_EXECUTABLE, MEM_READABLE, MEM_SHARED, MEM_WRITABLE, WuwaMemRegionEntry};
use anyhow::{anyhow, Result};
use log::debug;
use nix::libc::close;
use nix::sys::mman::{mmap, munmap, MapFlags, ProtFlags};
use std::ffi::CStr;
use std::num::NonZeroUsize;
use std::os::fd::BorrowedFd;

/// Simplified memory range classification matching the Kotlin MemoryRange enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemRange {
    Jh, // Java Heap
    Ch, // C++ heap
    Ca, // C++ alloc
    Cd, // C++ .data
    Cb, // C++ .bss
    Ps, // PPSSPP
    An, // Anonymous
    J,  // Java
    S,  // Stack
    As, // Ashmem
    V,  // Video
    O,  // Other
    B,  // Bad
    Xa, // Code app
    Xs, // Code system
    Dx, // DEX
    Jc, // JIT cache code
    Oa, // OAT Code
    Vx, // VDEX
    Ts, // Thread stack
    Xx, // No perm
}

impl MemRange {
    /// Parse a range code string (case-insensitive) into a MemRange.
    pub fn from_code(code: &str) -> Option<Self> {
        match code.to_lowercase().as_str() {
            "jh" => Some(Self::Jh),
            "ch" => Some(Self::Ch),
            "ca" => Some(Self::Ca),
            "cd" => Some(Self::Cd),
            "cb" => Some(Self::Cb),
            "ps" => Some(Self::Ps),
            "an" | "a" => Some(Self::An),
            "j" => Some(Self::J),
            "s" => Some(Self::S),
            "as" => Some(Self::As),
            "v" => Some(Self::V),
            "o" => Some(Self::O),
            "b" => Some(Self::B),
            "xa" => Some(Self::Xa),
            "xs" => Some(Self::Xs),
            "dx" => Some(Self::Dx),
            "jc" => Some(Self::Jc),
            "oa" => Some(Self::Oa),
            "vx" => Some(Self::Vx),
            "ts" => Some(Self::Ts),
            "xx" => Some(Self::Xx),
            _ => None,
        }
    }
}

/// Classify a single memory region entry into a MemRange.
/// This is a Rust port of the Kotlin `classifyRegion` in `DevideMemRange.kt`.
fn classify_region(entry: &WuwaMemRegionEntry) -> Option<MemRange> {
    let start = entry.start;
    let end = entry.end;
    if start == end {
        return None;
    }

    let is_readable = entry.type_ & MEM_READABLE != 0;
    let is_writable = entry.type_ & MEM_WRITABLE != 0;
    let is_executable = entry.type_ & MEM_EXECUTABLE != 0;
    let is_shared = entry.type_ & MEM_SHARED != 0;
    let non_prot = !is_readable && !is_writable && !is_executable;

    // Extract name as string
    let name = CStr::from_bytes_until_nul(&entry.name)
        .map(|c| c.to_string_lossy().into_owned())
        .unwrap_or_default();

    // Non-writable + executable => code sections
    if !is_writable && is_executable {
        if name.ends_with(".oat") && name.starts_with("/data/misc") {
            return Some(MemRange::Oa);
        }
        if name.contains("jit-cache")
            || name.contains("jit-code-cache")
            || name.contains("dalvik-jit")
        {
            return Some(MemRange::Jc);
        }
        if name.contains("/data/") {
            return Some(MemRange::Xa);
        }
        return Some(MemRange::Xs);
    }

    // /dev/ devices
    if !name.is_empty() && name.starts_with("/dev/") {
        let dev_gpu_keywords = [
            "/dev/mali", "/dev/kgsl", "/dev/nv", "/dev/tegra", "/dev/ion",
            "/dev/pvr", "/dev/render", "/dev/galcore", "/dev/fimg2d",
            "/dev/quadd", "/dev/graphics", "/dev/mm_", "/dev/dri/",
        ];
        for kw in &dev_gpu_keywords {
            if name.to_lowercase().contains(&kw.to_lowercase()) {
                return Some(MemRange::V);
            }
        }
        if name.contains("/dev/xLog") {
            return Some(MemRange::B);
        }
    }

    // Fonts => Bad or Other
    if !name.is_empty() {
        if name.starts_with("/system/fonts/")
            || name.starts_with("/product/fonts/")
            || name.starts_with("/data/data/com.google.android.gms/files/fonts/")
        {
            return if is_readable && is_shared {
                Some(MemRange::B)
            } else {
                Some(MemRange::O)
            };
        }
        if name.starts_with("anon_inode:dma_buf") {
            return Some(MemRange::B);
        }
    }

    // Special address
    if start == 0x10001000 && is_readable && is_writable {
        return Some(MemRange::S);
    }

    if !name.is_empty() {
        // Thread stack
        if (name.contains("[anon:stack_and_tls:")
            || name.contains("[anon:thread signal stack]"))
            && is_readable
            && is_writable
        {
            return Some(MemRange::Ts);
        }

        // VDEX
        if name.ends_with(".vdex") && is_readable {
            return Some(MemRange::Vx);
        }

        // DEX/ODEX
        if (name.ends_with(".dex")
            || name.ends_with(".odex")
            || name.contains(".dex (del")
            || name.contains(".odex (del"))
            && is_readable
        {
            return Some(MemRange::Dx);
        }

        // .bss
        if name.contains("[anon:.bss]") {
            return Some(MemRange::Cb);
        }

        // System
        if name.starts_with("/system/") {
            return Some(MemRange::O);
        }
        if name.starts_with("/dev/zero") {
            return Some(MemRange::O);
        }

        // PPSSPP
        if name.contains("PPSSPP_RAM") {
            return Some(MemRange::Ps);
        }

        // Filter out certain names
        let should_check = !name.is_empty()
            && !name.contains("system@")
            && !name.contains("gralloc")
            && !name.starts_with("[vdso]")
            && !name.starts_with("[vectors]")
            && !(name.starts_with("/dev/") && !name.starts_with("/dev/ashmem"));

        if should_check {
            // Dalvik/Java heap
            if name.contains("dalvik") {
                let is_dalvik_heap = (name.contains("eap")
                    || name.contains("dalvik-alloc")
                    || name.contains("dalvik-main")
                    || name.contains("dalvik-large")
                    || name.contains("dalvik-free"))
                    && !name.contains("itmap")
                    && !name.contains("ygote")
                    && !name.contains("ard")
                    && !name.contains("jit")
                    && !name.contains("inear");
                return if is_dalvik_heap {
                    Some(MemRange::Jh)
                } else {
                    Some(MemRange::J)
                };
            }

            // Shared libraries
            if name.contains("/lib") && name.contains(".so") {
                if name.contains("/data/") {
                    return Some(MemRange::Cd);
                }
            }

            // C++ alloc
            if name.contains("malloc") || name.contains("anon:scudo:") {
                return Some(MemRange::Ca);
            }

            // C++ heap
            if name.contains("[heap]") {
                return Some(MemRange::Ch);
            }

            // Stack
            if name.contains("[stack") {
                return Some(MemRange::S);
            }

            // Ashmem
            if name.starts_with("/dev/ashmem") && !name.contains("MemoryHeapBase") {
                return Some(MemRange::As);
            }
        }
    }

    // Anonymous (empty name)
    if name.is_empty() {
        return Some(MemRange::An);
    }

    if non_prot {
        return Some(MemRange::Xx);
    }

    Some(MemRange::O)
}

/// Query memory regions from the driver, classify each one, and return
/// filtered `(start, end)` pairs matching any of the requested ranges.
pub fn get_filtered_regions(requested_ranges: &[MemRange]) -> Result<Vec<(u64, u64)>> {
    let dm = DRIVER_MANAGER
        .read()
        .map_err(|e| anyhow!("Failed to acquire DriverManager lock: {}", e))?;

    if !dm.is_process_bound() {
        return Err(anyhow!("No process bound"));
    }

    let pid = dm.get_bound_pid();
    let driver = dm.get_driver().ok_or_else(|| anyhow!("Driver not loaded"))?;

    let result = driver
        .query_mem_regions(pid, 0, 0)
        .map_err(|e| anyhow!("Failed to query memory regions: {}", e))?;

    let borrowed_fd = unsafe { BorrowedFd::borrow_raw(result.fd) };

    let mapped = unsafe {
        mmap(
            None,
            NonZeroUsize::new(result.buffer_size)
                .ok_or_else(|| anyhow!("Invalid buffer size"))?,
            ProtFlags::PROT_READ,
            MapFlags::MAP_PRIVATE,
            borrowed_fd,
            0,
        )
    }
    .map_err(|e| anyhow!("mmap failed: {}", e))?;

    let entries_ptr = mapped.as_ptr() as *const WuwaMemRegionEntry;
    let mut regions = Vec::new();
    let mut readable_count = 0usize;
    let mut non_readable_count = 0usize;

    for i in 0..result.entry_count {
        let entry = unsafe { &*entries_ptr.add(i) };
        if let Some(range) = classify_region(entry) {
            if requested_ranges.contains(&range) {
                let is_readable = entry.type_ & MEM_READABLE != 0;
                if is_readable {
                    readable_count += 1;
                    regions.push((entry.start, entry.end));
                } else {
                    non_readable_count += 1;
                    // Skip non-readable regions - can't search memory we can't read
                }
            }
        }
    }

    unsafe {
        let _ = munmap(mapped, result.buffer_size);
        close(result.fd);
    }

    log::debug!(
        "get_filtered_regions: {} regions matched out of {} (readable={}, non_readable={})",
        regions.len(),
        result.entry_count,
        readable_count,
        non_readable_count
    );

    Ok(regions)
}

use std::time::Duration;
#[inline]
#[must_use]
pub const fn board_index(board_size: usize, r: usize, c: usize) -> usize {
    r * board_size + c
}
#[inline]
#[must_use]
pub fn duration_to_ns(duration: Duration) -> u64 {
    u64::try_from(duration.as_nanos()).unwrap_or(u64::MAX)
}
#[cfg(target_os = "windows")]
#[repr(C)]
struct MemoryStatusEx {
    dw_length: u32,
    dw_memory_load: u32,
    ull_total_phys: u64,
    ull_avail_phys: u64,
    ull_total_page_file: u64,
    ull_avail_page_file: u64,
    ull_total_virtual: u64,
    ull_avail_virtual: u64,
    ull_avail_extended_virtual: u64,
}
#[cfg(target_os = "windows")]
impl Default for MemoryStatusEx {
    fn default() -> Self {
        unsafe { std::mem::zeroed() }
    }
}
#[cfg(target_os = "windows")]
#[link(name = "kernel32")]
unsafe extern "system" {
    fn GlobalMemoryStatusEx(lpBuffer: *mut MemoryStatusEx) -> i32;
}
#[cfg(target_os = "windows")]
#[must_use]
pub fn available_memory_bytes() -> Option<u64> {
    let dw_length = u32::try_from(std::mem::size_of::<MemoryStatusEx>()).ok()?;
    let mut status = MemoryStatusEx {
        dw_length,
        ..MemoryStatusEx::default()
    };
    let ok = unsafe { GlobalMemoryStatusEx(&raw mut status) };
    if ok == 0 {
        return None;
    }
    Some(status.ull_avail_phys)
}
#[cfg(target_os = "linux")]
#[must_use]
pub fn available_memory_bytes() -> Option<u64> {
    let contents = std::fs::read_to_string("/proc/meminfo").ok()?;
    for line in contents.lines() {
        let line = line.trim_start();
        if let Some(rest) = line.strip_prefix("MemAvailable:") {
            let mut parts = rest.split_whitespace();
            let value_kb: u64 = parts.next()?.parse().ok()?;
            return Some(value_kb.saturating_mul(1024));
        }
    }
    None
}
#[cfg(not(any(target_os = "windows", target_os = "linux")))]
#[must_use]
pub fn available_memory_bytes() -> Option<u64> {
    None
}

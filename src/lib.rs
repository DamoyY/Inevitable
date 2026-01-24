#[macro_export]
macro_rules! for_each_move_apply_timing {
    ($macro:ident) => {
        $macro! {
            board_update_ns => board_update_time_ns,
            bitboard_update_ns => bitboard_update_time_ns,
            threat_index_update_ns => threat_index_update_time_ns,
            candidate_remove_ns => candidate_remove_time_ns,
            candidate_neighbor_ns => candidate_neighbor_time_ns,
            candidate_insert_ns => candidate_insert_time_ns,
            candidate_newly_added_ns => candidate_newly_added_time_ns,
            candidate_history_ns => candidate_history_time_ns,
            hash_update_ns => hash_update_time_ns,
        }
    };
}
pub mod alloc_stats {
    use std::{
        alloc::{GlobalAlloc, Layout},
        cell::Cell,
        sync::atomic::{AtomicU64, Ordering},
        time::{Duration, Instant},
    };

    use mimalloc::MiMalloc;

    use crate::utils::duration_to_ns;
    static ALLOC_TIME_NS: AtomicU64 = AtomicU64::new(0);
    static DEALLOC_TIME_NS: AtomicU64 = AtomicU64::new(0);
    static REALLOC_TIME_NS: AtomicU64 = AtomicU64::new(0);
    static ALLOC_ZEROED_TIME_NS: AtomicU64 = AtomicU64::new(0);
    thread_local! {
        static ALLOC_TRACKING_DEPTH: Cell<u32> = const { Cell::new(0) };
    }
    #[must_use]
    pub struct AllocTrackingGuard;
    impl AllocTrackingGuard {
        pub fn new() -> Self {
            ALLOC_TRACKING_DEPTH.with(|depth| {
                depth.set(depth.get().saturating_add(1));
            });
            Self
        }
    }
    impl Default for AllocTrackingGuard {
        fn default() -> Self {
            Self::new()
        }
    }
    impl Drop for AllocTrackingGuard {
        fn drop(&mut self) {
            ALLOC_TRACKING_DEPTH.with(|depth| {
                depth.set(depth.get().saturating_sub(1));
            });
        }
    }
    pub fn reset_alloc_timing_ns() {
        ALLOC_TIME_NS.store(0, Ordering::Relaxed);
        DEALLOC_TIME_NS.store(0, Ordering::Relaxed);
        REALLOC_TIME_NS.store(0, Ordering::Relaxed);
        ALLOC_ZEROED_TIME_NS.store(0, Ordering::Relaxed);
    }
    #[derive(Clone, Copy, Default)]
    pub struct AllocTimingSnapshot {
        pub alloc_ns: u64,
        pub dealloc_ns: u64,
        pub realloc_ns: u64,
        pub alloc_zeroed_ns: u64,
    }
    impl AllocTimingSnapshot {
        #[must_use]
        pub const fn total_ns(self) -> u64 {
            self.alloc_ns
                .saturating_add(self.dealloc_ns)
                .saturating_add(self.realloc_ns)
                .saturating_add(self.alloc_zeroed_ns)
        }
    }
    pub fn alloc_timing_snapshot() -> AllocTimingSnapshot {
        AllocTimingSnapshot {
            alloc_ns: ALLOC_TIME_NS.load(Ordering::Relaxed),
            dealloc_ns: DEALLOC_TIME_NS.load(Ordering::Relaxed),
            realloc_ns: REALLOC_TIME_NS.load(Ordering::Relaxed),
            alloc_zeroed_ns: ALLOC_ZEROED_TIME_NS.load(Ordering::Relaxed),
        }
    }
    fn tracking_enabled() -> bool {
        ALLOC_TRACKING_DEPTH.with(|depth| depth.get() > 0)
    }
    fn record_alloc_time(target: &AtomicU64, elapsed: Duration) {
        target.fetch_add(duration_to_ns(elapsed), Ordering::Relaxed);
    }
    fn track_alloc_time<R>(target: &AtomicU64, action: impl FnOnce() -> R) -> R {
        if tracking_enabled() {
            let start = Instant::now();
            let result = action();
            record_alloc_time(target, start.elapsed());
            result
        } else {
            action()
        }
    }
    #[must_use]
    pub struct TrackingAllocator {
        inner: MiMalloc,
    }
    impl TrackingAllocator {
        pub const fn new() -> Self {
            Self { inner: MiMalloc }
        }
    }
    impl Default for TrackingAllocator {
        fn default() -> Self {
            Self::new()
        }
    }
    unsafe impl GlobalAlloc for TrackingAllocator {
        unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
            track_alloc_time(&ALLOC_TIME_NS, || unsafe { self.inner.alloc(layout) })
        }

        unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
            track_alloc_time(&DEALLOC_TIME_NS, || unsafe {
                self.inner.dealloc(ptr, layout);
            });
        }

        unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
            track_alloc_time(&REALLOC_TIME_NS, || unsafe {
                self.inner.realloc(ptr, layout, new_size)
            })
        }

        unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
            track_alloc_time(&ALLOC_ZEROED_TIME_NS, || unsafe {
                self.inner.alloc_zeroed(layout)
            })
        }
    }
}
pub mod config {
    use std::{fs, process, thread};

    use serde::Deserialize;
    #[derive(Debug, Deserialize, Clone, Copy)]
    pub struct EvaluationConfig {
        pub proximity_kernel_size: usize,
        pub proximity_scale: f32,
        pub positional_bonus_scale: f32,
        pub score_win: f32,
        pub score_live_four: f32,
        pub score_blocked_four: f32,
        pub score_live_three: f32,
        pub score_live_two: f32,
        pub score_block_win: f32,
        pub score_block_live_four: f32,
        pub score_block_blocked_four: f32,
        pub score_block_live_three: f32,
    }
    #[derive(Debug, Deserialize)]
    pub struct Config {
        pub board_size: usize,
        pub win_len: usize,
        pub verbose: bool,
        pub num_threads: usize,
        pub evaluation: EvaluationConfig,
        #[serde(default = "default_min_available_memory_mb")]
        pub min_available_memory_mb: u64,
        #[serde(default = "default_memory_check_interval_ms")]
        pub memory_check_interval_ms: u64,
    }

    const fn default_min_available_memory_mb() -> u64 {
        1024
    }

    const fn default_memory_check_interval_ms() -> u64 {
        500
    }
    impl Config {
        pub fn load() -> Self {
            let config_str = fs::read_to_string("config.yaml").unwrap_or_else(|err| {
                eprintln!("无法读取 config.yaml: {err}");
                process::exit(1);
            });
            let mut config: Self = serde_yaml::from_str(&config_str).unwrap_or_else(|err| {
                eprintln!("解析 config.yaml 失败: {err}");
                process::exit(1);
            });
            if config.num_threads == 0 {
                config.num_threads =
                    thread::available_parallelism().map_or(4, std::num::NonZero::get);
            }
            config
        }
    }
}
pub mod game_state;
pub mod pns;
pub mod ui;
pub mod utils {
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
}

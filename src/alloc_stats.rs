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

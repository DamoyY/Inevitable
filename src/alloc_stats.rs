use std::alloc::{GlobalAlloc, Layout};
use std::cell::Cell;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use mimalloc::MiMalloc;

static ALLOC_FREE_TIME_NS: AtomicU64 = AtomicU64::new(0);

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

pub fn reset_alloc_free_time_ns() {
    ALLOC_FREE_TIME_NS.store(0, Ordering::Relaxed);
}

pub fn alloc_free_time_ns() -> u64 {
    ALLOC_FREE_TIME_NS.load(Ordering::Relaxed)
}

fn tracking_enabled() -> bool {
    ALLOC_TRACKING_DEPTH.with(|depth| depth.get() > 0)
}

fn duration_to_ns(duration: Duration) -> u64 {
    let nanos = duration.as_nanos();
    u64::try_from(nanos).unwrap_or(u64::MAX)
}

fn record_alloc_time(elapsed: Duration) {
    ALLOC_FREE_TIME_NS.fetch_add(duration_to_ns(elapsed), Ordering::Relaxed);
}

fn track_alloc_time<R>(action: impl FnOnce() -> R) -> R {
    if tracking_enabled() {
        let start = Instant::now();
        let result = action();
        record_alloc_time(start.elapsed());
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
        track_alloc_time(|| unsafe { self.inner.alloc(layout) })
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        track_alloc_time(|| unsafe { self.inner.dealloc(ptr, layout) });
    }

    unsafe fn realloc(
        &self,
        ptr: *mut u8,
        layout: Layout,
        new_size: usize,
    ) -> *mut u8 {
        track_alloc_time(|| unsafe { self.inner.realloc(ptr, layout, new_size) })
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        track_alloc_time(|| unsafe { self.inner.alloc_zeroed(layout) })
    }
}

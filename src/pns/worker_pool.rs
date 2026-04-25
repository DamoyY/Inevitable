use super::{SharedTree, context::ThreadLocalContext, node::Worker};
use crate::{alloc_stats::AllocTrackingGuard, checked, game_state::GameState};
use alloc::{sync::Arc, vec::Vec};
use core::panic::AssertUnwindSafe;
use std::{
    panic,
    sync::{Condvar, Mutex, MutexGuard},
    thread::{self, JoinHandle},
};
struct WorkerPoolState {
    generation: u64,
    active_workers: usize,
    ready_workers: usize,
    worker_failed: bool,
    shutdown: bool,
}
struct WorkerPoolSync {
    state: Mutex<WorkerPoolState>,
    round_condvar: Condvar,
    idle_condvar: Condvar,
    ready_condvar: Condvar,
}
impl WorkerPoolSync {
    const fn new() -> Self {
        Self {
            state: Mutex::new(WorkerPoolState {
                generation: 0,
                active_workers: 0,
                ready_workers: 0,
                worker_failed: false,
                shutdown: false,
            }),
            round_condvar: Condvar::new(),
            idle_condvar: Condvar::new(),
            ready_condvar: Condvar::new(),
        }
    }
    fn lock_state(&self) -> MutexGuard<'_, WorkerPoolState> {
        match self.state.lock() {
            Ok(guard) => guard,
            Err(err) => err.into_inner(),
        }
    }
    fn wait<'guard>(
        condvar: &Condvar,
        guard: MutexGuard<'guard, WorkerPoolState>,
    ) -> MutexGuard<'guard, WorkerPoolState> {
        match condvar.wait(guard) {
            Ok(waited_guard) => waited_guard,
            Err(err) => err.into_inner(),
        }
    }
    fn mark_ready(&self) {
        let mut state = self.lock_state();
        state.ready_workers = checked::add_usize(
            state.ready_workers,
            1_usize,
            "WorkerPoolSync::mark_ready::ready_workers",
        );
        drop(state);
        self.ready_condvar.notify_one();
    }
    fn wait_until_ready(&self, expected_workers: usize) -> Result<(), ()> {
        let mut state = self.lock_state();
        while state.ready_workers < expected_workers && !state.worker_failed {
            state = Self::wait(&self.ready_condvar, state);
        }
        if state.worker_failed { Err(()) } else { Ok(()) }
    }
    fn wait_for_round(&self, observed_generation: &mut u64) -> bool {
        let mut state = self.lock_state();
        while !state.shutdown && state.generation == *observed_generation {
            state = Self::wait(&self.round_condvar, state);
        }
        if state.shutdown {
            return false;
        }
        *observed_generation = state.generation;
        true
    }
    fn begin_round_and_wait(&self, worker_count: usize) {
        let mut state = self.lock_state();
        if state.worker_failed {
            eprintln!("工作线程池已失效，无法继续搜索。");
            panic!("工作线程池已失效");
        }
        if state.active_workers != 0 {
            eprintln!("工作线程池上一轮搜索尚未结束。");
            panic!("工作线程池上一轮搜索尚未结束");
        }
        state.generation = checked::add_u64(
            state.generation,
            1_u64,
            "WorkerPoolSync::begin_round_and_wait::generation",
        );
        state.active_workers = worker_count;
        self.round_condvar.notify_all();
        while state.active_workers > 0 && !state.worker_failed {
            state = Self::wait(&self.idle_condvar, state);
        }
        if state.worker_failed {
            eprintln!("工作线程在搜索过程中异常退出。");
            panic!("工作线程在搜索过程中异常退出");
        }
    }
    fn finish_round(&self, tree: &SharedTree, round_panicking: bool) {
        let mut state = self.lock_state();
        if round_panicking {
            state.worker_failed = true;
            state.shutdown = true;
            tree.mark_solved();
        }
        if state.active_workers == 0 {
            eprintln!("工作线程轮次计数异常。");
            panic!("工作线程轮次计数异常");
        }
        state.active_workers = checked::sub_usize(
            state.active_workers,
            1_usize,
            "WorkerPoolSync::finish_round::active_workers",
        );
        if state.active_workers == 0 {
            self.idle_condvar.notify_all();
        }
        if state.worker_failed || state.shutdown {
            self.round_condvar.notify_all();
            self.idle_condvar.notify_all();
            self.ready_condvar.notify_all();
        }
    }
    fn mark_thread_failure(&self, tree: &SharedTree) {
        let mut state = self.lock_state();
        state.worker_failed = true;
        state.shutdown = true;
        tree.mark_solved();
        drop(state);
        self.round_condvar.notify_all();
        self.idle_condvar.notify_all();
        self.ready_condvar.notify_all();
    }
    fn shutdown(&self) {
        let mut state = self.lock_state();
        state.shutdown = true;
        drop(state);
        self.round_condvar.notify_all();
        self.idle_condvar.notify_all();
        self.ready_condvar.notify_all();
    }
}
struct WorkerRoundGuard {
    sync: Arc<WorkerPoolSync>,
    tree: Arc<SharedTree>,
}
impl WorkerRoundGuard {
    const fn new(sync: Arc<WorkerPoolSync>, tree: Arc<SharedTree>) -> Self {
        Self { sync, tree }
    }
}
impl Drop for WorkerRoundGuard {
    fn drop(&mut self) {
        self.sync.finish_round(&self.tree, thread::panicking());
    }
}
pub(crate) struct WorkerPool {
    tree: Arc<SharedTree>,
    sync: Arc<WorkerPoolSync>,
    handles: Vec<JoinHandle<()>>,
}
impl WorkerPool {
    pub(crate) fn new(tree: Arc<SharedTree>, game_state: &GameState, num_threads: usize) -> Self {
        let sync = Arc::new(WorkerPoolSync::new());
        let mut handles = Vec::with_capacity(num_threads);
        for thread_id in 0..num_threads {
            let cloned_tree = Arc::clone(&tree);
            let cloned_sync = Arc::clone(&sync);
            let worker_game_state = (*game_state).clone();
            handles.push(thread::spawn(move || {
                run_worker_thread(&cloned_tree, &worker_game_state, thread_id, &cloned_sync);
            }));
        }
        let mut pool = Self {
            tree,
            sync,
            handles,
        };
        if pool.sync.wait_until_ready(num_threads).is_err() {
            pool.shutdown_and_join();
            eprintln!("工作线程池初始化失败。");
            panic!("工作线程池初始化失败");
        }
        pool
    }
    pub(crate) fn run_and_wait(&self) {
        self.sync.begin_round_and_wait(self.handles.len());
    }
    fn shutdown_and_join(&mut self) {
        self.tree.mark_solved();
        self.sync.shutdown();
        while let Some(handle) = self.handles.pop() {
            if handle.join().is_err() {
                eprintln!("工作线程异常退出。");
            }
        }
    }
}
impl Drop for WorkerPool {
    fn drop(&mut self) {
        self.shutdown_and_join();
    }
}
fn run_worker_thread(
    tree: &Arc<SharedTree>,
    game_state: &GameState,
    thread_id: usize,
    sync: &Arc<WorkerPoolSync>,
) {
    let thread_tree = Arc::clone(tree);
    let thread_sync = Arc::clone(sync);
    let result = panic::catch_unwind(AssertUnwindSafe(|| {
        let ctx = {
            let _alloc_guard = AllocTrackingGuard::new();
            ThreadLocalContext::new((*game_state).clone(), thread_id)
        };
        thread_sync.mark_ready();
        let mut worker = Worker::new(Arc::clone(&thread_tree), ctx);
        let mut observed_generation = 0_u64;
        loop {
            if !thread_sync.wait_for_round(&mut observed_generation) {
                return;
            }
            let _round_guard =
                WorkerRoundGuard::new(Arc::clone(&thread_sync), Arc::clone(&thread_tree));
            let _alloc_guard = AllocTrackingGuard::new();
            worker.run();
        }
    }));
    if result.is_err() {
        thread_sync.mark_thread_failure(&thread_tree);
    }
}

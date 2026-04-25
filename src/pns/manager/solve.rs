use super::super::{SharedTree, context::ThreadLocalContext, node::Worker};
use super::ParallelSolver;
use crate::alloc_stats::AllocTrackingGuard;
use crate::checked;
use alloc::sync::Arc;
use core::sync::atomic::{AtomicBool, Ordering};
use std::{thread, time::Instant};
pub(super) fn solve(solver: &ParallelSolver, verbose: bool) -> bool {
    let start_time = Instant::now();
    let _alloc_guard = AllocTrackingGuard::new();
    let tree = Arc::clone(&solver.tree);
    if tree.stop_requested() {
        return false;
    }
    if tree.root.is_terminal() {
        if verbose {
            println!(
                "根节点已是终端状态: PN={}, DN={}",
                super::logging::format_sci_u64(tree.root.get_pn()),
                super::logging::format_sci_u64(tree.root.get_dn())
            );
        }
        if tree.root.get_pn() == 0 && !tree.root.is_expanded() {
            let mut ctx = ThreadLocalContext::new(super::setup::clone_game_state(solver), 0);
            tree.expand_node(&tree.root, &mut ctx);
            tree.update_node_pdn(&tree.root);
        }
        return tree.root.get_pn() == 0;
    }
    let handles = spawn_workers(solver, &tree);
    wait_for_workers(handles);
    let elapsed = start_time.elapsed().as_secs_f64();
    if verbose {
        super::logging::write_csv_log(&solver.tree, super::setup::current_turn(solver), elapsed);
    }
    solver.tree.root.get_pn() == 0
}
fn spawn_workers(solver: &ParallelSolver, tree: &Arc<SharedTree>) -> Vec<thread::JoinHandle<()>> {
    (0..solver.num_threads)
        .map(|thread_id| {
            let cloned_tree = Arc::clone(tree);
            let game_state = super::setup::clone_game_state(solver);
            thread::spawn(move || {
                let _alloc_guard = AllocTrackingGuard::new();
                let ctx = ThreadLocalContext::new(game_state, thread_id);
                let mut worker = Worker::new(cloned_tree, ctx);
                worker.run();
            })
        })
        .collect()
}
fn wait_for_workers(handles: Vec<thread::JoinHandle<()>>) {
    for handle in handles {
        if handle.join().is_err() {
            eprintln!("工作线程异常退出。");
        }
    }
}
pub(super) fn run_iterative_deepening<R, H>(
    solver: &mut ParallelSolver,
    stop_flag: &Arc<AtomicBool>,
    mut depth: usize,
    hooks: &mut H,
) -> R
where
    H: super::deepening::IterativeDeepeningHooks<R>,
{
    loop {
        if stop_flag.load(Ordering::Acquire) {
            return hooks.on_stop(solver);
        }
        hooks.before_solve(depth, solver);
        let found = hooks.solve(solver);
        if stop_flag.load(Ordering::Acquire) || solver.tree.stop_requested() {
            return hooks.on_stop(solver);
        }
        hooks.after_solve(depth, solver, found);
        if found {
            return hooks.on_found(depth, solver);
        }
        depth = checked::add_usize(depth, 1_usize, "ParallelSolver::run_iterative_deepening");
        if stop_flag.load(Ordering::Acquire) {
            return hooks.on_stop(solver);
        }
        super::setup::increase_depth_limit(solver, depth);
    }
}

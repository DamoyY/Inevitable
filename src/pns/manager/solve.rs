use super::super::context::ThreadLocalContext;
use super::ParallelSolver;
use crate::alloc_stats::AllocTrackingGuard;
use crate::checked;
use alloc::sync::Arc;
use core::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;
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
    solver.worker_pool.run_and_wait();
    let elapsed = start_time.elapsed().as_secs_f64();
    if verbose {
        super::logging::write_csv_log(&solver.tree, super::setup::current_turn(solver), elapsed);
    }
    solver.tree.root.get_pn() == 0
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

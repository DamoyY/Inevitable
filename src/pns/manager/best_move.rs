use super::super::{NodeTable, TranspositionTable};
use super::{ParallelSolver, SearchParams};
use crate::{checked, config::EvaluationWeights};
use alloc::sync::Arc;
use core::sync::atomic::AtomicBool;
pub(super) fn find_best_move_iterative_deepening(
    initial_board: Vec<u8>,
    board_size: usize,
    win_len: usize,
    num_threads: usize,
    evaluation: EvaluationWeights,
    verbose: bool,
) -> Option<(usize, usize)> {
    let params = SearchParams::new(board_size, win_len, num_threads, evaluation);
    find_best_move_with_tt(initial_board, params, verbose, None, None).0
}
pub(super) fn find_best_move_with_tt(
    initial_board: Vec<u8>,
    params: SearchParams,
    verbose: bool,
    existing_tt: Option<TranspositionTable>,
    existing_node_table: Option<NodeTable>,
) -> (Option<(usize, usize)>, TranspositionTable, NodeTable) {
    let stop_flag = Arc::new(AtomicBool::new(false));
    find_best_move_with_tt_and_stop(
        initial_board,
        params,
        verbose,
        &stop_flag,
        existing_tt,
        existing_node_table,
    )
}
pub(super) fn find_best_move_with_tt_and_stop(
    initial_board: Vec<u8>,
    params: SearchParams,
    verbose: bool,
    stop_flag: &Arc<AtomicBool>,
    existing_tt: Option<TranspositionTable>,
    existing_node_table: Option<NodeTable>,
) -> (Option<(usize, usize)>, TranspositionTable, NodeTable) {
    let depth = 1_usize;
    let mut solver = super::setup::with_tt_and_stop(
        initial_board,
        params,
        Some(depth),
        stop_flag,
        existing_tt,
        existing_node_table,
    );
    let mut hooks = super::deepening::BestMoveDeepening { verbose };
    super::solve::run_iterative_deepening(&mut solver, stop_flag, depth, &mut hooks)
}
pub(super) fn get_tt(solver: &ParallelSolver) -> TranspositionTable {
    solver.tree.get_tt()
}
pub(super) fn get_node_table(solver: &ParallelSolver) -> NodeTable {
    solver.tree.get_node_table()
}
pub(super) fn get_best_move(solver: &ParallelSolver) -> Option<(usize, usize)> {
    let root = &solver.tree.root;
    if root.get_pn() != 0 {
        return None;
    }
    let children = root.children.get()?.clone();
    if children.is_empty() {
        return None;
    }
    let root_win_len = root.get_win_len();
    let winning_children: Vec<_> = children
        .iter()
        .filter(|child_ref| {
            child_ref.node.get_pn() == 0
                && checked::add_u64(
                    1_u64,
                    child_ref.node.get_win_len(),
                    "ParallelSolver::get_best_move::root_win_len",
                ) == root_win_len
        })
        .collect();
    if winning_children.is_empty() {
        children
            .iter()
            .filter(|child_ref| child_ref.node.get_pn() == 0)
            .min_by_key(|child_ref| (child_ref.node.get_win_len(), child_ref.mov))
            .map(|child_ref| child_ref.mov)
    } else {
        winning_children
            .iter()
            .min_by_key(|child_ref| (child_ref.node.get_win_len(), child_ref.mov))
            .map(|child_ref| child_ref.mov)
    }
}

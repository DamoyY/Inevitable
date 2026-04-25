use super::super::{
    NodeTable, SharedTree, TranspositionTable, WorkerPool, context::ThreadLocalContext,
};
use super::{ParallelSolver, SearchParams};
use crate::{
    alloc_stats,
    alloc_stats::AllocTrackingGuard,
    checked,
    config::EvaluationWeights,
    game_state::{GameState, ZobristHasher},
};
use alloc::sync::Arc;
use core::sync::atomic::AtomicBool;
pub(super) fn new(
    initial_board: Vec<u8>,
    board_size: usize,
    win_len: usize,
    depth_limit: Option<usize>,
    num_threads: usize,
    evaluation: EvaluationWeights,
) -> ParallelSolver {
    let params = SearchParams::new(board_size, win_len, num_threads, evaluation);
    with_tt(initial_board, params, depth_limit, None, None)
}
pub(super) fn with_tt(
    initial_board: Vec<u8>,
    params: SearchParams,
    depth_limit: Option<usize>,
    existing_tt: Option<TranspositionTable>,
    existing_node_table: Option<NodeTable>,
) -> ParallelSolver {
    let stop_flag = Arc::new(AtomicBool::new(false));
    with_tt_and_stop(
        initial_board,
        params,
        depth_limit,
        &stop_flag,
        existing_tt,
        existing_node_table,
    )
}
pub(super) fn with_tt_and_stop(
    initial_board: Vec<u8>,
    params: SearchParams,
    depth_limit: Option<usize>,
    stop_flag: &Arc<AtomicBool>,
    existing_tt: Option<TranspositionTable>,
    existing_node_table: Option<NodeTable>,
) -> ParallelSolver {
    alloc_stats::reset_alloc_timing_ns();
    let _alloc_guard = AllocTrackingGuard::new();
    let hasher = Arc::new(ZobristHasher::new(params.board_size));
    let game_state = GameState::new(
        initial_board,
        params.board_size,
        hasher,
        1,
        params.win_len,
        params.evaluation,
    );
    let root_hash = game_state.position.get_canonical_hash();
    let root_pos_hash = game_state.position.get_hash();
    let tree = Arc::new(SharedTree::with_tt_and_stop(
        1,
        root_hash,
        root_pos_hash,
        depth_limit,
        Arc::clone(stop_flag),
        existing_tt,
        existing_node_table,
    ));
    tree.evaluate_node(&tree.root, &ThreadLocalContext::new(game_state.clone(), 0));
    let worker_pool = WorkerPool::new(Arc::clone(&tree), &game_state, params.num_threads);
    ParallelSolver {
        tree,
        worker_pool,
        base_game_state: game_state,
        board_size: params.board_size,
        win_len: params.win_len,
    }
}
pub(super) fn clone_game_state(solver: &ParallelSolver) -> GameState {
    solver.base_game_state.clone()
}
pub(super) fn current_turn(solver: &ParallelSolver) -> usize {
    solver
        .base_game_state
        .position
        .board
        .iter()
        .fold(0_usize, |count, &cell| {
            checked::add_usize(
                count,
                usize::from(cell == 2),
                "ParallelSolver::current_turn",
            )
        })
}
pub(super) fn increase_depth_limit(solver: &ParallelSolver, new_limit: usize) {
    solver.tree.increase_depth_limit(new_limit);
}

use super::super::{NodeTable, TranspositionTable};
use super::{BenchmarkResult, ParallelSolver, SearchParams};
use alloc::sync::Arc;
use core::sync::atomic::AtomicBool;
impl ParallelSolver {
    pub fn new(
        initial_board: Vec<u8>,
        board_size: usize,
        win_len: usize,
        depth_limit: Option<usize>,
        num_threads: usize,
        evaluation: crate::config::EvaluationWeights,
    ) -> Self {
        super::setup::new(
            initial_board,
            board_size,
            win_len,
            depth_limit,
            num_threads,
            evaluation,
        )
    }
    pub fn with_tt(
        initial_board: Vec<u8>,
        params: SearchParams,
        depth_limit: Option<usize>,
        existing_tt: Option<TranspositionTable>,
        existing_node_table: Option<NodeTable>,
    ) -> Self {
        super::setup::with_tt(
            initial_board,
            params,
            depth_limit,
            existing_tt,
            existing_node_table,
        )
    }
    pub fn with_tt_and_stop(
        initial_board: Vec<u8>,
        params: SearchParams,
        depth_limit: Option<usize>,
        stop_flag: &Arc<AtomicBool>,
        existing_tt: Option<TranspositionTable>,
        existing_node_table: Option<NodeTable>,
    ) -> Self {
        super::setup::with_tt_and_stop(
            initial_board,
            params,
            depth_limit,
            stop_flag,
            existing_tt,
            existing_node_table,
        )
    }
    pub fn increase_depth_limit(&self, new_limit: usize) {
        super::setup::increase_depth_limit(self, new_limit);
    }
    pub fn solve(&self, verbose: bool) -> bool {
        super::solve::solve(self, verbose)
    }
    pub fn benchmark_next_move(
        initial_board: &[u8],
        params: SearchParams,
        runs: usize,
        stop_flag: &Arc<AtomicBool>,
    ) -> Option<BenchmarkResult> {
        super::benchmark::benchmark_next_move(initial_board, params, runs, stop_flag)
    }
    pub fn find_best_move_iterative_deepening(
        initial_board: Vec<u8>,
        board_size: usize,
        win_len: usize,
        num_threads: usize,
        evaluation: crate::config::EvaluationWeights,
        verbose: bool,
    ) -> Option<(usize, usize)> {
        super::best_move::find_best_move_iterative_deepening(
            initial_board,
            board_size,
            win_len,
            num_threads,
            evaluation,
            verbose,
        )
    }
    pub fn find_best_move_with_tt(
        initial_board: Vec<u8>,
        params: SearchParams,
        verbose: bool,
        existing_tt: Option<TranspositionTable>,
        existing_node_table: Option<NodeTable>,
    ) -> (Option<(usize, usize)>, TranspositionTable, NodeTable) {
        super::best_move::find_best_move_with_tt(
            initial_board,
            params,
            verbose,
            existing_tt,
            existing_node_table,
        )
    }
    pub fn find_best_move_with_tt_and_stop(
        initial_board: Vec<u8>,
        params: SearchParams,
        verbose: bool,
        stop_flag: &Arc<AtomicBool>,
        existing_tt: Option<TranspositionTable>,
        existing_node_table: Option<NodeTable>,
    ) -> (Option<(usize, usize)>, TranspositionTable, NodeTable) {
        super::best_move::find_best_move_with_tt_and_stop(
            initial_board,
            params,
            verbose,
            stop_flag,
            existing_tt,
            existing_node_table,
        )
    }
    pub fn get_tt(&self) -> TranspositionTable {
        super::best_move::get_tt(self)
    }
    pub fn get_node_table(&self) -> NodeTable {
        super::best_move::get_node_table(self)
    }
    pub fn get_best_move(&self) -> Option<(usize, usize)> {
        super::best_move::get_best_move(self)
    }
    pub fn root_pn(&self) -> u64 {
        super::accessors::root_pn(self)
    }
    pub fn root_dn(&self) -> u64 {
        super::accessors::root_dn(self)
    }
    pub fn root_player(&self) -> u8 {
        super::accessors::root_player(self)
    }
    pub fn root_win_len(&self) -> u64 {
        super::accessors::root_win_len(self)
    }
    pub const fn game_state(&self) -> &crate::game_state::GameState {
        super::accessors::game_state(self)
    }
    pub const fn board_size(&self) -> usize {
        super::accessors::board_size(self)
    }
    pub const fn win_len(&self) -> usize {
        super::accessors::win_len(self)
    }
}

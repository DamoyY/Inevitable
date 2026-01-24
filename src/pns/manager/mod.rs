use std::{
    collections::BTreeMap,
    sync::{Arc, atomic::AtomicBool},
    time::Instant,
};

use super::{
    TreeStatsSnapshot,
    context::ThreadLocalContext,
    shared_tree::{NodeTable, SharedTree, TranspositionTable},
};
use crate::{
    alloc_stats,
    config::EvaluationConfig,
    game_state::{GomokuGameState, ZobristHasher},
};
mod logging;
mod solve;
use logging::{format_sci_u64, format_sci_usize};
pub struct ParallelSolver {
    pub tree: Arc<SharedTree>,
    pub base_game_state: GomokuGameState,
    pub num_threads: usize,
    board_size: usize,
    win_len: usize,
}
#[derive(Clone, Copy)]
pub struct SearchParams {
    pub board_size: usize,
    pub win_len: usize,
    pub num_threads: usize,
    pub evaluation: EvaluationConfig,
}
impl SearchParams {
    #[must_use]
    pub const fn new(
        board_size: usize,
        win_len: usize,
        num_threads: usize,
        evaluation: EvaluationConfig,
    ) -> Self {
        Self {
            board_size,
            win_len,
            num_threads,
            evaluation,
        }
    }
}
pub struct BenchmarkResult {
    pub elapsed_secs: f64,
    pub stats: TreeStatsSnapshot,
    pub tt_size: usize,
    pub node_table_size: usize,
}
#[derive(Default)]
struct DepthAccumulator {
    total_stats: TreeStatsSnapshot,
    total_elapsed_secs: f64,
    total_tt_size: u64,
    total_node_table_size: u64,
    count: u64,
}
impl DepthAccumulator {
    fn add_sample(
        &mut self,
        stats: TreeStatsSnapshot,
        elapsed_secs: f64,
        tt_size: u64,
        node_table_size: u64,
    ) {
        self.total_stats.add_assign(&stats);
        self.total_elapsed_secs += elapsed_secs;
        self.total_tt_size = self.total_tt_size.saturating_add(tt_size);
        self.total_node_table_size = self.total_node_table_size.saturating_add(node_table_size);
        self.count = self.count.saturating_add(1);
    }

    fn average(&self) -> (TreeStatsSnapshot, f64, usize, usize) {
        let count = self.count.max(1);
        let divisor = f64::from(u32::try_from(count).unwrap_or(u32::MAX));
        let stats = self.total_stats.div_round(count);
        let elapsed_secs = self.total_elapsed_secs / divisor;
        let tt_size_u64 = (self.total_tt_size.saturating_add(count / 2)) / count;
        let node_table_size_u64 = (self.total_node_table_size.saturating_add(count / 2)) / count;
        let tt_size = usize::try_from(tt_size_u64).unwrap_or(usize::MAX);
        let node_table_size = usize::try_from(node_table_size_u64).unwrap_or(usize::MAX);
        (stats, elapsed_secs, tt_size, node_table_size)
    }
}

fn write_benchmark_logs(per_depth: BTreeMap<usize, DepthAccumulator>) {
    for (depth, acc) in per_depth {
        if acc.count == 0 {
            continue;
        }
        let (stats, elapsed_secs, tt_size, node_table_size) = acc.average();
        logging::write_csv_log_snapshot(
            1,
            elapsed_secs,
            stats,
            tt_size,
            node_table_size,
            Some(depth),
        );
    }
}
trait IterativeDeepeningHooks<R> {
    fn on_stop(&mut self, solver: &mut ParallelSolver) -> R;
    fn before_solve(&mut self, _depth: usize, _solver: &mut ParallelSolver) {}
    fn solve(&mut self, solver: &mut ParallelSolver) -> bool;
    fn after_solve(&mut self, _depth: usize, _solver: &mut ParallelSolver, _found: bool) {}
    fn on_found(&mut self, _depth: usize, solver: &mut ParallelSolver) -> R;
}
struct BenchmarkDeepening<'a> {
    start: Instant,
    per_depth: &'a mut BTreeMap<usize, DepthAccumulator>,
    prev_stats: TreeStatsSnapshot,
    prev_elapsed: f64,
    last_tt_size: u64,
    last_node_table_size: u64,
    total_stats: &'a mut TreeStatsSnapshot,
    total_elapsed_secs: &'a mut f64,
    total_tt_size: &'a mut u64,
    total_node_table_size: &'a mut u64,
}
impl IterativeDeepeningHooks<Option<()>> for BenchmarkDeepening<'_> {
    fn on_stop(&mut self, _solver: &mut ParallelSolver) -> Option<()> {
        None
    }

    fn solve(&mut self, solver: &mut ParallelSolver) -> bool {
        solver.solve(false)
    }

    fn after_solve(&mut self, depth: usize, solver: &mut ParallelSolver, _found: bool) {
        let elapsed = self.start.elapsed().as_secs_f64();
        let current_stats = solver.tree.stats_snapshot();
        let delta_stats = current_stats.delta_since(&self.prev_stats);
        let delta_elapsed = (elapsed - self.prev_elapsed).max(0.0);
        let tt_size = solver.tree.get_tt_size() as u64;
        let node_table_size = solver.tree.get_node_table_size() as u64;
        let entry = self.per_depth.entry(depth).or_default();
        entry.add_sample(delta_stats, delta_elapsed, tt_size, node_table_size);
        self.prev_stats = current_stats;
        self.prev_elapsed = elapsed;
        self.last_tt_size = tt_size;
        self.last_node_table_size = node_table_size;
    }

    fn on_found(&mut self, _depth: usize, solver: &mut ParallelSolver) -> Option<()> {
        solver.get_best_move()?;
        *self.total_elapsed_secs += self.prev_elapsed;
        self.total_stats.add_assign(&self.prev_stats);
        *self.total_tt_size = (*self.total_tt_size).saturating_add(self.last_tt_size);
        *self.total_node_table_size =
            (*self.total_node_table_size).saturating_add(self.last_node_table_size);
        Some(())
    }
}
struct BestMoveDeepening {
    verbose: bool,
}
impl IterativeDeepeningHooks<(Option<(usize, usize)>, TranspositionTable, NodeTable)>
    for BestMoveDeepening
{
    fn on_stop(
        &mut self,
        solver: &mut ParallelSolver,
    ) -> (Option<(usize, usize)>, TranspositionTable, NodeTable) {
        (None, solver.get_tt(), solver.get_node_table())
    }

    fn before_solve(&mut self, depth: usize, _solver: &mut ParallelSolver) {
        if self.verbose {
            println!("尝试搜索深度 D={depth}", depth = format_sci_usize(depth));
        }
    }

    fn solve(&mut self, solver: &mut ParallelSolver) -> bool {
        solver.solve(self.verbose)
    }

    fn on_found(
        &mut self,
        _depth: usize,
        solver: &mut ParallelSolver,
    ) -> (Option<(usize, usize)>, TranspositionTable, NodeTable) {
        let best_move = solver.get_best_move();
        if self.verbose {
            let path_len = format_sci_u64(solver.root_win_len());
            let best_move_display = best_move.map_or_else(
                || "None".to_string(),
                |(x, y)| format!("({}, {})", format_sci_usize(x), format_sci_usize(y)),
            );
            println!("在 {path_len} 步内找到路径，最佳首步: {best_move_display}");
        }
        (best_move, solver.get_tt(), solver.get_node_table())
    }
}
impl ParallelSolver {
    #[must_use]
    pub fn new(
        initial_board: Vec<u8>,
        board_size: usize,
        win_len: usize,
        depth_limit: Option<usize>,
        num_threads: usize,
        evaluation: EvaluationConfig,
    ) -> Self {
        let params = SearchParams::new(board_size, win_len, num_threads, evaluation);
        Self::with_tt(initial_board, params, depth_limit, None, None)
    }

    #[must_use]
    pub fn with_tt(
        initial_board: Vec<u8>,
        params: SearchParams,
        depth_limit: Option<usize>,
        existing_tt: Option<TranspositionTable>,
        existing_node_table: Option<NodeTable>,
    ) -> Self {
        Self::with_tt_and_stop(
            initial_board,
            params,
            depth_limit,
            &Arc::new(AtomicBool::new(false)),
            existing_tt,
            existing_node_table,
        )
    }

    #[must_use]
    pub fn with_tt_and_stop(
        initial_board: Vec<u8>,
        params: SearchParams,
        depth_limit: Option<usize>,
        stop_flag: &Arc<AtomicBool>,
        existing_tt: Option<TranspositionTable>,
        existing_node_table: Option<NodeTable>,
    ) -> Self {
        alloc_stats::reset_alloc_timing_ns();
        let _alloc_guard = alloc_stats::AllocTrackingGuard::new();
        let hasher = Arc::new(ZobristHasher::new(params.board_size));
        let game_state = GomokuGameState::new(
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
        Self {
            tree,
            base_game_state: game_state,
            num_threads: params.num_threads,
            board_size: params.board_size,
            win_len: params.win_len,
        }
    }

    fn clone_game_state(&self) -> GomokuGameState {
        self.base_game_state.clone()
    }

    fn current_turn(&self) -> usize {
        self.base_game_state
            .position
            .board
            .iter()
            .fold(0usize, |count, &cell| count + usize::from(cell == 2))
    }

    pub fn increase_depth_limit(&mut self, new_limit: usize) {
        if let Some(tree) = Arc::get_mut(&mut self.tree) {
            tree.increase_depth_limit(new_limit);
        } else {
            eprintln!("无法取得 SharedTree 的可变引用，跳过深度调整");
        }
    }
}

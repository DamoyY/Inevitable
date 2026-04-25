use super::super::{SharedTree, TreeStatsSnapshot, WorkerPool};
use crate::{config::EvaluationWeights, game_state::GameState};
use alloc::sync::Arc;
pub struct ParallelSolver {
    pub(crate) tree: Arc<SharedTree>,
    pub(crate) worker_pool: WorkerPool,
    pub(crate) base_game_state: GameState,
    pub(crate) board_size: usize,
    pub(crate) win_len: usize,
}
#[derive(Clone, Copy)]
pub struct SearchParams {
    pub board_size: usize,
    pub win_len: usize,
    pub num_threads: usize,
    pub evaluation: EvaluationWeights,
}
impl SearchParams {
    #[inline]
    #[must_use]
    pub const fn new(
        board_size: usize,
        win_len: usize,
        num_threads: usize,
        evaluation: EvaluationWeights,
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

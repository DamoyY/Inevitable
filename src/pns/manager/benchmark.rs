use super::super::{TreeStatsSnapshot, stats_def::to_f64};
use super::{BenchmarkResult, SearchParams};
use crate::checked;
use alloc::{collections::BTreeMap, sync::Arc};
use core::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;
pub(super) fn benchmark_next_move(
    initial_board: &[u8],
    params: SearchParams,
    runs: usize,
    stop_flag: &Arc<AtomicBool>,
) -> Option<BenchmarkResult> {
    if runs == 0 {
        return None;
    }
    let base_board = initial_board.to_vec();
    let mut per_depth: BTreeMap<usize, super::deepening::DepthAccumulator> = BTreeMap::new();
    let mut total_stats = TreeStatsSnapshot::default();
    let mut total_elapsed_secs = 0.0_f64;
    let mut total_tt_size: u64 = 0;
    let mut total_node_table_size: u64 = 0;
    for _ in 0..runs {
        if stop_flag.load(Ordering::Acquire) {
            return None;
        }
        let depth = 1_usize;
        let mut solver = super::setup::with_tt_and_stop(
            base_board.clone(),
            params,
            Some(depth),
            stop_flag,
            None,
            None,
        );
        let mut hooks = super::deepening::BenchmarkDeepening {
            start: Instant::now(),
            per_depth: &mut per_depth,
            prev_stats: TreeStatsSnapshot::default(),
            prev_elapsed: 0.0,
            last_tt_size: 0,
            last_node_table_size: 0,
            total_stats: &mut total_stats,
            total_elapsed_secs: &mut total_elapsed_secs,
            total_tt_size: &mut total_tt_size,
            total_node_table_size: &mut total_node_table_size,
        };
        super::solve::run_iterative_deepening(&mut solver, stop_flag, depth, &mut hooks)?;
    }
    let runs_count = checked::usize_to_u64(runs, "ParallelSolver::benchmark_next_move::runs_count");
    let stats = total_stats.div_round(runs_count);
    let elapsed_secs = total_elapsed_secs / to_f64(runs_count);
    let tt_size = checked::u64_to_usize(
        checked::rounded_div_u64(
            total_tt_size,
            runs_count,
            "ParallelSolver::benchmark_next_move::tt_size",
        ),
        "ParallelSolver::benchmark_next_move::tt_size",
    );
    let node_table_size = checked::u64_to_usize(
        checked::rounded_div_u64(
            total_node_table_size,
            runs_count,
            "ParallelSolver::benchmark_next_move::node_table_size",
        ),
        "ParallelSolver::benchmark_next_move::node_table_size",
    );
    super::deepening::write_benchmark_logs(per_depth);
    Some(BenchmarkResult {
        elapsed_secs,
        stats,
        tt_size,
        node_table_size,
    })
}

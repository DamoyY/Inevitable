use super::super::{NodeTable, TranspositionTable, TreeStatsSnapshot};
use super::ParallelSolver;
use super::logging::{format_sci_u64, format_sci_usize, write_csv_log_snapshot};
use crate::checked;
use alloc::{collections::BTreeMap, string::String};
use std::time::Instant;
#[derive(Default)]
pub(super) struct DepthAccumulator {
    total_stats: TreeStatsSnapshot,
    total_elapsed_secs: f64,
    total_tt_size: u64,
    total_node_table_size: u64,
    count: u64,
}
impl DepthAccumulator {
    pub(super) fn add_sample(
        &mut self,
        stats: TreeStatsSnapshot,
        elapsed_secs: f64,
        tt_size: u64,
        node_table_size: u64,
    ) {
        self.total_stats.add_assign(&stats);
        self.total_elapsed_secs += elapsed_secs;
        self.total_tt_size = checked::add_u64(
            self.total_tt_size,
            tt_size,
            "DepthAccumulator::total_tt_size",
        );
        self.total_node_table_size = checked::add_u64(
            self.total_node_table_size,
            node_table_size,
            "DepthAccumulator::total_node_table_size",
        );
        self.count = checked::add_u64(self.count, 1_u64, "DepthAccumulator::count");
    }
    fn average(&self) -> (TreeStatsSnapshot, f64, usize, usize) {
        if self.count == 0_u64 {
            eprintln!("DepthAccumulator::average 的样本数不能为 0。");
            panic!("DepthAccumulator::average 的样本数不能为 0");
        }
        let divisor = super::super::stats_def::to_f64(self.count);
        let stats = self.total_stats.div_round(self.count);
        let elapsed_secs = self.total_elapsed_secs / divisor;
        let tt_size = checked::u64_to_usize(
            checked::rounded_div_u64(self.total_tt_size, self.count, "DepthAccumulator::tt_size"),
            "DepthAccumulator::tt_size",
        );
        let node_table_size = checked::u64_to_usize(
            checked::rounded_div_u64(
                self.total_node_table_size,
                self.count,
                "DepthAccumulator::node_table_size",
            ),
            "DepthAccumulator::node_table_size",
        );
        (stats, elapsed_secs, tt_size, node_table_size)
    }
}
pub(super) fn write_benchmark_logs(per_depth: BTreeMap<usize, DepthAccumulator>) {
    for (depth, acc) in per_depth {
        if acc.count == 0 {
            continue;
        }
        let (stats, elapsed_secs, tt_size, node_table_size) = acc.average();
        write_csv_log_snapshot(
            1,
            elapsed_secs,
            stats,
            tt_size,
            node_table_size,
            Some(depth),
        );
    }
}
pub(super) trait IterativeDeepeningHooks<R> {
    fn on_stop(&mut self, solver: &mut ParallelSolver) -> R;
    fn before_solve(&mut self, _depth: usize, _solver: &mut ParallelSolver) {}
    fn solve(&mut self, solver: &mut ParallelSolver) -> bool;
    fn after_solve(&mut self, _depth: usize, _solver: &mut ParallelSolver, _found: bool) {}
    fn on_found(&mut self, _depth: usize, solver: &mut ParallelSolver) -> R;
}
pub(super) struct BenchmarkDeepening<'benchmark> {
    pub start: Instant,
    pub per_depth: &'benchmark mut BTreeMap<usize, DepthAccumulator>,
    pub prev_stats: TreeStatsSnapshot,
    pub prev_elapsed: f64,
    pub last_tt_size: u64,
    pub last_node_table_size: u64,
    pub total_stats: &'benchmark mut TreeStatsSnapshot,
    pub total_elapsed_secs: &'benchmark mut f64,
    pub total_tt_size: &'benchmark mut u64,
    pub total_node_table_size: &'benchmark mut u64,
}
impl IterativeDeepeningHooks<Option<()>> for BenchmarkDeepening<'_> {
    fn on_stop(&mut self, _solver: &mut ParallelSolver) -> Option<()> {
        None
    }
    fn before_solve(&mut self, _depth: usize, _solver: &mut ParallelSolver) {}
    fn solve(&mut self, solver: &mut ParallelSolver) -> bool {
        solver.solve(false)
    }
    fn after_solve(&mut self, depth: usize, solver: &mut ParallelSolver, _found: bool) {
        let elapsed = self.start.elapsed().as_secs_f64();
        let current_stats = solver.tree.stats_snapshot();
        let delta_stats = current_stats.delta_since(&self.prev_stats);
        let delta_elapsed = (elapsed - self.prev_elapsed).max(0.0_f64);
        let tt_size =
            checked::usize_to_u64(solver.tree.get_tt_size(), "BenchmarkDeepening::tt_size");
        let node_table_size = checked::usize_to_u64(
            solver.tree.get_node_table_size(),
            "BenchmarkDeepening::node_table_size",
        );
        self.per_depth.entry(depth).or_default().add_sample(
            delta_stats,
            delta_elapsed,
            tt_size,
            node_table_size,
        );
        self.prev_stats = current_stats;
        self.prev_elapsed = elapsed;
        self.last_tt_size = tt_size;
        self.last_node_table_size = node_table_size;
    }
    fn on_found(&mut self, _depth: usize, solver: &mut ParallelSolver) -> Option<()> {
        solver.get_best_move()?;
        *self.total_elapsed_secs += self.prev_elapsed;
        self.total_stats.add_assign(&self.prev_stats);
        *self.total_tt_size = checked::add_u64(
            *self.total_tt_size,
            self.last_tt_size,
            "BenchmarkDeepening::total_tt_size",
        );
        *self.total_node_table_size = checked::add_u64(
            *self.total_node_table_size,
            self.last_node_table_size,
            "BenchmarkDeepening::total_node_table_size",
        );
        Some(())
    }
}
pub(super) struct BestMoveDeepening {
    pub verbose: bool,
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
    fn after_solve(&mut self, _depth: usize, _solver: &mut ParallelSolver, _found: bool) {}
    fn on_found(
        &mut self,
        _depth: usize,
        solver: &mut ParallelSolver,
    ) -> (Option<(usize, usize)>, TranspositionTable, NodeTable) {
        let best_move = solver.get_best_move();
        if self.verbose {
            let path_len = format_sci_u64(solver.root_win_len());
            let best_move_display = best_move.map_or_else(
                || String::from("None"),
                |(x, y)| format!("({}, {})", format_sci_usize(x), format_sci_usize(y)),
            );
            println!("在 {path_len} 步内找到路径，最佳首步: {best_move_display}");
        }
        (best_move, solver.get_tt(), solver.get_node_table())
    }
}

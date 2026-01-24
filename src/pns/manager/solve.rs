use std::{
    collections::BTreeMap,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::Instant,
};

use super::{
    super::TreeStatsSnapshot,
    BenchmarkDeepening, BenchmarkResult, BestMoveDeepening, DepthAccumulator,
    IterativeDeepeningHooks, ParallelSolver,
    logging::{format_sci_u64, write_csv_log},
    write_benchmark_logs,
};
use crate::{
    alloc_stats::AllocTrackingGuard,
    pns::{NodeTable, SharedTree, TranspositionTable, Worker, context::ThreadLocalContext},
};
impl ParallelSolver {
    #[must_use]
    pub fn solve(&self, verbose: bool) -> bool {
        let start_time = Instant::now();
        let _alloc_guard = AllocTrackingGuard::new();
        let tree = Arc::clone(&self.tree);
        if tree.stop_requested() {
            return false;
        }
        if tree.root.is_terminal() {
            if verbose {
                println!(
                    "根节点已是终端状态: PN={}, DN={}",
                    format_sci_u64(tree.root.get_pn()),
                    format_sci_u64(tree.root.get_dn())
                );
            }
            if tree.root.get_pn() == 0 && !tree.root.is_expanded() {
                let mut ctx = ThreadLocalContext::new(self.clone_game_state(), 0);
                tree.expand_node(&tree.root, &mut ctx);
                tree.update_node_pdn(&tree.root);
            }
            return tree.root.get_pn() == 0;
        }
        let handles = self.spawn_workers(&tree);
        Self::wait_for_workers(handles);
        let elapsed = start_time.elapsed().as_secs_f64();
        if verbose {
            write_csv_log(&self.tree, self.current_turn(), elapsed);
        }
        self.tree.root.get_pn() == 0
    }

    fn spawn_workers(&self, tree: &Arc<SharedTree>) -> Vec<thread::JoinHandle<()>> {
        (0..self.num_threads)
            .map(|thread_id| {
                let tree = Arc::clone(tree);
                let game_state = self.clone_game_state();
                thread::spawn(move || {
                    let _alloc_guard = AllocTrackingGuard::new();
                    let ctx = ThreadLocalContext::new(game_state, thread_id);
                    let mut worker = Worker::new(tree, ctx);
                    worker.run();
                })
            })
            .collect()
    }

    fn wait_for_workers(handles: Vec<thread::JoinHandle<()>>) {
        for handle in handles {
            let _ = handle.join();
        }
    }

    fn run_iterative_deepening<R, H>(
        solver: &mut Self,
        stop_flag: &Arc<AtomicBool>,
        mut depth: usize,
        hooks: &mut H,
    ) -> R
    where
        H: IterativeDeepeningHooks<R>,
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
            depth += 1;
            if stop_flag.load(Ordering::Acquire) {
                return hooks.on_stop(solver);
            }
            solver.increase_depth_limit(depth);
        }
    }

    pub fn benchmark_next_move(
        initial_board: &[u8],
        params: super::SearchParams,
        runs: usize,
        stop_flag: &Arc<AtomicBool>,
    ) -> Option<BenchmarkResult> {
        if runs == 0 {
            return None;
        }
        let base_board = initial_board.to_vec();
        let mut per_depth: BTreeMap<usize, DepthAccumulator> = BTreeMap::new();
        let mut total_stats = TreeStatsSnapshot::default();
        let mut total_elapsed_secs = 0.0;
        let mut total_tt_size: u64 = 0;
        let mut total_node_table_size: u64 = 0;
        for _ in 0..runs {
            if stop_flag.load(Ordering::Acquire) {
                return None;
            }
            let start = Instant::now();
            let depth = 1usize;
            let mut solver = Self::with_tt_and_stop(
                base_board.clone(),
                params,
                Some(depth),
                stop_flag,
                None,
                None,
            );
            let mut hooks = BenchmarkDeepening {
                start,
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
            Self::run_iterative_deepening(&mut solver, stop_flag, depth, &mut hooks)?;
        }
        let runs_count = runs as u64;
        let runs_divisor = f64::from(u32::try_from(runs).unwrap_or(u32::MAX));
        write_benchmark_logs(per_depth);
        let stats = total_stats.div_round(runs_count);
        let elapsed_secs = total_elapsed_secs / runs_divisor;
        let tt_size_u64 = (total_tt_size.saturating_add(runs_count / 2)) / runs_count;
        let node_table_size_u64 =
            (total_node_table_size.saturating_add(runs_count / 2)) / runs_count;
        let tt_size = usize::try_from(tt_size_u64).unwrap_or(usize::MAX);
        let node_table_size = usize::try_from(node_table_size_u64).unwrap_or(usize::MAX);
        Some(BenchmarkResult {
            elapsed_secs,
            stats,
            tt_size,
            node_table_size,
        })
    }

    #[must_use]
    pub fn find_best_move_iterative_deepening(
        initial_board: Vec<u8>,
        board_size: usize,
        win_len: usize,
        num_threads: usize,
        evaluation: crate::config::EvaluationConfig,
        verbose: bool,
    ) -> Option<(usize, usize)> {
        let params = super::SearchParams::new(board_size, win_len, num_threads, evaluation);
        Self::find_best_move_with_tt(initial_board, params, verbose, None, None).0
    }

    #[must_use]
    pub fn find_best_move_with_tt(
        initial_board: Vec<u8>,
        params: super::SearchParams,
        verbose: bool,
        existing_tt: Option<TranspositionTable>,
        existing_node_table: Option<NodeTable>,
    ) -> (Option<(usize, usize)>, TranspositionTable, NodeTable) {
        Self::find_best_move_with_tt_and_stop(
            initial_board,
            params,
            verbose,
            &Arc::new(AtomicBool::new(false)),
            existing_tt,
            existing_node_table,
        )
    }

    #[must_use]
    pub fn find_best_move_with_tt_and_stop(
        initial_board: Vec<u8>,
        params: super::SearchParams,
        verbose: bool,
        stop_flag: &Arc<AtomicBool>,
        existing_tt: Option<TranspositionTable>,
        existing_node_table: Option<NodeTable>,
    ) -> (Option<(usize, usize)>, TranspositionTable, NodeTable) {
        let depth = 1usize;
        let mut solver = Self::with_tt_and_stop(
            initial_board,
            params,
            Some(depth),
            stop_flag,
            existing_tt,
            existing_node_table,
        );
        let mut hooks = BestMoveDeepening { verbose };
        Self::run_iterative_deepening(&mut solver, stop_flag, depth, &mut hooks)
    }

    #[must_use]
    pub fn get_tt(&self) -> TranspositionTable {
        self.tree.get_tt()
    }

    #[must_use]
    pub fn get_node_table(&self) -> NodeTable {
        self.tree.get_node_table()
    }

    #[must_use]
    pub fn get_best_move(&self) -> Option<(usize, usize)> {
        let root = &self.tree.root;
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
            .filter(|c| {
                c.node.get_pn() == 0 && 1u64.saturating_add(c.node.get_win_len()) == root_win_len
            })
            .collect();
        if winning_children.is_empty() {
            children
                .iter()
                .filter(|c| c.node.get_pn() == 0)
                .min_by_key(|c| (c.node.get_win_len(), c.mov))
                .map(|c| c.mov)
        } else {
            winning_children
                .iter()
                .min_by_key(|c| (c.node.get_win_len(), c.mov))
                .map(|c| c.mov)
        }
    }

    #[must_use]
    pub fn root_pn(&self) -> u64 {
        self.tree.root.get_pn()
    }

    #[must_use]
    pub fn root_dn(&self) -> u64 {
        self.tree.root.get_dn()
    }

    #[must_use]
    pub fn root_player(&self) -> u8 {
        self.tree.root.player
    }

    #[must_use]
    pub fn root_win_len(&self) -> u64 {
        self.tree.root.get_win_len()
    }

    #[must_use]
    pub const fn game_state(&self) -> &crate::game_state::GomokuGameState {
        &self.base_game_state
    }

    #[must_use]
    pub const fn board_size(&self) -> usize {
        self.board_size
    }

    #[must_use]
    pub const fn win_len(&self) -> usize {
        self.win_len
    }
}

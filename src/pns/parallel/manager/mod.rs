use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use super::{
    context::ThreadLocalContext,
    shared_tree::{NodeTable, SharedTree, TranspositionTable},
};
use crate::{alloc_stats, game_state::{GomokuGameState, ZobristHasher}};
mod logging;
mod metrics;
mod solve;
use metrics::{format_sci_u64, format_sci_usize};
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
}

impl SearchParams {
    #[must_use]
    pub const fn new(
        board_size: usize,
        win_len: usize,
        num_threads: usize,
    ) -> Self {
        Self {
            board_size,
            win_len,
            num_threads,
        }
    }
}

impl ParallelSolver {
    #[must_use]
    pub fn new(
        initial_board: Vec<Vec<u8>>,
        board_size: usize,
        win_len: usize,
        depth_limit: Option<usize>,
        num_threads: usize,
    ) -> Self {
        Self::with_tt(
            initial_board,
            board_size,
            win_len,
            depth_limit,
            num_threads,
            None,
            None,
        )
    }

    #[must_use]
    pub fn with_tt(
        initial_board: Vec<Vec<u8>>,
        board_size: usize,
        win_len: usize,
        depth_limit: Option<usize>,
        num_threads: usize,
        existing_tt: Option<TranspositionTable>,
        existing_node_table: Option<NodeTable>,
    ) -> Self {
        Self::with_tt_and_stop(
            initial_board,
            SearchParams::new(board_size, win_len, num_threads),
            depth_limit,
            &Arc::new(AtomicBool::new(false)),
            existing_tt,
            existing_node_table,
        )
    }

    #[must_use]
    pub fn with_tt_and_stop(
        initial_board: Vec<Vec<u8>>,
        params: SearchParams,
        depth_limit: Option<usize>,
        stop_flag: &Arc<AtomicBool>,
        existing_tt: Option<TranspositionTable>,
        existing_node_table: Option<NodeTable>,
    ) -> Self {
        alloc_stats::reset_alloc_free_time_ns();
        let _alloc_guard = alloc_stats::AllocTrackingGuard::new();
        let hasher = Arc::new(ZobristHasher::new(params.board_size));
        let game_state = GomokuGameState::new(initial_board, hasher, 1, params.win_len);
        let root_hash = game_state.get_canonical_hash();
        let root_pos_hash = game_state.get_hash();

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

    pub fn increase_depth_limit(&mut self, new_limit: usize) {
        if let Some(tree) = Arc::get_mut(&mut self.tree) {
            tree.increase_depth_limit(new_limit);
        } else {
            eprintln!("无法取得 SharedTree 的可变引用，跳过深度调整");
        }
    }

    #[must_use]
    pub fn find_best_move_iterative_deepening(
        initial_board: Vec<Vec<u8>>,
        board_size: usize,
        win_len: usize,
        num_threads: usize,
        verbose: bool,
    ) -> Option<(usize, usize)> {
        Self::find_best_move_with_tt(
            initial_board,
            board_size,
            win_len,
            num_threads,
            verbose,
            None,
            None,
        )
        .0
    }

    #[must_use]
    pub fn find_best_move_with_tt(
        initial_board: Vec<Vec<u8>>,
        board_size: usize,
        win_len: usize,
        num_threads: usize,
        verbose: bool,
        existing_tt: Option<TranspositionTable>,
        existing_node_table: Option<NodeTable>,
    ) -> (Option<(usize, usize)>, TranspositionTable, NodeTable) {
        Self::find_best_move_with_tt_and_stop(
            initial_board,
            SearchParams::new(board_size, win_len, num_threads),
            verbose,
            &Arc::new(AtomicBool::new(false)),
            existing_tt,
            existing_node_table,
        )
    }

    #[must_use]
    pub fn find_best_move_with_tt_and_stop(
        initial_board: Vec<Vec<u8>>,
        params: SearchParams,
        verbose: bool,
        stop_flag: &Arc<AtomicBool>,
        existing_tt: Option<TranspositionTable>,
        existing_node_table: Option<NodeTable>,
    ) -> (Option<(usize, usize)>, TranspositionTable, NodeTable) {
        let mut depth = 1usize;
        let mut solver = Self::with_tt_and_stop(
            initial_board,
            params,
            Some(depth),
            stop_flag,
            existing_tt,
            existing_node_table,
        );
        loop {
            if stop_flag.load(Ordering::Acquire) {
                return (None, solver.get_tt(), solver.get_node_table());
            }
            if verbose {
                println!("尝试搜索深度 D={depth}", depth = format_sci_usize(depth));
            }
            let found = solver.solve(verbose);
            if stop_flag.load(Ordering::Acquire) || solver.tree.stop_requested() {
                return (None, solver.get_tt(), solver.get_node_table());
            }
            if found {
                let best_move = solver.get_best_move();
                if verbose {
                    let path_len = format_sci_u64(solver.root_win_len());
                    let best_move_display = best_move.map_or_else(
                        || "None".to_string(),
                        |(x, y)| format!("({}, {})", format_sci_usize(x), format_sci_usize(y)),
                    );
                    println!("在 {path_len} 步内找到路径，最佳首步: {best_move_display}");
                }
                return (best_move, solver.get_tt(), solver.get_node_table());
            }
            depth += 1;
            if stop_flag.load(Ordering::Acquire) {
                return (None, solver.get_tt(), solver.get_node_table());
            }
            solver.increase_depth_limit(depth);
        }
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
        let children = {
            let children_guard = root.children.read();
            children_guard.as_ref().cloned()?
        };
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
    pub const fn game_state(&self) -> &GomokuGameState {
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
impl Clone for GomokuGameState {
    fn clone(&self) -> Self {
        let hasher = Arc::clone(&self.hasher);
        let mut state = Self::new(self.board.clone(), hasher, 1, self.win_len);
        state.hash = self.hash;
        state
    }
}

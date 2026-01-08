use std::{collections::HashMap, sync::Arc};

use crate::game_state::{GomokuGameState, ZobristHasher};
use crate::pns::{PNSNode, TTEntry};

mod evaluation;
mod expansion;
mod update;

pub struct PNSSolver {
    pub game_state: GomokuGameState,
    pub(crate) transposition_table: HashMap<(u64, u8), TTEntry>,
    pub nodes: Vec<PNSNode>,
    pub root: usize,
    pub(crate) iterations: u64,
    pub(crate) nodes_processed: u64,
    pub(crate) tt_lookups: u64,
    pub(crate) tt_hits: u64,
    pub(crate) tt_stores: u64,
    pub(crate) eval_calls: u64,
    pub(crate) eval_time_ns: u64,
    pub(crate) expand_time_ns: u64,
    pub(crate) movegen_time_ns: u64,
    pub(crate) children_generated: u64,
    pub(crate) depth_cutoffs: u64,
    pub(crate) early_cutoffs: u64,
    pub(crate) best_move: Option<(usize, usize)>,
    pub(crate) depth_limit: Option<usize>,
}

impl PNSSolver {
    pub fn new(
        initial_board: Vec<Vec<u8>>,
        board_size: usize,
        win_len: usize,
        depth_limit: Option<usize>,
    ) -> Self {
        let hasher = Arc::new(ZobristHasher::new(board_size));
        let game_state = GomokuGameState::new(initial_board, hasher, 1, win_len);

        let mut solver = Self {
            game_state,
            transposition_table: HashMap::new(),
            nodes: Vec::new(),
            root: 0,
            iterations: 0,
            nodes_processed: 0,
            tt_lookups: 0,
            tt_hits: 0,
            tt_stores: 0,
            eval_calls: 0,
            eval_time_ns: 0,
            expand_time_ns: 0,
            movegen_time_ns: 0,
            children_generated: 0,
            depth_cutoffs: 0,
            early_cutoffs: 0,
            best_move: None,
            depth_limit,
        };

        let root_node = PNSNode::new(1, None, None, 0);
        solver.nodes.push(root_node);
        solver.root = 0;
        solver.nodes[solver.root].hash = solver.game_state.get_canonical_hash();
        solver.evaluate_and_set_proof_numbers(solver.root);

        solver
    }

    pub fn root_pn(&self) -> u64 {
        self.nodes[self.root].pn
    }

    pub fn root_dn(&self) -> u64 {
        self.nodes[self.root].dn
    }

    pub fn root_player(&self) -> u8 {
        self.nodes[self.root].player
    }
}

pub(super) fn duration_to_ns(duration: std::time::Duration) -> u64 {
    let nanos = duration.as_nanos();
    if nanos > u128::from(u64::MAX) {
        u64::MAX
    } else {
        nanos as u64
    }
}

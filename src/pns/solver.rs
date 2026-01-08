use std::{collections::HashMap, sync::Arc, time::Instant};

use crate::{
    game_state::{GomokuGameState, ZobristHasher},
    pns::{PNSNode, TTEntry},
};

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

    pub(crate) fn evaluate_and_set_proof_numbers(&mut self, node_idx: usize) {
        let start = Instant::now();
        self.eval_calls = self.eval_calls.saturating_add(1);
        let node = &self.nodes[node_idx];
        let tt_key = (node.hash, node.player);

        self.tt_lookups = self.tt_lookups.saturating_add(1);
        if let Some(entry) = self.transposition_table.get(&tt_key) {
            self.tt_hits = self.tt_hits.saturating_add(1);
            self.nodes[node_idx].pn = entry.pn;
            self.nodes[node_idx].dn = entry.dn;
            self.nodes[node_idx].win_len = entry.win_len;
            self.nodes[node_idx].is_expanded = true;
            self.eval_time_ns = self
                .eval_time_ns
                .saturating_add(duration_to_ns(start.elapsed()));
            return;
        }

        let node = &self.nodes[node_idx];
        let mut p1_wins = false;
        let mut p2_wins = false;

        if node.mov.is_some() {
            let opponent = 3 - node.player;
            if self.game_state.check_win(opponent) {
                if opponent == 1 {
                    p1_wins = true;
                } else {
                    p2_wins = true;
                }
            }
        } else {
            if self.game_state.check_win(1) {
                p1_wins = true;
            }
            if self.game_state.check_win(2) {
                p2_wins = true;
            }
        }

        if p1_wins {
            self.nodes[node_idx].pn = 0;
            self.nodes[node_idx].dn = u64::MAX;
            self.nodes[node_idx].win_len = 0;
            self.nodes[node_idx].is_expanded = true;
        } else if p2_wins {
            self.nodes[node_idx].pn = u64::MAX;
            self.nodes[node_idx].dn = 0;
            self.nodes[node_idx].is_expanded = true;
        } else if let Some(limit) = self.depth_limit
            && self.nodes[node_idx].depth >= limit
        {
            self.depth_cutoffs = self.depth_cutoffs.saturating_add(1);
            self.nodes[node_idx].pn = u64::MAX;
            self.nodes[node_idx].dn = 0;
            self.nodes[node_idx].is_expanded = true;
            self.nodes[node_idx].is_depth_limited = true;
        }

        self.eval_time_ns = self
            .eval_time_ns
            .saturating_add(duration_to_ns(start.elapsed()));
    }

    pub(crate) fn expand_node(&mut self, node_idx: usize) {
        let node = &self.nodes[node_idx];
        if node.is_expanded || node.pn == 0 || node.dn == 0 {
            return;
        }

        let expand_start = Instant::now();
        self.nodes_processed += 1;

        if let Some(limit) = self.depth_limit
            && self.nodes[node_idx].depth >= limit
        {
            self.depth_cutoffs = self.depth_cutoffs.saturating_add(1);
            self.nodes[node_idx].is_expanded = true;
            self.nodes[node_idx].is_depth_limited = true;
            self.nodes[node_idx].pn = 1;
            self.nodes[node_idx].dn = 1;
            self.nodes[node_idx].win_len = u64::MAX;
            self.nodes[node_idx].children.clear();
            self.expand_time_ns = self
                .expand_time_ns
                .saturating_add(duration_to_ns(expand_start.elapsed()));
            return;
        }

        self.nodes[node_idx].is_expanded = true;
        let player = self.nodes[node_idx].player;
        let depth = self.nodes[node_idx].depth;
        let is_or_node = self.nodes[node_idx].is_or_node();

        let movegen_start = Instant::now();
        let legal_moves = self.game_state.get_legal_moves(player);
        let legal_moves_len = legal_moves.len();
        self.movegen_time_ns = self
            .movegen_time_ns
            .saturating_add(duration_to_ns(movegen_start.elapsed()));
        let mut child_indices = Vec::new();

        let mut generated = 0usize;
        for mov in legal_moves {
            self.game_state.make_move(mov, player);

            let child_idx = self.nodes.len();
            let mut child_node = PNSNode::new(3 - player, Some(node_idx), Some(mov), depth + 1);
            child_node.hash = self.game_state.get_canonical_hash();
            self.nodes.push(child_node);

            self.evaluate_and_set_proof_numbers(child_idx);

            self.game_state.undo_move(mov);

            child_indices.push(child_idx);
            generated += 1;
            self.children_generated = self.children_generated.saturating_add(1);

            let child = &self.nodes[child_idx];
            if is_or_node && child.pn == 0 {
                break;
            }
            if !is_or_node && child.dn == 0 {
                break;
            }
        }

        self.nodes[node_idx].children = child_indices;
        if generated < legal_moves_len {
            self.early_cutoffs = self.early_cutoffs.saturating_add(1);
        }
        self.expand_time_ns = self
            .expand_time_ns
            .saturating_add(duration_to_ns(expand_start.elapsed()));
    }

    pub(crate) fn update_node_pdn(&mut self, node_idx: usize) {
        let node = &self.nodes[node_idx];

        if node.is_depth_limited && node.children.is_empty() {
            self.nodes[node_idx].pn = 1;
            self.nodes[node_idx].dn = 1;
            self.nodes[node_idx].win_len = u64::MAX;
            return;
        }

        if !node.is_expanded {
            return;
        }

        if node.children.is_empty() {
            if node.is_or_node() {
                self.nodes[node_idx].pn = u64::MAX;
                self.nodes[node_idx].dn = 0;
                self.nodes[node_idx].win_len = u64::MAX;
            } else {
                self.nodes[node_idx].pn = 0;
                self.nodes[node_idx].dn = u64::MAX;
                self.nodes[node_idx].win_len = 0;
            }
            return;
        }

        let children = self.nodes[node_idx].children.clone();
        let is_or_node = self.nodes[node_idx].is_or_node();

        if is_or_node {
            let min_pn = children
                .iter()
                .map(|&idx| self.nodes[idx].pn)
                .min()
                .unwrap_or(u64::MAX);
            let sum_dn = children
                .iter()
                .map(|&idx| self.nodes[idx].dn)
                .fold(0u64, |acc, x| acc.saturating_add(x));

            self.nodes[node_idx].pn = min_pn;
            self.nodes[node_idx].dn = sum_dn;

            let proven_children: Vec<usize> = children
                .iter()
                .filter(|&&idx| self.nodes[idx].pn == 0)
                .copied()
                .collect();

            if !proven_children.is_empty() {
                let min_win_len = proven_children
                    .iter()
                    .map(|&idx| self.nodes[idx].win_len)
                    .min()
                    .unwrap_or(u64::MAX);
                self.nodes[node_idx].win_len = 1u64.saturating_add(min_win_len);
            } else {
                self.nodes[node_idx].win_len = u64::MAX;
            }
        } else {
            let sum_pn = children
                .iter()
                .map(|&idx| self.nodes[idx].pn)
                .fold(0u64, |acc, x| acc.saturating_add(x));
            let min_dn = children
                .iter()
                .map(|&idx| self.nodes[idx].dn)
                .min()
                .unwrap_or(u64::MAX);

            self.nodes[node_idx].pn = sum_pn;
            self.nodes[node_idx].dn = min_dn;

            if min_dn == 0 {
                self.nodes[node_idx].win_len = u64::MAX;
            } else if children.iter().all(|&idx| self.nodes[idx].pn == 0) {
                let max_win_len = children
                    .iter()
                    .map(|&idx| self.nodes[idx].win_len)
                    .max()
                    .unwrap_or(0);
                self.nodes[node_idx].win_len = 1u64.saturating_add(max_win_len);
            } else {
                self.nodes[node_idx].win_len = u64::MAX;
            }
        }
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

fn duration_to_ns(duration: std::time::Duration) -> u64 {
    let nanos = duration.as_nanos();
    if nanos > u128::from(u64::MAX) {
        u64::MAX
    } else {
        nanos as u64
    }
}

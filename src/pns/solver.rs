use std::collections::HashMap;

use crate::game_state::{GomokuGameState, ZobristHasher};

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct TTEntry {
    pub pn: u64,
    pub dn: u64,
    pub win_len: u64,
}

pub struct PNSNode {
    pub player: u8,
    pub parent: Option<usize>,
    pub mov: Option<(usize, usize)>,
    pub children: Vec<usize>,
    pub pn: u64,
    pub dn: u64,
    pub is_expanded: bool,
    pub win_len: u64,
    pub depth: usize,
    pub is_depth_limited: bool,
    pub hash: u64,
}

impl PNSNode {
    pub fn new(
        player: u8,
        parent: Option<usize>,
        mov: Option<(usize, usize)>,
        depth: usize,
    ) -> Self {
        Self {
            player,
            parent,
            mov,
            children: Vec::new(),
            pn: 1,
            dn: 1,
            is_expanded: false,
            win_len: u64::MAX,
            depth,
            is_depth_limited: false,
            hash: 0,
        }
    }

    pub fn is_or_node(&self) -> bool {
        self.player == 1
    }
}

pub struct PNSSolver {
    pub game_state: GomokuGameState,
    pub(crate) transposition_table: HashMap<(u64, u8), TTEntry>,
    pub nodes: Vec<PNSNode>,
    pub root: usize,
    pub(crate) iterations: u64,
    pub(crate) nodes_processed: u64,
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
        let hasher = ZobristHasher::new(board_size);
        let game_state = GomokuGameState::new(initial_board, hasher, 1, win_len);

        let mut solver = Self {
            game_state,
            transposition_table: HashMap::new(),
            nodes: Vec::new(),
            root: 0,
            iterations: 0,
            nodes_processed: 0,
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
        let node = &self.nodes[node_idx];
        let tt_key = (node.hash, node.player);

        if let Some(entry) = self.transposition_table.get(&tt_key) {
            self.nodes[node_idx].pn = entry.pn;
            self.nodes[node_idx].dn = entry.dn;
            self.nodes[node_idx].win_len = entry.win_len;
            self.nodes[node_idx].is_expanded = true;
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
            self.nodes[node_idx].pn = u64::MAX;
            self.nodes[node_idx].dn = 0;
            self.nodes[node_idx].is_expanded = true;
            self.nodes[node_idx].is_depth_limited = true;
        }
    }

    pub(crate) fn expand_node(&mut self, node_idx: usize) {
        let node = &self.nodes[node_idx];
        if node.is_expanded || node.pn == 0 || node.dn == 0 {
            return;
        }

        self.nodes_processed += 1;

        if let Some(limit) = self.depth_limit
            && self.nodes[node_idx].depth >= limit
        {
            self.nodes[node_idx].is_expanded = true;
            self.nodes[node_idx].is_depth_limited = true;
            self.nodes[node_idx].pn = 1;
            self.nodes[node_idx].dn = 1;
            self.nodes[node_idx].win_len = u64::MAX;
            self.nodes[node_idx].children.clear();
            return;
        }

        self.nodes[node_idx].is_expanded = true;
        let player = self.nodes[node_idx].player;
        let depth = self.nodes[node_idx].depth;
        let is_or_node = self.nodes[node_idx].is_or_node();

        let legal_moves = self.game_state.get_legal_moves(player);
        let mut child_indices = Vec::new();

        for mov in legal_moves {
            self.game_state.make_move(mov, player);

            let child_idx = self.nodes.len();
            let mut child_node = PNSNode::new(3 - player, Some(node_idx), Some(mov), depth + 1);
            child_node.hash = self.game_state.get_canonical_hash();
            self.nodes.push(child_node);

            self.evaluate_and_set_proof_numbers(child_idx);

            self.game_state.undo_move(mov);

            child_indices.push(child_idx);

            let child = &self.nodes[child_idx];
            if is_or_node && child.pn == 0 {
                break;
            }
            if !is_or_node && child.dn == 0 {
                break;
            }
        }

        self.nodes[node_idx].children = child_indices;
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

use std::collections::{HashSet, VecDeque};

use super::solver::{PNSNode, PNSSolver};
use crate::game_state::{GomokuGameState, ZobristHasher};

impl PNSSolver {
    pub fn increase_depth_limit(&mut self, new_depth_limit: usize) {
        if let Some(current_limit) = self.depth_limit
            && new_depth_limit <= current_limit
        {
            return;
        }

        self.transposition_table.clear();
        self.depth_limit = Some(new_depth_limit);

        let mut visited = HashSet::new();
        let mut queue = VecDeque::new();
        queue.push_back(self.root);
        visited.insert(self.root);

        let mut nodes_to_reset = Vec::new();

        while let Some(node_idx) = queue.pop_front() {
            if self.nodes[node_idx].is_depth_limited {
                nodes_to_reset.push(node_idx);
            }
            for &child_idx in &self.nodes[node_idx].children.clone() {
                if !visited.contains(&child_idx) {
                    visited.insert(child_idx);
                    queue.push_back(child_idx);
                }
            }
        }

        let temp_board = self.game_state.board.clone();
        let board_size = self.game_state.board_size;
        let win_len = self.game_state.win_len;

        for node_idx in &nodes_to_reset {
            self.nodes[*node_idx].pn = 1;
            self.nodes[*node_idx].dn = 1;
            self.nodes[*node_idx].is_expanded = false;
            self.nodes[*node_idx].is_depth_limited = false;

            let mut path = Vec::new();
            let mut curr = *node_idx;
            while let Some(parent_idx) = self.nodes[curr].parent {
                let parent_player = self.nodes[parent_idx].player;
                let mov = self.nodes[curr].mov.unwrap();
                path.push((mov, parent_player));
                curr = parent_idx;
            }
            path.reverse();

            let hasher = ZobristHasher::new(board_size);
            let mut temp_game_state = GomokuGameState::new(temp_board.clone(), hasher, 1, win_len);

            for &(mov, player) in &path {
                temp_game_state.make_move(mov, player);
            }

            let node = &self.nodes[*node_idx];
            let tt_key = (node.hash, node.player);

            let mut p1_wins = false;
            let mut p2_wins = false;

            if node.mov.is_some() {
                let opponent = 3 - node.player;
                if temp_game_state.check_win(opponent) {
                    if opponent == 1 {
                        p1_wins = true;
                    } else {
                        p2_wins = true;
                    }
                }
            } else {
                if temp_game_state.check_win(1) {
                    p1_wins = true;
                }
                if temp_game_state.check_win(2) {
                    p2_wins = true;
                }
            }

            if let Some(entry) = self.transposition_table.get(&tt_key) {
                self.nodes[*node_idx].pn = entry.pn;
                self.nodes[*node_idx].dn = entry.dn;
                self.nodes[*node_idx].win_len = entry.win_len;
                self.nodes[*node_idx].is_expanded = true;
            } else if p1_wins {
                self.nodes[*node_idx].pn = 0;
                self.nodes[*node_idx].dn = u64::MAX;
                self.nodes[*node_idx].win_len = 0;
                self.nodes[*node_idx].is_expanded = true;
            } else if p2_wins {
                self.nodes[*node_idx].pn = u64::MAX;
                self.nodes[*node_idx].dn = 0;
                self.nodes[*node_idx].is_expanded = true;
            } else if let Some(limit) = self.depth_limit
                && self.nodes[*node_idx].depth >= limit
            {
                self.nodes[*node_idx].pn = u64::MAX;
                self.nodes[*node_idx].dn = 0;
                self.nodes[*node_idx].is_expanded = true;
                self.nodes[*node_idx].is_depth_limited = true;
            }
        }

        let mut parents_to_update: HashSet<usize> = nodes_to_reset
            .iter()
            .filter_map(|&idx| self.nodes[idx].parent)
            .collect();

        while !parents_to_update.is_empty() {
            let mut next_parents = HashSet::new();
            for p in &parents_to_update {
                let old_pn = self.nodes[*p].pn;
                let old_dn = self.nodes[*p].dn;
                self.update_node_pdn(*p);
                if (self.nodes[*p].pn != old_pn || self.nodes[*p].dn != old_dn)
                    && let Some(parent_idx) = self.nodes[*p].parent
                {
                    next_parents.insert(parent_idx);
                }
            }
            parents_to_update = next_parents;
        }
    }

    pub(crate) fn rebase_depth(&mut self, node_idx: usize, base_depth: usize) {
        let mut visited = HashSet::new();
        let mut queue = VecDeque::new();
        queue.push_back((node_idx, base_depth));
        visited.insert(node_idx);

        while let Some((curr, depth)) = queue.pop_front() {
            self.nodes[curr].depth = depth;
            for &child_idx in &self.nodes[curr].children.clone() {
                if !visited.contains(&child_idx) {
                    visited.insert(child_idx);
                    queue.push_back((child_idx, depth + 1));
                }
            }
        }
    }

    pub fn update_root_after_move(&mut self, mov: (usize, usize)) {
        if !self.nodes[self.root].is_expanded {
            self.expand_node(self.root);
        }

        let current_player = self.nodes[self.root].player;
        let children = self.nodes[self.root].children.clone();

        let mut next_root = None;
        for &child_idx in &children {
            if self.nodes[child_idx].mov == Some(mov) {
                next_root = Some(child_idx);
                break;
            }
        }

        self.game_state.make_move(mov, current_player);

        if let Some(new_root) = next_root {
            self.root = new_root;
            self.nodes[self.root].parent = None;
            self.rebase_depth(self.root, 0);
            println!(
                "搜索树已沿走法 {:?} 前进。新根节点深度为 {}。",
                mov, self.nodes[self.root].depth
            );
        } else {
            println!("警告: 在搜索树中未找到着法 {:?}。", mov);
            let new_player = 3 - current_player;
            let mut new_root_node = PNSNode::new(new_player, None, None, 0);
            new_root_node.hash = self.game_state.get_canonical_hash();
            self.root = self.nodes.len();
            self.nodes.push(new_root_node);
            self.evaluate_and_set_proof_numbers(self.root);
        }
    }
}

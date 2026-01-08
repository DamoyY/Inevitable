use std::time::Instant;

use super::{duration_to_ns, PNSSolver, PNSNode};

impl PNSSolver {
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
}

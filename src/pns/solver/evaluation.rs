use std::time::Instant;

use super::{duration_to_ns, PNSSolver};

impl PNSSolver {
    pub(crate) fn evaluate_and_set_proof_numbers(&mut self, node_idx: usize) {
        let start = Instant::now();
        self.eval_calls = self.eval_calls.saturating_add(1);
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
        } else {
            let tt_key = (node.hash, node.player);
            self.tt_lookups = self.tt_lookups.saturating_add(1);
            if let Some(entry) = self.transposition_table.get(&tt_key) {
                self.tt_hits = self.tt_hits.saturating_add(1);
                self.nodes[node_idx].pn = entry.pn;
                self.nodes[node_idx].dn = entry.dn;
                self.nodes[node_idx].win_len = entry.win_len;
            }
        }

        self.eval_time_ns = self
            .eval_time_ns
            .saturating_add(duration_to_ns(start.elapsed()));
    }
}

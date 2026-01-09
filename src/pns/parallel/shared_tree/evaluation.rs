use std::{sync::atomic::Ordering, time::Instant};

use super::{SharedTree, duration_to_ns};
use crate::pns::parallel::{context::ThreadLocalContext, node::ParallelNode};

impl SharedTree {
    pub fn evaluate_node(&self, node: &ParallelNode, ctx: &ThreadLocalContext) {
        let start = Instant::now();
        self.total_eval_calls.fetch_add(1, Ordering::Relaxed);
        let tt_entry = self.lookup_tt(node.hash, node.player);
        if let Some(entry) = tt_entry
            && (entry.pn == 0 || entry.dn == 0)
        {
            node.set_pn(entry.pn);
            node.set_dn(entry.dn);
            node.set_win_len(entry.win_len);
            self.total_eval_time_ns
                .fetch_add(duration_to_ns(start.elapsed()), Ordering::Relaxed);
            return;
        }
        let mut p1_wins = false;
        let mut p2_wins = false;
        if node.depth > 0 {
            let opponent = 3 - node.player;
            if ctx.check_win(opponent) {
                if opponent == 1 {
                    p1_wins = true;
                } else {
                    p2_wins = true;
                }
            }
        } else {
            if ctx.check_win(1) {
                p1_wins = true;
            }
            if ctx.check_win(2) {
                p2_wins = true;
            }
        }
        if p1_wins {
            node.set_proven();
            node.set_win_len(0);
        } else if p2_wins {
            node.set_disproven();
        } else if let Some(limit) = self.depth_limit
            && node.depth >= limit
        {
            self.total_depth_cutoffs.fetch_add(1, Ordering::Relaxed);
            node.set_depth_cutoff(true);
            node.set_is_depth_limited(true);
            node.set_pn(u64::MAX);
            node.set_dn(u64::MAX);
        } else if let Some(entry) = tt_entry {
            node.set_pn(entry.pn);
            node.set_dn(entry.dn);
            node.set_win_len(entry.win_len);
        }
        self.total_eval_time_ns
            .fetch_add(duration_to_ns(start.elapsed()), Ordering::Relaxed);
    }
}

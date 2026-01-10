use std::{
    sync::{Arc, atomic::Ordering},
    time::Instant,
};

use super::{SharedTree, duration_to_ns};
use crate::{
    alloc_stats::AllocTrackingGuard,
    game_state::MoveApplyTiming,
    pns::parallel::{
        context::ThreadLocalContext,
        node::{ChildRef, NodeRef, ParallelNode},
    },
};

#[derive(Default)]
struct MoveTimingTotals {
    board_update_time_ns: u64,
    bitboard_update_time_ns: u64,
    threat_index_update_time_ns: u64,
    candidate_remove_time_ns: u64,
    candidate_neighbor_time_ns: u64,
    candidate_insert_time_ns: u64,
    candidate_newly_added_time_ns: u64,
    candidate_history_time_ns: u64,
    hash_update_time_ns: u64,
    pos_hash_time_ns: u64,
    move_undo_time_ns: u64,
    node_table_lookups: u64,
}

impl MoveTimingTotals {
    const fn record_move_timing(&mut self, timing: &MoveApplyTiming) {
        self.board_update_time_ns = self
            .board_update_time_ns
            .wrapping_add(timing.board_update_ns);
        self.bitboard_update_time_ns = self
            .bitboard_update_time_ns
            .wrapping_add(timing.bitboard_update_ns);
        self.threat_index_update_time_ns = self
            .threat_index_update_time_ns
            .wrapping_add(timing.threat_index_update_ns);
        self.candidate_remove_time_ns = self
            .candidate_remove_time_ns
            .wrapping_add(timing.candidate_remove_ns);
        self.candidate_neighbor_time_ns = self
            .candidate_neighbor_time_ns
            .wrapping_add(timing.candidate_neighbor_ns);
        self.candidate_insert_time_ns = self
            .candidate_insert_time_ns
            .wrapping_add(timing.candidate_insert_ns);
        self.candidate_newly_added_time_ns = self
            .candidate_newly_added_time_ns
            .wrapping_add(timing.candidate_newly_added_ns);
        self.candidate_history_time_ns = self
            .candidate_history_time_ns
            .wrapping_add(timing.candidate_history_ns);
        self.hash_update_time_ns = self.hash_update_time_ns.wrapping_add(timing.hash_update_ns);
    }

    fn record_pos_hash_time(&mut self, elapsed: std::time::Duration) {
        self.pos_hash_time_ns = self.pos_hash_time_ns.wrapping_add(duration_to_ns(elapsed));
    }

    fn record_move_undo_time(&mut self, elapsed: std::time::Duration) {
        self.move_undo_time_ns = self.move_undo_time_ns.wrapping_add(duration_to_ns(elapsed));
    }

    const fn increment_node_table_lookups(&mut self) {
        self.node_table_lookups = self.node_table_lookups.wrapping_add(1);
    }

    fn flush_to_shared(&self, shared: &SharedTree) {
        shared
            .total_board_update_time_ns
            .fetch_add(self.board_update_time_ns, Ordering::Relaxed);
        shared
            .total_bitboard_update_time_ns
            .fetch_add(self.bitboard_update_time_ns, Ordering::Relaxed);
        shared
            .total_threat_index_update_time_ns
            .fetch_add(self.threat_index_update_time_ns, Ordering::Relaxed);
        shared
            .total_candidate_remove_time_ns
            .fetch_add(self.candidate_remove_time_ns, Ordering::Relaxed);
        shared
            .total_candidate_neighbor_time_ns
            .fetch_add(self.candidate_neighbor_time_ns, Ordering::Relaxed);
        shared
            .total_candidate_insert_time_ns
            .fetch_add(self.candidate_insert_time_ns, Ordering::Relaxed);
        shared
            .total_candidate_newly_added_time_ns
            .fetch_add(self.candidate_newly_added_time_ns, Ordering::Relaxed);
        shared
            .total_candidate_history_time_ns
            .fetch_add(self.candidate_history_time_ns, Ordering::Relaxed);
        shared
            .total_hash_update_time_ns
            .fetch_add(self.hash_update_time_ns, Ordering::Relaxed);
        shared
            .total_hash_time_ns
            .fetch_add(self.pos_hash_time_ns, Ordering::Relaxed);
        shared
            .total_node_table_lookups
            .fetch_add(self.node_table_lookups, Ordering::Relaxed);
        shared
            .total_move_undo_time_ns
            .fetch_add(self.move_undo_time_ns, Ordering::Relaxed);
    }
}
impl SharedTree {
    pub fn expand_node(&self, node: &NodeRef, ctx: &mut ThreadLocalContext) -> bool {
        let expand_start = Instant::now();
        let children_lock_start = Instant::now();
        let mut write_guard = node.children.write();
        self.total_children_lock_time_ns.fetch_add(
            duration_to_ns(children_lock_start.elapsed()),
            Ordering::Relaxed,
        );
        if write_guard.is_some() {
            return false;
        }
        self.increment_expansions();
        let _alloc_guard = AllocTrackingGuard::new();
        if let Some(limit) = self.depth_limit
            && node.depth >= limit
        {
            *write_guard = Some(Vec::new());
            self.total_depth_cutoffs.fetch_add(1, Ordering::Relaxed);
            node.set_depth_cutoff(true);
            node.set_is_depth_limited(true);
            drop(write_guard);
            self.total_expand_time_ns
                .fetch_add(duration_to_ns(expand_start.elapsed()), Ordering::Relaxed);
            return true;
        }
        let player = node.player;
        let depth = node.depth;
        let is_or_node = node.is_or_node();
        let movegen_start = Instant::now();
        let legal_moves = ctx.get_legal_moves(player);
        let legal_moves_len = legal_moves.len();
        self.total_movegen_time_ns
            .fetch_add(duration_to_ns(movegen_start.elapsed()), Ordering::Relaxed);
        let mut children = Vec::with_capacity(legal_moves.len());
        let mut timing_totals = MoveTimingTotals::default();
        for mov in legal_moves {
            let move_timing = ctx.make_move_with_timing(mov, player);
            timing_totals.record_move_timing(&move_timing);
            let pos_hash_start = Instant::now();
            let child_pos_hash = ctx.get_hash();
            timing_totals.record_pos_hash_time(pos_hash_start.elapsed());
            timing_totals.increment_node_table_lookups();
            let node_key = (child_pos_hash, depth + 1);
            let is_depth_limited = self.depth_limit.is_some_and(|limit| depth + 1 >= limit);
            let child = self.get_or_create_child(ctx, node_key, player, depth, is_depth_limited);
            let undo_start = Instant::now();
            ctx.undo_move(mov);
            timing_totals.record_move_undo_time(undo_start.elapsed());
            let proof_number = child.get_pn();
            let disproof_number = child.get_dn();
            children.push(ChildRef { node: child, mov });
            if is_or_node {
                if proof_number == 0 {
                    break;
                }
            } else if disproof_number == 0 || proof_number == u64::MAX {
                break;
            }
        }
        timing_totals.flush_to_shared(self);
        if children.len() < legal_moves_len {
            self.total_early_cutoffs.fetch_add(1, Ordering::Relaxed);
        }
        self.total_children_generated
            .fetch_add(children.len() as u64, Ordering::Relaxed);
        *write_guard = Some(children);
        drop(write_guard);
        self.total_expand_time_ns
            .fetch_add(duration_to_ns(expand_start.elapsed()), Ordering::Relaxed);
        true
    }

    fn get_or_create_child(
        &self,
        ctx: &ThreadLocalContext,
        node_key: (u64, usize),
        player: u8,
        depth: usize,
        is_depth_limited: bool,
    ) -> Arc<ParallelNode> {
        let lookup_start = Instant::now();
        let existing_child = {
            let node_table = self.node_table.read();
            node_table.get(&node_key).map(Arc::clone)
        };
        self.total_node_table_lookup_time_ns
            .fetch_add(duration_to_ns(lookup_start.elapsed()), Ordering::Relaxed);
        existing_child.map_or_else(
            || {
                let child_hash_start = Instant::now();
                let child_hash = ctx.get_canonical_hash();
                self.total_hash_time_ns.fetch_add(
                    duration_to_ns(child_hash_start.elapsed()),
                    Ordering::Relaxed,
                );
                let child = Arc::new(ParallelNode::new(
                    3 - player,
                    depth + 1,
                    child_hash,
                    is_depth_limited,
                ));
                self.evaluate_node(&child, ctx);
                let insert_start = Instant::now();
                {
                    let mut node_table = self.node_table.write();
                    node_table.insert(node_key, Arc::clone(&child));
                }
                self.total_node_table_write_time_ns
                    .fetch_add(duration_to_ns(insert_start.elapsed()), Ordering::Relaxed);
                self.total_nodes_created.fetch_add(1, Ordering::Relaxed);
                child
            },
            |child| {
                self.total_node_table_hits.fetch_add(1, Ordering::Relaxed);
                child
            },
        )
    }
}

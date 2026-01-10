use std::{
    sync::{Arc, atomic::Ordering},
    time::Instant,
};

use super::{SharedTree, duration_to_ns};
use crate::{
    alloc_stats::AllocTrackingGuard,
    pns::parallel::{
        context::ThreadLocalContext,
        node::{ChildRef, NodeRef, ParallelNode},
        TreeStatsAccumulator,
    },
};

impl SharedTree {
    pub fn expand_node(&self, node: &NodeRef, ctx: &mut ThreadLocalContext) -> bool {
        let expand_start = Instant::now();
        let children_lock_start = Instant::now();
        let mut write_guard = node.children.write();
        self.stats.children_lock_time_ns.fetch_add(
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
            self.stats.depth_cutoffs.fetch_add(1, Ordering::Relaxed);
            node.set_depth_cutoff(true);
            node.set_is_depth_limited(true);
            drop(write_guard);
            self.stats.expand_time_ns
                .fetch_add(duration_to_ns(expand_start.elapsed()), Ordering::Relaxed);
            return true;
        }
        let player = node.player;
        let depth = node.depth;
        let is_or_node = node.is_or_node();
        let movegen_start = Instant::now();
        let legal_moves = ctx.get_legal_moves(player);
        let legal_moves_len = legal_moves.len();
        self.stats.movegen_time_ns
            .fetch_add(duration_to_ns(movegen_start.elapsed()), Ordering::Relaxed);
        let mut children = Vec::with_capacity(legal_moves.len());
        let mut local_stats = TreeStatsAccumulator::default();
        for mov in legal_moves {
            let move_timing = ctx.make_move_with_timing(mov, player);
            local_stats.add_move_apply_timing(&move_timing);
            let pos_hash_start = Instant::now();
            let child_pos_hash = ctx.get_hash();
            local_stats.hash_time_ns =
                local_stats.hash_time_ns.wrapping_add(duration_to_ns(pos_hash_start.elapsed()));
            local_stats.node_table_lookups = local_stats.node_table_lookups.wrapping_add(1);
            let node_key = (child_pos_hash, depth + 1);
            let is_depth_limited = self.depth_limit.is_some_and(|limit| depth + 1 >= limit);
            let child = self.get_or_create_child(ctx, node_key, player, depth, is_depth_limited);
            let undo_start = Instant::now();
            ctx.undo_move(mov);
            local_stats.move_undo_time_ns =
                local_stats.move_undo_time_ns.wrapping_add(duration_to_ns(undo_start.elapsed()));
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
        self.stats.merge(&local_stats);
        if children.len() < legal_moves_len {
            self.stats.early_cutoffs.fetch_add(1, Ordering::Relaxed);
        }
        self.stats.children_generated
            .fetch_add(children.len() as u64, Ordering::Relaxed);
        *write_guard = Some(children);
        drop(write_guard);
        self.stats.expand_time_ns
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
        self.stats.node_table_lookup_time_ns
            .fetch_add(duration_to_ns(lookup_start.elapsed()), Ordering::Relaxed);
        existing_child.map_or_else(
            || {
                let child_hash_start = Instant::now();
                let child_hash = ctx.get_canonical_hash();
                self.stats.hash_time_ns.fetch_add(
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
                self.stats.node_table_write_time_ns
                    .fetch_add(duration_to_ns(insert_start.elapsed()), Ordering::Relaxed);
                self.stats.nodes_created.fetch_add(1, Ordering::Relaxed);
                child
            },
            |child| {
                self.stats.node_table_hits.fetch_add(1, Ordering::Relaxed);
                child
            },
        )
    }
}

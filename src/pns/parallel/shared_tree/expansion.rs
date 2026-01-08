use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Instant;

use crate::pns::parallel::context::ThreadLocalContext;
use crate::pns::parallel::node::{ChildRef, NodeRef, ParallelNode};

use super::{duration_to_ns, SharedTree};

impl SharedTree {
    pub fn expand_node(&self, node: &NodeRef, ctx: &mut ThreadLocalContext) -> bool {
        {
            let read_guard = node.children.read();
            if read_guard.is_some() {
                return false;
            }
        }

        let expand_start = Instant::now();
        let mut write_guard = node.children.write();
        if write_guard.is_some() {
            return false;
        }

        self.increment_expansions();

        if let Some(limit) = self.depth_limit
            && node.depth >= limit
        {
            *write_guard = Some(Vec::new());
            self.total_depth_cutoffs.fetch_add(1, Ordering::Relaxed);
            node.set_depth_cutoff(true);
            node.set_is_depth_limited(true);
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

        for mov in legal_moves {
            ctx.make_move(mov, player);
            let child_pos_hash = ctx.get_hash();
            self.total_node_table_lookups
                .fetch_add(1, Ordering::Relaxed);
            let node_key = (child_pos_hash, depth + 1);

            let is_depth_limited = self
                .depth_limit
                .is_some_and(|limit| depth + 1 >= limit);

            let child = if let Some(entry) = self.node_table.get(&node_key) {
                self.total_node_table_hits.fetch_add(1, Ordering::Relaxed);
                Arc::clone(entry.value())
            } else {
                let child_hash = ctx.get_canonical_hash();
                let child = Arc::new(ParallelNode::new(
                    3 - player,
                    depth + 1,
                    child_hash,
                    is_depth_limited,
                ));
                self.evaluate_node(&child, ctx);
                self.node_table.insert(node_key, Arc::clone(&child));
                self.total_nodes_created.fetch_add(1, Ordering::Relaxed);
                child
            };
            ctx.undo_move(mov);

            let child_pn = child.get_pn();
            let child_dn = child.get_dn();

            children.push(ChildRef { node: child, mov });

            if is_or_node && child_pn == 0 {
                break;
            }
            if !is_or_node && child_dn == 0 {
                break;
            }
        }

        if children.len() < legal_moves_len {
            self.total_early_cutoffs.fetch_add(1, Ordering::Relaxed);
        }
        self.total_children_generated
            .fetch_add(children.len() as u64, Ordering::Relaxed);
        *write_guard = Some(children);
        self.total_expand_time_ns
            .fetch_add(duration_to_ns(expand_start.elapsed()), Ordering::Relaxed);
        true
    }
}

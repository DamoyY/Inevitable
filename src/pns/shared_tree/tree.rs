use super::{
    super::{
        TreeStatsAtomic,
        node::{ChildRef, NodeRef, ParallelNode},
    },
    NodeTable, ShardedMap, TranspositionTable,
};
use crate::{
    alloc_stats::AllocTrackingGuard,
    checked,
    pns::{TTEntry, TreeStatsAccumulator},
    utils::duration_to_ns,
};
use alloc::{collections::VecDeque, sync::Arc};
use core::sync::atomic::{AtomicBool, Ordering};
use std::{collections::HashSet, time::Instant};
pub struct SharedTree {
    pub(crate) root: NodeRef,
    pub(crate) transposition_table: TranspositionTable,
    pub(crate) node_table: NodeTable,
    pub(crate) depth_limit: Option<usize>,
    pub(crate) solved: AtomicBool,
    pub(crate) stop_flag: Arc<AtomicBool>,
    pub(crate) stats: TreeStatsAtomic,
}
impl SharedTree {
    fn push_unvisited_children<F>(
        node: &NodeRef,
        visited: &mut HashSet<*const ParallelNode>,
        mut push: F,
    ) where
        F: FnMut(NodeRef),
    {
        if let Some(children) = node.children.get() {
            for child in children {
                let ptr = Arc::as_ptr(&child.node);
                if visited.insert(ptr) {
                    push(Arc::clone(&child.node));
                }
            }
        }
    }
    #[inline]
    #[must_use]
    pub fn with_tt_and_stop(
        root_player: u8,
        root_hash: u64,
        root_pos_hash: u64,
        depth_limit: Option<usize>,
        stop_flag: Arc<AtomicBool>,
        existing_tt: Option<TranspositionTable>,
        existing_node_table: Option<NodeTable>,
    ) -> Self {
        let root = Arc::new(ParallelNode::new(root_player, 0, root_hash, false));
        let node_table = existing_node_table.unwrap_or_else(|| Arc::new(ShardedMap::new()));
        node_table.insert((root_pos_hash, 0), Arc::clone(&root));
        let transposition_table = existing_tt.unwrap_or_else(|| Arc::new(ShardedMap::new()));
        let stats = TreeStatsAtomic::new();
        stats.nodes_created.store(1, Ordering::Relaxed);
        Self {
            root,
            transposition_table,
            node_table,
            depth_limit,
            solved: AtomicBool::new(false),
            stop_flag,
            stats,
        }
    }
    #[inline]
    pub fn increase_depth_limit(&mut self, new_depth_limit: usize) {
        if let Some(current_limit) = self.depth_limit
            && new_depth_limit <= current_limit
        {
            return;
        }
        self.depth_limit = Some(new_depth_limit);
        self.solved.store(false, Ordering::Release);
        let mut queue_visited = HashSet::new();
        let mut queue = VecDeque::new();
        queue.push_back(Arc::clone(&self.root));
        queue_visited.insert(Arc::as_ptr(&self.root));
        while let Some(node) = queue.pop_front() {
            node.set_is_depth_limited(node.depth >= new_depth_limit);
            if node.is_depth_cutoff() && node.depth < new_depth_limit {
                node.set_depth_cutoff(false);
                node.set_pn(1);
                node.set_dn(1);
                node.set_win_len(u64::MAX);
            }
            Self::push_unvisited_children(&node, &mut queue_visited, |child| {
                queue.push_back(child);
            });
        }
        let mut stack = Vec::new();
        let mut postorder_visited = HashSet::new();
        let mut postorder = Vec::new();
        stack.push((Arc::clone(&self.root), false));
        postorder_visited.insert(Arc::as_ptr(&self.root));
        while let Some((node, processed)) = stack.pop() {
            if processed {
                postorder.push(node);
                continue;
            }
            stack.push((Arc::clone(&node), true));
            Self::push_unvisited_children(&node, &mut postorder_visited, |child| {
                stack.push((child, false));
            });
        }
        for node in postorder {
            self.update_node_pdn(&node);
        }
    }
    #[inline]
    pub fn is_solved(&self) -> bool {
        self.solved.load(Ordering::Acquire)
    }
    #[inline]
    pub fn stop_requested(&self) -> bool {
        self.stop_flag.load(Ordering::Acquire)
    }
    #[inline]
    pub fn should_stop(&self) -> bool {
        self.is_solved() || self.stop_requested()
    }
    #[inline]
    pub fn mark_solved(&self) {
        self.solved.store(true, Ordering::Release);
    }
    #[inline]
    pub fn increment_iterations(&self) {
        self.stats.iterations.fetch_add(1, Ordering::Relaxed);
    }
    #[inline]
    pub fn increment_expansions(&self) {
        self.stats.expansions.fetch_add(1, Ordering::Relaxed);
    }
    #[inline]
    #[must_use]
    pub fn stats_snapshot(&self) -> super::super::TreeStatsSnapshot {
        self.stats.snapshot()
    }
    #[inline]
    pub fn get_tt(&self) -> TranspositionTable {
        Arc::clone(&self.transposition_table)
    }
    #[inline]
    pub fn get_node_table(&self) -> NodeTable {
        Arc::clone(&self.node_table)
    }
    #[inline]
    pub fn get_tt_size(&self) -> usize {
        self.transposition_table.len()
    }
    #[inline]
    pub fn get_node_table_size(&self) -> usize {
        self.node_table.len()
    }
    #[inline]
    pub fn lookup_tt(&self, hash: u64, player: u8) -> Option<TTEntry> {
        self.stats.tt_lookups.fetch_add(1, Ordering::Relaxed);
        let entry = self.transposition_table.get(&(hash, player));
        if entry.is_some() {
            self.stats.tt_hits.fetch_add(1, Ordering::Relaxed);
        }
        entry
    }
    #[inline]
    pub fn store_tt(&self, hash: u64, player: u8, entry: TTEntry) {
        self.transposition_table.insert((hash, player), entry);
        self.stats.tt_stores.fetch_add(1, Ordering::Relaxed);
    }
    #[inline]
    pub fn select_best_child(node: &NodeRef) -> Option<ChildRef> {
        let children = node.children.get()?;
        let is_or_node = node.is_or_node();
        children
            .iter()
            .min_by_key(|child_ref| {
                if is_or_node {
                    (
                        child_ref.node.get_effective_pn(),
                        child_ref.node.get_win_len(),
                    )
                } else {
                    (
                        child_ref.node.get_effective_dn(),
                        child_ref.node.get_win_len(),
                    )
                }
            })
            .cloned()
    }
    #[inline]
    pub fn evaluate_node(
        &self,
        node: &ParallelNode,
        ctx: &super::super::context::ThreadLocalContext,
    ) {
        let start = Instant::now();
        self.stats.eval_calls.fetch_add(1, Ordering::Relaxed);
        let tt_entry = self.lookup_tt(node.hash, node.player);
        if let Some(entry) = tt_entry
            && (entry.pn == 0 || entry.dn == 0)
        {
            node.set_pn(entry.pn);
            node.set_dn(entry.dn);
            node.set_win_len(entry.win_len);
            self.stats
                .eval_time_ns
                .fetch_add(duration_to_ns(start.elapsed()), Ordering::Relaxed);
            return;
        }
        let mut p1_wins = false;
        let mut p2_wins = false;
        if node.depth > 0 {
            let opponent = checked::opponent_player(node.player, "SharedTree::evaluate_node");
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
            self.stats.depth_cutoffs.fetch_add(1, Ordering::Relaxed);
            node.set_depth_cutoff(true);
            node.set_is_depth_limited(true);
            node.set_pn(u64::MAX);
            node.set_dn(u64::MAX);
        } else if let Some(entry) = tt_entry {
            node.set_pn(entry.pn);
            node.set_dn(entry.dn);
            node.set_win_len(entry.win_len);
        }
        self.stats
            .eval_time_ns
            .fetch_add(duration_to_ns(start.elapsed()), Ordering::Relaxed);
    }
    #[inline]
    pub fn expand_node(
        &self,
        node: &NodeRef,
        ctx: &mut super::super::context::ThreadLocalContext,
    ) -> bool {
        if node.children.get().is_some() || node.is_depth_cutoff() {
            return false;
        }
        let expand_start = Instant::now();
        let _alloc_guard = AllocTrackingGuard::new();
        if let Some(limit) = self.depth_limit
            && node.depth >= limit
        {
            if !node.try_mark_depth_cutoff() {
                return false;
            }
            self.stats.depth_cutoffs.fetch_add(1, Ordering::Relaxed);
            node.set_is_depth_limited(true);
            node.set_pn(u64::MAX);
            node.set_dn(u64::MAX);
            node.set_win_len(u64::MAX);
            self.stats
                .expand_time_ns
                .fetch_add(duration_to_ns(expand_start.elapsed()), Ordering::Relaxed);
            return true;
        }
        let player = node.player;
        let depth = node.depth;
        let is_or_node = node.is_or_node();
        let move_gen_timing = ctx.refresh_legal_moves(player);
        self.stats
            .move_gen_candidates_time_ns
            .fetch_add(move_gen_timing.candidate_gen_ns, Ordering::Relaxed);
        self.stats
            .move_gen_scoring_time_ns
            .fetch_add(move_gen_timing.scoring_ns, Ordering::Relaxed);
        let legal_moves = core::mem::take(&mut ctx.legal_moves);
        let legal_moves_len = legal_moves.len();
        let mut children = Vec::with_capacity(legal_moves_len);
        let mut local_stats = TreeStatsAccumulator::default();
        for &mov in &legal_moves {
            let move_timing = ctx.make_move_with_timing(mov, player);
            local_stats.add_move_apply_timing(&move_timing);
            let pos_hash_start = Instant::now();
            let child_pos_hash = ctx.get_hash();
            local_stats.hash_time_ns = checked::add_u64(
                local_stats.hash_time_ns,
                duration_to_ns(pos_hash_start.elapsed()),
                "SharedTree::expand_node::hash_time_ns",
            );
            let child_depth = checked::add_usize(depth, 1_usize, "SharedTree::expand_node::depth");
            let node_key = (child_pos_hash, child_depth);
            let is_depth_limited = self.depth_limit.is_some_and(|limit| child_depth >= limit);
            let child = ctx.get_cached_node(&node_key).unwrap_or_else(|| {
                local_stats.node_table_lookups = checked::add_u64(
                    local_stats.node_table_lookups,
                    1_u64,
                    "SharedTree::expand_node::node_table_lookups",
                );
                let child =
                    self.get_or_create_child(ctx, node_key, player, depth, is_depth_limited);
                ctx.cache_node(node_key, Arc::clone(&child));
                child
            });
            let undo_start = Instant::now();
            ctx.undo_move(mov, player);
            local_stats.move_undo_time_ns = checked::add_u64(
                local_stats.move_undo_time_ns,
                duration_to_ns(undo_start.elapsed()),
                "SharedTree::expand_node::move_undo_time_ns",
            );
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
        ctx.legal_moves = legal_moves;
        let early_cutoff = children.len() < legal_moves_len;
        let children_len =
            checked::usize_to_u64(children.len(), "SharedTree::expand_node::children_len");
        if node.children.set(children).is_err() {
            return false;
        }
        self.stats.merge(&local_stats);
        self.increment_expansions();
        if early_cutoff {
            self.stats.early_cutoffs.fetch_add(1, Ordering::Relaxed);
        }
        self.stats
            .children_generated
            .fetch_add(children_len, Ordering::Relaxed);
        self.stats
            .expand_time_ns
            .fetch_add(duration_to_ns(expand_start.elapsed()), Ordering::Relaxed);
        true
    }
    fn get_or_create_child(
        &self,
        ctx: &super::super::context::ThreadLocalContext,
        node_key: (u64, usize),
        player: u8,
        depth: usize,
        is_depth_limited: bool,
    ) -> Arc<ParallelNode> {
        let lookup_start = Instant::now();
        let existing_child = self.node_table.get(&node_key);
        self.stats
            .node_table_lookup_time_ns
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
                    checked::opponent_player(player, "SharedTree::get_or_create_child"),
                    checked::add_usize(depth, 1_usize, "SharedTree::get_or_create_child::depth"),
                    child_hash,
                    is_depth_limited,
                ));
                self.evaluate_node(&child, ctx);
                let insert_start = Instant::now();
                self.node_table.insert(node_key, Arc::clone(&child));
                self.stats
                    .node_table_write_time_ns
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
    #[inline]
    pub fn update_node_pdn(&self, node: &NodeRef) {
        let prev_proof = node.get_pn();
        let prev_disproof = node.get_dn();
        let prev_win_len = node.get_win_len();
        let Some(children) = node.children.get() else {
            if node.is_depth_limited() && node.is_depth_cutoff() {
                node.set_pn(u64::MAX);
                node.set_dn(u64::MAX);
                node.set_win_len(u64::MAX);
                self.store_tt_if_changed(node, prev_proof, prev_disproof, prev_win_len);
            }
            return;
        };
        if node.is_depth_limited() && children.is_empty() {
            node.set_pn(u64::MAX);
            node.set_dn(u64::MAX);
            node.set_win_len(u64::MAX);
            self.store_tt_if_changed(node, prev_proof, prev_disproof, prev_win_len);
            return;
        }
        if children.is_empty() {
            if node.is_or_node() {
                node.set_pn(u64::MAX);
                node.set_dn(0);
                node.set_win_len(u64::MAX);
            } else {
                node.set_pn(0);
                node.set_dn(u64::MAX);
                node.set_win_len(0);
            }
            self.store_tt_if_changed(node, prev_proof, prev_disproof, prev_win_len);
            return;
        }
        let is_or_node = node.is_or_node();
        let mut pn_min = u64::MAX;
        let mut pn_sum = 0_u64;
        let mut dn_min = u64::MAX;
        let mut dn_sum = 0_u64;
        let mut min_proven_win_len = u64::MAX;
        let mut max_proven_win_len = 0_u64;
        let mut all_children_proven = true;
        for child in children {
            let cpn = child.node.get_pn();
            let cdn = child.node.get_dn();
            let cwl = child.node.get_win_len();
            pn_min = pn_min.min(cpn);
            pn_sum = pn_sum.saturating_add(cpn);
            dn_min = dn_min.min(cdn);
            dn_sum = dn_sum.saturating_add(cdn);
            if cpn == 0 {
                min_proven_win_len = min_proven_win_len.min(cwl);
                max_proven_win_len = max_proven_win_len.max(cwl);
            } else {
                all_children_proven = false;
            }
        }
        if is_or_node {
            node.set_pn(pn_min);
            node.set_dn(dn_sum);
            if min_proven_win_len < u64::MAX {
                node.set_win_len(1_u64.saturating_add(min_proven_win_len));
            } else {
                node.set_win_len(u64::MAX);
            }
        } else {
            node.set_pn(pn_sum);
            node.set_dn(dn_min);
            if dn_min == 0 {
                node.set_win_len(u64::MAX);
            } else if all_children_proven {
                node.set_win_len(1_u64.saturating_add(max_proven_win_len));
            } else {
                node.set_win_len(u64::MAX);
            }
        }
        self.store_tt_if_changed(node, prev_proof, prev_disproof, prev_win_len);
    }
    fn store_tt_if_changed(
        &self,
        node: &NodeRef,
        prev_proof: u64,
        prev_disproof: u64,
        prev_win_len: u64,
    ) {
        if node.is_depth_limited() {
            return;
        }
        let pn = node.get_pn();
        let dn = node.get_dn();
        if pn == u64::MAX && dn == u64::MAX {
            return;
        }
        let win_len = node.get_win_len();
        if pn == prev_proof && dn == prev_disproof && win_len == prev_win_len {
            return;
        }
        self.store_tt(node.hash, node.player, TTEntry { pn, dn, win_len });
    }
}

use std::{
    collections::{HashSet, VecDeque},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

use super::{
    super::{
        TreeStatsAtomic,
        node::{ChildRef, NodeRef, ParallelNode},
    },
    NodeTable, ShardedMap, TranspositionTable,
};
use crate::pns::TTEntry;
pub struct SharedTree {
    pub root: NodeRef,
    pub transposition_table: TranspositionTable,
    pub node_table: NodeTable,
    pub depth_limit: Option<usize>,
    pub solved: AtomicBool,
    pub(super) stop_flag: Arc<AtomicBool>,
    pub stats: TreeStatsAtomic,
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

    #[must_use]
    pub fn new(
        root_player: u8,
        root_hash: u64,
        root_pos_hash: u64,
        depth_limit: Option<usize>,
    ) -> Self {
        Self::with_tt(
            root_player,
            root_hash,
            root_pos_hash,
            depth_limit,
            None,
            None,
        )
    }

    #[must_use]
    pub fn with_tt(
        root_player: u8,
        root_hash: u64,
        root_pos_hash: u64,
        depth_limit: Option<usize>,
        existing_tt: Option<TranspositionTable>,
        existing_node_table: Option<NodeTable>,
    ) -> Self {
        Self::with_tt_and_stop(
            root_player,
            root_hash,
            root_pos_hash,
            depth_limit,
            Arc::new(AtomicBool::new(false)),
            existing_tt,
            existing_node_table,
        )
    }

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

    pub fn increase_depth_limit(&mut self, new_depth_limit: usize) {
        if let Some(current_limit) = self.depth_limit
            && new_depth_limit <= current_limit
        {
            return;
        }
        self.depth_limit = Some(new_depth_limit);
        self.solved.store(false, Ordering::Release);
        let mut visited = HashSet::new();
        let mut queue = VecDeque::new();
        queue.push_back(Arc::clone(&self.root));
        visited.insert(Arc::as_ptr(&self.root));
        while let Some(node) = queue.pop_front() {
            node.set_is_depth_limited(node.depth >= new_depth_limit);
            if node.is_depth_cutoff() && node.depth < new_depth_limit {
                node.set_depth_cutoff(false);
                node.set_pn(1);
                node.set_dn(1);
                node.set_win_len(u64::MAX);
            }
            Self::push_unvisited_children(&node, &mut visited, |child| {
                queue.push_back(child);
            });
        }
        let mut stack = Vec::new();
        let mut visited = HashSet::new();
        let mut postorder = Vec::new();
        stack.push((Arc::clone(&self.root), false));
        visited.insert(Arc::as_ptr(&self.root));
        while let Some((node, processed)) = stack.pop() {
            if processed {
                postorder.push(node);
                continue;
            }
            stack.push((Arc::clone(&node), true));
            Self::push_unvisited_children(&node, &mut visited, |child| {
                stack.push((child, false));
            });
        }
        for node in postorder {
            self.update_node_pdn(&node);
        }
    }

    pub fn is_solved(&self) -> bool {
        self.solved.load(Ordering::Acquire)
    }

    pub fn stop_requested(&self) -> bool {
        self.stop_flag.load(Ordering::Acquire)
    }

    pub fn should_stop(&self) -> bool {
        self.is_solved() || self.stop_requested()
    }

    pub fn mark_solved(&self) {
        self.solved.store(true, Ordering::Release);
    }

    pub fn select_best_child(&self, node: &NodeRef) -> Option<ChildRef> {
        let children = node.children.get()?;
        let is_or_node = node.is_or_node();
        children
            .iter()
            .min_by_key(|c| {
                if is_or_node {
                    (c.node.get_effective_pn(), c.node.get_win_len())
                } else {
                    (c.node.get_effective_dn(), c.node.get_win_len())
                }
            })
            .cloned()
    }

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
        let mut pn_sum = 0u64;
        let mut dn_min = u64::MAX;
        let mut dn_sum = 0u64;
        let mut min_proven_win_len = u64::MAX;
        let mut max_proven_win_len = 0u64;
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
                node.set_win_len(1u64.saturating_add(min_proven_win_len));
            } else {
                node.set_win_len(u64::MAX);
            }
        } else {
            node.set_pn(pn_sum);
            node.set_dn(dn_min);
            if dn_min == 0 {
                node.set_win_len(u64::MAX);
            } else if all_children_proven {
                node.set_win_len(1u64.saturating_add(max_proven_win_len));
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

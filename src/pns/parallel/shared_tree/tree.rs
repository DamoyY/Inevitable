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
        children.iter().min_by_key(|c| {
            if is_or_node {
                (c.node.get_effective_pn(), c.node.get_win_len())
            } else {
                (c.node.get_effective_dn(), c.node.get_win_len())
            }
        }).cloned()
    }
}

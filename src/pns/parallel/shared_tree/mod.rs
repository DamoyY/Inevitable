use std::{
    collections::{HashMap, HashSet, VecDeque},
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
};

use parking_lot::RwLock;

use super::node::{NodeRef, ParallelNode};
use crate::pns::TTEntry;
mod evaluation;
mod expansion;
mod update;
pub type TranspositionTable = Arc<RwLock<HashMap<(u64, u8), TTEntry>>>;
pub struct SharedTree {
    pub root: NodeRef,
    pub transposition_table: TranspositionTable,
    pub node_table: RwLock<HashMap<(u64, usize), NodeRef>>,
    pub depth_limit: Option<usize>,
    pub solved: AtomicBool,
    pub total_iterations: AtomicU64,
    pub total_expansions: AtomicU64,
    pub total_tt_lookups: AtomicU64,
    pub total_tt_hits: AtomicU64,
    pub total_tt_stores: AtomicU64,
    pub total_eval_calls: AtomicU64,
    pub total_eval_time_ns: AtomicU64,
    pub total_expand_time_ns: AtomicU64,
    pub total_movegen_time_ns: AtomicU64,
    pub total_move_apply_time_ns: AtomicU64,
    pub total_hash_time_ns: AtomicU64,
    pub total_node_table_time_ns: AtomicU64,
    pub total_children_generated: AtomicU64,
    pub total_depth_cutoffs: AtomicU64,
    pub total_early_cutoffs: AtomicU64,
    pub total_node_table_lookups: AtomicU64,
    pub total_node_table_hits: AtomicU64,
    pub total_nodes_created: AtomicU64,
}

impl SharedTree {
    #[must_use]
    pub fn new(
        root_player: u8,
        root_hash: u64,
        root_pos_hash: u64,
        depth_limit: Option<usize>,
    ) -> Self {
        Self::with_tt(root_player, root_hash, root_pos_hash, depth_limit, None)
    }

    #[must_use]
    pub fn with_tt(
        root_player: u8,
        root_hash: u64,
        root_pos_hash: u64,
        depth_limit: Option<usize>,
        existing_tt: Option<TranspositionTable>,
    ) -> Self {
        let root = Arc::new(ParallelNode::new(root_player, 0, root_hash, false));
        let mut node_table = HashMap::new();
        node_table.insert((root_pos_hash, 0), Arc::clone(&root));
        let transposition_table =
            existing_tt.unwrap_or_else(|| Arc::new(RwLock::new(HashMap::new())));
        Self {
            root,
            transposition_table,
            node_table: RwLock::new(node_table),
            depth_limit,
            solved: AtomicBool::new(false),
            total_iterations: AtomicU64::new(0),
            total_expansions: AtomicU64::new(0),
            total_tt_lookups: AtomicU64::new(0),
            total_tt_hits: AtomicU64::new(0),
            total_tt_stores: AtomicU64::new(0),
            total_eval_calls: AtomicU64::new(0),
            total_eval_time_ns: AtomicU64::new(0),
            total_expand_time_ns: AtomicU64::new(0),
            total_movegen_time_ns: AtomicU64::new(0),
            total_move_apply_time_ns: AtomicU64::new(0),
            total_hash_time_ns: AtomicU64::new(0),
            total_node_table_time_ns: AtomicU64::new(0),
            total_children_generated: AtomicU64::new(0),
            total_depth_cutoffs: AtomicU64::new(0),
            total_early_cutoffs: AtomicU64::new(0),
            total_node_table_lookups: AtomicU64::new(0),
            total_node_table_hits: AtomicU64::new(0),
            total_nodes_created: AtomicU64::new(1),
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

                let mut children_guard = node.children.write();
                if matches!(children_guard.as_ref(), Some(children) if children.is_empty()) {
                    *children_guard = None;
                }
            }

            let children_guard = node.children.read();
            if let Some(children) = children_guard.as_ref() {
                for child in children {
                    let ptr = Arc::as_ptr(&child.node);
                    if visited.insert(ptr) {
                        queue.push_back(Arc::clone(&child.node));
                    }
                }
            }
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
            let children_guard = node.children.read();
            if let Some(children) = children_guard.as_ref() {
                for child in children {
                    let ptr = Arc::as_ptr(&child.node);
                    if visited.insert(ptr) {
                        stack.push((Arc::clone(&child.node), false));
                    }
                }
            }
        }

        for node in postorder {
            self.update_node_pdn(&node);
        }
    }

    pub fn is_solved(&self) -> bool {
        self.solved.load(Ordering::Acquire)
    }

    pub fn mark_solved(&self) {
        self.solved.store(true, Ordering::Release);
    }

    pub fn increment_iterations(&self) {
        self.total_iterations.fetch_add(1, Ordering::Relaxed);
    }

    pub fn increment_expansions(&self) {
        self.total_expansions.fetch_add(1, Ordering::Relaxed);
    }

    pub fn get_iterations(&self) -> u64 {
        self.total_iterations.load(Ordering::Relaxed)
    }

    pub fn get_expansions(&self) -> u64 {
        self.total_expansions.load(Ordering::Relaxed)
    }

    pub fn get_tt_lookups(&self) -> u64 {
        self.total_tt_lookups.load(Ordering::Relaxed)
    }

    pub fn get_tt_hits(&self) -> u64 {
        self.total_tt_hits.load(Ordering::Relaxed)
    }

    pub fn get_tt_stores(&self) -> u64 {
        self.total_tt_stores.load(Ordering::Relaxed)
    }

    pub fn get_eval_calls(&self) -> u64 {
        self.total_eval_calls.load(Ordering::Relaxed)
    }

    pub fn get_eval_time_ns(&self) -> u64 {
        self.total_eval_time_ns.load(Ordering::Relaxed)
    }

    pub fn get_expand_time_ns(&self) -> u64 {
        self.total_expand_time_ns.load(Ordering::Relaxed)
    }

    pub fn get_movegen_time_ns(&self) -> u64 {
        self.total_movegen_time_ns.load(Ordering::Relaxed)
    }

    pub fn get_move_apply_time_ns(&self) -> u64 {
        self.total_move_apply_time_ns.load(Ordering::Relaxed)
    }

    pub fn get_hash_time_ns(&self) -> u64 {
        self.total_hash_time_ns.load(Ordering::Relaxed)
    }

    pub fn get_node_table_time_ns(&self) -> u64 {
        self.total_node_table_time_ns.load(Ordering::Relaxed)
    }

    pub fn get_children_generated(&self) -> u64 {
        self.total_children_generated.load(Ordering::Relaxed)
    }

    pub fn get_depth_cutoffs(&self) -> u64 {
        self.total_depth_cutoffs.load(Ordering::Relaxed)
    }

    pub fn get_early_cutoffs(&self) -> u64 {
        self.total_early_cutoffs.load(Ordering::Relaxed)
    }

    pub fn get_node_table_lookups(&self) -> u64 {
        self.total_node_table_lookups.load(Ordering::Relaxed)
    }

    pub fn get_node_table_hits(&self) -> u64 {
        self.total_node_table_hits.load(Ordering::Relaxed)
    }

    pub fn get_nodes_created(&self) -> u64 {
        self.total_nodes_created.load(Ordering::Relaxed)
    }

    pub fn get_tt(&self) -> TranspositionTable {
        Arc::clone(&self.transposition_table)
    }

    pub fn get_tt_size(&self) -> usize {
        self.transposition_table.read().len()
    }

    pub fn get_node_table_size(&self) -> usize {
        self.node_table.read().len()
    }

    pub fn lookup_tt(&self, hash: u64, player: u8) -> Option<TTEntry> {
        self.total_tt_lookups.fetch_add(1, Ordering::Relaxed);
        let entry = self
            .transposition_table
            .read()
            .get(&(hash, player))
            .copied();
        if entry.is_some() {
            self.total_tt_hits.fetch_add(1, Ordering::Relaxed);
        }
        entry
    }

    pub fn store_tt(&self, hash: u64, player: u8, entry: TTEntry) {
        self.transposition_table
            .write()
            .insert((hash, player), entry);
        self.total_tt_stores.fetch_add(1, Ordering::Relaxed);
    }
}

pub(super) fn duration_to_ns(duration: std::time::Duration) -> u64 {
    let nanos = duration.as_nanos();
    u64::try_from(nanos).unwrap_or(u64::MAX)
}

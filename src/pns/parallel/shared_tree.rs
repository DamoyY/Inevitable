use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicU64, Ordering},
};
use std::collections::{HashSet, VecDeque};
use std::time::Instant;

use dashmap::DashMap;

use super::{
    context::ThreadLocalContext,
    node::{ChildRef, NodeRef, ParallelNode},
};
use crate::pns::TTEntry;

pub struct SharedTree {
    pub root: NodeRef,
    pub transposition_table: DashMap<(u64, u8), TTEntry>,
    pub node_table: DashMap<(u64, usize), NodeRef>,
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
    pub total_children_generated: AtomicU64,
    pub total_depth_cutoffs: AtomicU64,
    pub total_early_cutoffs: AtomicU64,
    pub total_node_table_lookups: AtomicU64,
    pub total_node_table_hits: AtomicU64,
    pub total_nodes_created: AtomicU64,
}

impl SharedTree {
    pub fn new(
        root_player: u8,
        root_hash: u64,
        root_pos_hash: u64,
        depth_limit: Option<usize>,
    ) -> Self {
        let root = Arc::new(ParallelNode::new(root_player, 0, root_hash, false));
        let node_table = DashMap::new();
        node_table.insert((root_pos_hash, 0), Arc::clone(&root));

        Self {
            root,
            transposition_table: DashMap::new(),
            node_table,
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

    pub fn lookup_tt(&self, hash: u64, player: u8) -> Option<TTEntry> {
        self.total_tt_lookups.fetch_add(1, Ordering::Relaxed);
        let entry = self.transposition_table.get(&(hash, player));
        if entry.is_some() {
            self.total_tt_hits.fetch_add(1, Ordering::Relaxed);
        }
        entry.map(|e| *e)
    }

    pub fn store_tt(&self, hash: u64, player: u8, entry: TTEntry) {
        self.transposition_table.insert((hash, player), entry);
        self.total_tt_stores.fetch_add(1, Ordering::Relaxed);
    }

    pub fn evaluate_node(&self, node: &ParallelNode, ctx: &ThreadLocalContext) {
        let start = Instant::now();
        self.total_eval_calls.fetch_add(1, Ordering::Relaxed);
        if let Some(entry) = self.lookup_tt(node.hash, node.player) {
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
            node.set_pn(0);
            node.set_dn(u64::MAX);
            node.set_win_len(0);
        } else if p2_wins {
            node.set_pn(u64::MAX);
            node.set_dn(0);
        } else if let Some(limit) = self.depth_limit
            && node.depth >= limit
        {
            self.total_depth_cutoffs.fetch_add(1, Ordering::Relaxed);
            node.set_depth_cutoff(true);
            node.set_is_depth_limited(true);
            node.set_pn(u64::MAX);
            node.set_dn(0);
        }

        self.total_eval_time_ns
            .fetch_add(duration_to_ns(start.elapsed()), Ordering::Relaxed);
    }

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

            let is_depth_limited = self.depth_limit.is_some_and(|limit| depth + 1 >= limit);

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

    pub fn update_node_pdn(&self, node: &NodeRef) {
        let children_guard = node.children.read();
        let children = match children_guard.as_ref() {
            Some(c) => c,
            None => return,
        };

        if node.is_depth_limited() && children.is_empty() {
            node.set_pn(1);
            node.set_dn(1);
            node.set_win_len(u64::MAX);
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
            return;
        }

        let is_or_node = node.is_or_node();

        if is_or_node {
            let mut min_pn = u64::MAX;
            let mut sum_dn = 0u64;
            let mut min_proven_win_len = u64::MAX;

            for child in children.iter() {
                let cpn = child.node.get_pn();
                let cdn = child.node.get_dn();
                let cwl = child.node.get_win_len();

                if cpn < min_pn {
                    min_pn = cpn;
                }
                sum_dn = sum_dn.saturating_add(cdn);

                if cpn == 0 && cwl < min_proven_win_len {
                    min_proven_win_len = cwl;
                }
            }

            node.set_pn(min_pn);
            node.set_dn(sum_dn);

            if min_proven_win_len < u64::MAX {
                node.set_win_len(1u64.saturating_add(min_proven_win_len));
            } else {
                node.set_win_len(u64::MAX);
            }
        } else {
            let mut sum_pn = 0u64;
            let mut min_dn = u64::MAX;
            let mut all_proven = true;
            let mut max_win_len = 0u64;

            for child in children.iter() {
                let cpn = child.node.get_pn();
                let cdn = child.node.get_dn();
                let cwl = child.node.get_win_len();

                sum_pn = sum_pn.saturating_add(cpn);
                if cdn < min_dn {
                    min_dn = cdn;
                }

                if cpn != 0 {
                    all_proven = false;
                } else if cwl > max_win_len {
                    max_win_len = cwl;
                }
            }

            node.set_pn(sum_pn);
            node.set_dn(min_dn);

            if min_dn == 0 {
                node.set_win_len(u64::MAX);
            } else if all_proven {
                node.set_win_len(1u64.saturating_add(max_win_len));
            } else {
                node.set_win_len(u64::MAX);
            }
        }

        let pn = node.get_pn();
        let dn = node.get_dn();
        if (pn == 0 || dn == 0) && !node.is_depth_limited() {
            self.store_tt(
                node.hash,
                node.player,
                TTEntry {
                    pn,
                    dn,
                    win_len: node.get_win_len(),
                },
            );
        }
    }
}

fn duration_to_ns(duration: std::time::Duration) -> u64 {
    let nanos = duration.as_nanos();
    if nanos > u128::from(u64::MAX) {
        u64::MAX
    } else {
        nanos as u64
    }
}

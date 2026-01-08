use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicU64, Ordering},
};

use dashmap::DashMap;

use super::{
    context::ThreadLocalContext,
    node::{NodeRef, ParallelNode},
};
use crate::pns::TTEntry;

pub struct SharedTree {
    pub root: NodeRef,
    pub transposition_table: DashMap<(u64, u8), TTEntry>,
    pub depth_limit: Option<usize>,
    pub solved: AtomicBool,
    pub total_iterations: AtomicU64,
    pub total_expansions: AtomicU64,
}

impl SharedTree {
    pub fn new(root_player: u8, root_hash: u64, depth_limit: Option<usize>) -> Self {
        let root = Arc::new(ParallelNode::new(root_player, None, 0, root_hash, false));

        Self {
            root,
            transposition_table: DashMap::new(),
            depth_limit,
            solved: AtomicBool::new(false),
            total_iterations: AtomicU64::new(0),
            total_expansions: AtomicU64::new(0),
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

    pub fn lookup_tt(&self, hash: u64, player: u8) -> Option<TTEntry> {
        self.transposition_table.get(&(hash, player)).map(|e| *e)
    }

    pub fn store_tt(&self, hash: u64, player: u8, entry: TTEntry) {
        self.transposition_table.insert((hash, player), entry);
    }

    pub fn evaluate_node(&self, node: &ParallelNode, ctx: &ThreadLocalContext) {
        if let Some(entry) = self.lookup_tt(node.hash, node.player) {
            node.set_pn(entry.pn);
            node.set_dn(entry.dn);
            node.set_win_len(entry.win_len);
            return;
        }

        let mut p1_wins = false;
        let mut p2_wins = false;

        if node.mov.is_some() {
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
            node.set_pn(u64::MAX);
            node.set_dn(0);
        }
    }

    pub fn expand_node(&self, node: &NodeRef, ctx: &mut ThreadLocalContext) -> bool {
        {
            let read_guard = node.children.read();
            if read_guard.is_some() {
                return false;
            }
        }

        let mut write_guard = node.children.write();
        if write_guard.is_some() {
            return false;
        }

        self.increment_expansions();

        if let Some(limit) = self.depth_limit
            && node.depth >= limit
        {
            *write_guard = Some(Vec::new());
            return true;
        }

        let player = node.player;
        let depth = node.depth;
        let is_or_node = node.is_or_node();
        let legal_moves = ctx.get_legal_moves(player);

        let mut children = Vec::with_capacity(legal_moves.len());

        for mov in legal_moves {
            ctx.make_move(mov, player);
            let child_hash = ctx.get_canonical_hash();

            let is_depth_limited = self.depth_limit.is_some_and(|limit| depth + 1 >= limit);

            let child = Arc::new(ParallelNode::new(
                3 - player,
                Some(mov),
                depth + 1,
                child_hash,
                is_depth_limited,
            ));

            self.evaluate_node(&child, ctx);
            ctx.undo_move(mov);

            let child_pn = child.get_pn();
            let child_dn = child.get_dn();

            children.push(child);

            if is_or_node && child_pn == 0 {
                break;
            }
            if !is_or_node && child_dn == 0 {
                break;
            }
        }

        *write_guard = Some(children);
        true
    }

    pub fn update_node_pdn(&self, node: &NodeRef) {
        let children_guard = node.children.read();
        let children = match children_guard.as_ref() {
            Some(c) => c,
            None => return,
        };

        if node.is_depth_limited && children.is_empty() {
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
                let cpn = child.get_pn();
                let cdn = child.get_dn();
                let cwl = child.get_win_len();

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
                let cpn = child.get_pn();
                let cdn = child.get_dn();
                let cwl = child.get_win_len();

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
        if (pn == 0 || dn == 0) && !node.is_depth_limited {
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

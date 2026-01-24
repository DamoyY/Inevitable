use std::sync::{
    Arc, OnceLock,
    atomic::{AtomicBool, AtomicU64, Ordering},
};

use super::{context::ThreadLocalContext, shared_tree::SharedTree};
pub type NodeRef = Arc<ParallelNode>;
#[derive(Clone)]
pub struct ChildRef {
    pub node: NodeRef,
    pub mov: (usize, usize),
}
pub struct ParallelNode {
    pub player: u8,
    pub depth: usize,
    pub hash: u64,
    pub pn: AtomicU64,
    pub dn: AtomicU64,
    pub virtual_pn: AtomicU64,
    pub virtual_dn: AtomicU64,
    pub win_len: AtomicU64,
    pub children: OnceLock<Vec<ChildRef>>,
    pub is_depth_limited: AtomicBool,
    pub depth_cutoff: AtomicBool,
}
impl ParallelNode {
    #[must_use]
    pub const fn new(player: u8, depth: usize, hash: u64, is_depth_limited: bool) -> Self {
        Self {
            player,
            depth,
            hash,
            pn: AtomicU64::new(1),
            dn: AtomicU64::new(1),
            virtual_pn: AtomicU64::new(0),
            virtual_dn: AtomicU64::new(0),
            win_len: AtomicU64::new(u64::MAX),
            children: OnceLock::new(),
            is_depth_limited: AtomicBool::new(is_depth_limited),
            depth_cutoff: AtomicBool::new(false),
        }
    }

    #[inline]
    pub const fn is_or_node(&self) -> bool {
        self.player == 1
    }

    #[inline]
    pub fn is_expanded(&self) -> bool {
        self.children.get().is_some() || self.is_depth_cutoff()
    }

    #[inline]
    pub fn is_terminal(&self) -> bool {
        let pn = self.pn.load(Ordering::Acquire);
        let dn = self.dn.load(Ordering::Acquire);
        pn == 0 || dn == 0
    }

    #[inline]
    pub fn get_pn(&self) -> u64 {
        self.pn.load(Ordering::Acquire)
    }

    #[inline]
    pub fn get_dn(&self) -> u64 {
        self.dn.load(Ordering::Acquire)
    }

    #[inline]
    pub fn get_virtual_pn(&self) -> u64 {
        self.virtual_pn.load(Ordering::Acquire)
    }

    #[inline]
    pub fn get_virtual_dn(&self) -> u64 {
        self.virtual_dn.load(Ordering::Acquire)
    }

    #[inline]
    pub fn get_effective_pn(&self) -> u64 {
        self.get_pn().saturating_add(self.get_virtual_pn())
    }

    #[inline]
    pub fn get_effective_dn(&self) -> u64 {
        self.get_dn().saturating_add(self.get_virtual_dn())
    }

    #[inline]
    pub fn get_win_len(&self) -> u64 {
        self.win_len.load(Ordering::Acquire)
    }

    #[inline]
    pub fn is_depth_limited(&self) -> bool {
        self.is_depth_limited.load(Ordering::Acquire)
    }

    #[inline]
    pub fn set_is_depth_limited(&self, value: bool) {
        self.is_depth_limited.store(value, Ordering::Release);
    }

    #[inline]
    pub fn is_depth_cutoff(&self) -> bool {
        self.depth_cutoff.load(Ordering::Acquire)
    }

    #[inline]
    pub fn set_depth_cutoff(&self, value: bool) {
        self.depth_cutoff.store(value, Ordering::Release);
    }

    #[inline]
    pub fn try_mark_depth_cutoff(&self) -> bool {
        self.depth_cutoff
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
    }

    #[inline]
    pub fn set_pn(&self, value: u64) {
        self.pn.store(value, Ordering::Release);
    }

    #[inline]
    pub fn set_dn(&self, value: u64) {
        self.dn.store(value, Ordering::Release);
    }

    #[inline]
    pub fn set_win_len(&self, value: u64) {
        self.win_len.store(value, Ordering::Release);
    }

    #[inline]
    pub fn add_virtual_pressure(&self, vpn: u64, vdn: u64) {
        self.virtual_pn.fetch_add(vpn, Ordering::AcqRel);
        self.virtual_dn.fetch_add(vdn, Ordering::AcqRel);
    }

    #[inline]
    pub fn remove_virtual_pressure(&self, vpn: u64, vdn: u64) {
        self.virtual_pn.fetch_sub(vpn, Ordering::AcqRel);
        self.virtual_dn.fetch_sub(vdn, Ordering::AcqRel);
    }

    pub fn set_proven(&self) {
        self.set_pn(0);
        self.set_dn(u64::MAX);
    }

    pub fn set_disproven(&self) {
        self.set_pn(u64::MAX);
        self.set_dn(0);
    }
}
const VIRTUAL_PRESSURE: u64 = 1;
pub struct Worker {
    pub tree: Arc<SharedTree>,
    pub ctx: ThreadLocalContext,
}
impl Worker {
    pub const fn new(tree: Arc<SharedTree>, ctx: ThreadLocalContext) -> Self {
        Self { tree, ctx }
    }

    pub fn run(&mut self) {
        while !self.tree.should_stop() {
            if self.tree.root.get_pn() == u64::MAX {
                self.tree.mark_solved();
                break;
            }
            self.tree.increment_iterations();
            self.one_iteration();
            let root = &self.tree.root;
            let pn = root.get_pn();
            let dn = root.get_dn();
            if pn == 0 || dn == 0 {
                self.tree.mark_solved();
                break;
            }
        }
    }

    fn one_iteration(&mut self) {
        self.ctx.clear_path();
        let root = Arc::clone(&self.tree.root);
        let leaf = self.select(root);
        if self.tree.should_stop() {
            self.backpropagate();
            return;
        }
        if let Some(leaf_node) = leaf
            && !leaf_node.is_terminal()
            && !leaf_node.is_expanded()
        {
            self.tree.expand_node(&leaf_node, &mut self.ctx);
            self.tree.update_node_pdn(&leaf_node);
        }
        self.backpropagate();
    }

    fn select(&mut self, start: NodeRef) -> Option<NodeRef> {
        let mut current = start;
        loop {
            if self.tree.should_stop() {
                return None;
            }
            if current.is_terminal() {
                return Some(current);
            }
            if !current.is_expanded() {
                return Some(current);
            }
            let Some(ChildRef {
                node: best_child,
                mov,
            }) = self.tree.select_best_child(&current)
            else {
                return Some(current);
            };
            if best_child.is_terminal() {
                return Some(best_child);
            }
            let player = current.player;
            best_child.add_virtual_pressure(VIRTUAL_PRESSURE, VIRTUAL_PRESSURE);
            self.ctx.make_move(mov, player);
            self.ctx.push_path(
                Arc::clone(&best_child),
                mov,
                player,
                VIRTUAL_PRESSURE,
                VIRTUAL_PRESSURE,
            );
            current = best_child;
        }
    }

    fn backpropagate(&mut self) {
        while let Some(entry) = self.ctx.pop_path() {
            self.ctx.undo_move(entry.mov, entry.player);
            entry
                .node
                .remove_virtual_pressure(entry.virtual_pn_added, entry.virtual_dn_added);
            self.tree.update_node_pdn(&entry.node);
        }
        self.tree.update_node_pdn(&self.tree.root);
    }
}

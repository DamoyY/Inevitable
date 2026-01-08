use parking_lot::RwLock;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;

pub type NodeRef = Arc<ParallelNode>;

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

    pub children: RwLock<Option<Vec<ChildRef>>>,
    pub is_depth_limited: AtomicBool,
    pub depth_cutoff: AtomicBool,
}

impl ParallelNode {
    pub fn new(
        player: u8,
        depth: usize,
        hash: u64,
        is_depth_limited: bool,
    ) -> Self {
        Self {
            player,
            depth,
            hash,
            pn: AtomicU64::new(1),
            dn: AtomicU64::new(1),
            virtual_pn: AtomicU64::new(0),
            virtual_dn: AtomicU64::new(0),
            win_len: AtomicU64::new(u64::MAX),
            children: RwLock::new(None),
            is_depth_limited: AtomicBool::new(is_depth_limited),
            depth_cutoff: AtomicBool::new(false),
        }
    }

    #[inline]
    pub fn is_or_node(&self) -> bool {
        self.player == 1
    }

    #[inline]
    pub fn is_expanded(&self) -> bool {
        self.children.read().is_some()
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

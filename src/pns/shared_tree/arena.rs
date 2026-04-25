use super::{
    super::{
        TreeStatsAtomic, TreeStatsSnapshot,
        node::{NodeRef, ParallelNode},
    },
    NodeTable, ShardedMap, TranspositionTable,
};
use crate::checked;
use crate::pns::TTEntry;
use alloc::sync::Arc;
use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};
static NEXT_STATS_SESSION_ID: AtomicU64 = AtomicU64::new(1_u64);
pub(crate) struct SharedTree {
    pub(crate) root: NodeRef,
    pub(crate) transposition_table: TranspositionTable,
    pub(crate) node_table: NodeTable,
    pub(crate) depth_limit: Option<usize>,
    pub(crate) solved: AtomicBool,
    pub(crate) stop_flag: Arc<AtomicBool>,
    pub(crate) stats: TreeStatsAtomic,
    stats_session_id: u64,
}
fn next_stats_session_id() -> u64 {
    loop {
        let current = NEXT_STATS_SESSION_ID.load(Ordering::Relaxed);
        let next = checked::add_u64(current, 1_u64, "SharedTree::next_stats_session_id");
        match NEXT_STATS_SESSION_ID.compare_exchange_weak(
            current,
            next,
            Ordering::Relaxed,
            Ordering::Relaxed,
        ) {
            Ok(_) => return current,
            Err(_) => core::hint::spin_loop(),
        }
    }
}
impl SharedTree {
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
        let stats_session_id = next_stats_session_id();
        Self {
            root,
            transposition_table,
            node_table,
            depth_limit,
            solved: AtomicBool::new(false),
            stop_flag,
            stats,
            stats_session_id,
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
    pub fn stats_snapshot(&self) -> TreeStatsSnapshot {
        self.stats.snapshot()
    }
    #[inline]
    #[must_use]
    pub const fn stats_session_id(&self) -> u64 {
        self.stats_session_id
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
}

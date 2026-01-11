use std::{
    hash::Hash,
    sync::{Arc, atomic::Ordering},
};

use ahash::RandomState;
use hashbrown::HashMap;
use parking_lot::RwLock;

use super::node::NodeRef;
use crate::pns::{TTEntry, parallel::TreeStatsSnapshot};
pub(super) use crate::utils::duration_to_ns;
mod evaluation;
mod expansion;
mod tree;
mod update;
pub use tree::SharedTree;
const SHARD_COUNT: usize = 64;
pub struct ShardedMap<K, V> {
    shards: Vec<RwLock<HashMap<K, V, RandomState>>>,
    hasher: RandomState,
}
impl<K: Hash + Eq, V: Clone> ShardedMap<K, V> {
    pub fn new() -> Self {
        debug_assert!(SHARD_COUNT.is_power_of_two());
        let hasher = RandomState::new();
        let mut shards = Vec::with_capacity(SHARD_COUNT);
        for _ in 0..SHARD_COUNT {
            shards.push(RwLock::new(HashMap::with_hasher(hasher.clone())));
        }
        Self { shards, hasher }
    }

    pub fn clear(&self) {
        for shard in &self.shards {
            shard.write().clear();
        }
    }

    pub fn get(&self, key: &K) -> Option<V> {
        let idx = self.shard_index(key);
        let guard = self.shards[idx].read();
        guard.get(key).cloned()
    }

    pub fn insert(&self, key: K, value: V) {
        let idx = self.shard_index(&key);
        let mut guard = self.shards[idx].write();
        guard.insert(key, value);
    }

    pub fn len(&self) -> usize {
        self.shards.iter().map(|shard| shard.read().len()).sum()
    }

    fn shard_index(&self, key: &K) -> usize {
        let hash = self.hasher.hash_one(key);
        let mask = (self.shards.len() - 1) as u64;
        usize::try_from(hash & mask).expect("shard index fits usize")
    }
}
impl<K: Hash + Eq, V: Clone> Default for ShardedMap<K, V> {
    fn default() -> Self {
        Self::new()
    }
}
pub type TranspositionTable = Arc<ShardedMap<(u64, u8), TTEntry>>;
pub type NodeTable = Arc<ShardedMap<(u64, usize), NodeRef>>;
impl SharedTree {
    pub fn increment_iterations(&self) {
        self.stats.iterations.fetch_add(1, Ordering::Relaxed);
    }

    pub fn increment_expansions(&self) {
        self.stats.expansions.fetch_add(1, Ordering::Relaxed);
    }

    #[must_use]
    pub fn stats_snapshot(&self) -> TreeStatsSnapshot {
        self.stats.snapshot()
    }

    pub fn get_tt(&self) -> TranspositionTable {
        Arc::clone(&self.transposition_table)
    }

    pub fn get_node_table(&self) -> NodeTable {
        Arc::clone(&self.node_table)
    }

    pub fn get_tt_size(&self) -> usize {
        self.transposition_table.len()
    }

    pub fn get_node_table_size(&self) -> usize {
        self.node_table.len()
    }

    pub fn lookup_tt(&self, hash: u64, player: u8) -> Option<TTEntry> {
        self.stats.tt_lookups.fetch_add(1, Ordering::Relaxed);
        let entry = self.transposition_table.get(&(hash, player));
        if entry.is_some() {
            self.stats.tt_hits.fetch_add(1, Ordering::Relaxed);
        }
        entry
    }

    pub fn store_tt(&self, hash: u64, player: u8, entry: TTEntry) {
        self.transposition_table.insert((hash, player), entry);
        self.stats.tt_stores.fetch_add(1, Ordering::Relaxed);
    }
}

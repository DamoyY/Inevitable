use std::{
    hash::Hash,
    sync::Arc,
};

use ahash::RandomState;
use hashbrown::HashMap;
use parking_lot::RwLock;

use super::node::NodeRef;
use crate::pns::TTEntry;
pub(super) use crate::utils::duration_to_ns;

mod evaluation;
mod expansion;
mod stats;
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

use super::node::NodeRef;
use crate::checked;
use crate::pns::TTEntry;
use ahash::RandomState;
use alloc::sync::Arc;
use core::hash::Hash;
use hashbrown::HashMap;
use parking_lot::RwLock;
pub(crate) mod tree;
const SHARD_COUNT: usize = 64;
pub struct ShardedMap<K, V> {
    shards: Vec<RwLock<HashMap<K, V, RandomState>>>,
    hasher: RandomState,
}
impl<K: Hash + Eq, V: Clone> ShardedMap<K, V> {
    pub fn new() -> Self {
        debug_assert!(SHARD_COUNT.is_power_of_two(), "分片数量必须是 2 的幂");
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
        let guard = self.shard(idx).read();
        guard.get(key).cloned()
    }
    pub fn insert(&self, key: K, value: V) {
        let idx = self.shard_index(&key);
        let mut guard = self.shard(idx).write();
        guard.insert(key, value);
    }
    pub fn len(&self) -> usize {
        self.shards.iter().map(|shard| shard.read().len()).sum()
    }
    fn shard_index(&self, key: &K) -> usize {
        let hash = self.hasher.hash_one(key);
        let last_shard_index =
            checked::sub_usize(self.shards.len(), 1_usize, "ShardedMap::shard_index::mask");
        let mask = checked::usize_to_u64(last_shard_index, "ShardedMap::shard_index::mask");
        checked::u64_to_usize(hash & mask, "ShardedMap::shard_index")
    }
    fn shard(&self, index: usize) -> &RwLock<HashMap<K, V, RandomState>> {
        let Some(shard) = self.shards.get(index) else {
            eprintln!("ShardedMap 分片索引越界: {index}");
            panic!("ShardedMap 分片索引越界");
        };
        shard
    }
}
impl<K: Hash + Eq, V: Clone> Default for ShardedMap<K, V> {
    fn default() -> Self {
        Self::new()
    }
}
pub type TranspositionTable = Arc<ShardedMap<(u64, u8), TTEntry>>;
pub type NodeTable = Arc<ShardedMap<(u64, usize), NodeRef>>;

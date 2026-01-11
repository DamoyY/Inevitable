use std::sync::{Arc, atomic::Ordering};

use super::{NodeTable, SharedTree, TranspositionTable};
use crate::pns::{TTEntry, parallel::TreeStatsSnapshot};

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

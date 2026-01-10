use std::sync::{Arc, atomic::Ordering};

use super::{NodeTable, SharedTree, TranspositionTable};
use crate::pns::TTEntry;

impl SharedTree {
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

    pub fn get_board_update_time_ns(&self) -> u64 {
        self.total_board_update_time_ns.load(Ordering::Relaxed)
    }

    pub fn get_bitboard_update_time_ns(&self) -> u64 {
        self.total_bitboard_update_time_ns.load(Ordering::Relaxed)
    }

    pub fn get_threat_index_update_time_ns(&self) -> u64 {
        self.total_threat_index_update_time_ns
            .load(Ordering::Relaxed)
    }

    pub fn get_candidate_remove_time_ns(&self) -> u64 {
        self.total_candidate_remove_time_ns.load(Ordering::Relaxed)
    }

    pub fn get_candidate_neighbor_time_ns(&self) -> u64 {
        self.total_candidate_neighbor_time_ns.load(Ordering::Relaxed)
    }

    pub fn get_candidate_insert_time_ns(&self) -> u64 {
        self.total_candidate_insert_time_ns.load(Ordering::Relaxed)
    }

    pub fn get_candidate_newly_added_time_ns(&self) -> u64 {
        self.total_candidate_newly_added_time_ns
            .load(Ordering::Relaxed)
    }

    pub fn get_candidate_history_time_ns(&self) -> u64 {
        self.total_candidate_history_time_ns.load(Ordering::Relaxed)
    }

    pub fn get_hash_update_time_ns(&self) -> u64 {
        self.total_hash_update_time_ns.load(Ordering::Relaxed)
    }

    pub fn get_move_undo_time_ns(&self) -> u64 {
        self.total_move_undo_time_ns.load(Ordering::Relaxed)
    }

    pub fn get_hash_time_ns(&self) -> u64 {
        self.total_hash_time_ns.load(Ordering::Relaxed)
    }

    pub fn get_children_lock_time_ns(&self) -> u64 {
        self.total_children_lock_time_ns
            .load(Ordering::Relaxed)
    }

    pub fn get_node_table_lookup_time_ns(&self) -> u64 {
        self.total_node_table_lookup_time_ns.load(Ordering::Relaxed)
    }

    pub fn get_node_table_write_time_ns(&self) -> u64 {
        self.total_node_table_write_time_ns.load(Ordering::Relaxed)
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

    pub fn get_node_table(&self) -> NodeTable {
        Arc::clone(&self.node_table)
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

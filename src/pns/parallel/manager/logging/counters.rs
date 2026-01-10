use super::super::metrics::TimingInput;
use crate::pns::parallel::SharedTree;

pub(super) struct LogCounters {
    pub(super) iterations: u64,
    pub(super) expansions: u64,
    pub(super) children_generated: u64,
    pub(super) expand_ns: u64,
    pub(super) movegen_ns: u64,
    pub(super) board_update_ns: u64,
    pub(super) bitboard_update_ns: u64,
    pub(super) threat_index_update_ns: u64,
    pub(super) candidate_update_ns: u64,
    pub(super) hash_update_ns: u64,
    pub(super) move_undo_ns: u64,
    pub(super) hash_ns: u64,
    pub(super) eval_ns: u64,
    pub(super) eval_calls: u64,
    pub(super) tt_lookups: u64,
    pub(super) tt_hits: u64,
    pub(super) node_table_lookups: u64,
    pub(super) node_table_hits: u64,
    pub(super) node_table_lookup_time_ns: u64,
    pub(super) node_table_write_time_ns: u64,
    pub(super) nodes_created: u64,
}

impl LogCounters {
    pub(super) fn from_tree(tree: &SharedTree) -> Self {
        Self {
            iterations: tree.get_iterations(),
            expansions: tree.get_expansions(),
            children_generated: tree.get_children_generated(),
            expand_ns: tree.get_expand_time_ns(),
            movegen_ns: tree.get_movegen_time_ns(),
            board_update_ns: tree.get_board_update_time_ns(),
            bitboard_update_ns: tree.get_bitboard_update_time_ns(),
            threat_index_update_ns: tree.get_threat_index_update_time_ns(),
            candidate_update_ns: tree.get_candidate_update_time_ns(),
            hash_update_ns: tree.get_hash_update_time_ns(),
            move_undo_ns: tree.get_move_undo_time_ns(),
            hash_ns: tree.get_hash_time_ns(),
            eval_ns: tree.get_eval_time_ns(),
            eval_calls: tree.get_eval_calls(),
            tt_lookups: tree.get_tt_lookups(),
            tt_hits: tree.get_tt_hits(),
            node_table_lookups: tree.get_node_table_lookups(),
            node_table_hits: tree.get_node_table_hits(),
            node_table_lookup_time_ns: tree.get_node_table_lookup_time_ns(),
            node_table_write_time_ns: tree.get_node_table_write_time_ns(),
            nodes_created: tree.get_nodes_created(),
        }
    }

    pub(super) const fn timing_input(&self) -> TimingInput {
        TimingInput {
            expansions: self.expansions,
            children_generated: self.children_generated,
            expand_ns: self.expand_ns,
            movegen_ns: self.movegen_ns,
            board_update_ns: self.board_update_ns,
            bitboard_update_ns: self.bitboard_update_ns,
            threat_index_update_ns: self.threat_index_update_ns,
            candidate_update_ns: self.candidate_update_ns,
            hash_update_ns: self.hash_update_ns,
            move_undo_ns: self.move_undo_ns,
            hash_ns: self.hash_ns,
            node_table_lookup_ns: self.node_table_lookup_time_ns,
            node_table_write_ns: self.node_table_write_time_ns,
            eval_ns: self.eval_ns,
            eval_calls: self.eval_calls,
        }
    }
}

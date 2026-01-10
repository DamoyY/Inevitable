use super::super::metrics::TimingInput;
use crate::pns::parallel::SharedTree;

const fn saturating_diff(current: u64, previous: u64) -> u64 {
    current.saturating_sub(previous)
}

pub(super) struct LogCounters {
    pub(super) iterations: u64,
    pub(super) expansions: u64,
    pub(super) children_generated: u64,
    pub(super) expand_ns: u64,
    pub(super) movegen_ns: u64,
    pub(super) move_make_ns: u64,
    pub(super) move_undo_ns: u64,
    pub(super) hash_ns: u64,
    pub(super) eval_ns: u64,
    pub(super) eval_calls: u64,
    pub(super) tt_lookups: u64,
    pub(super) tt_hits: u64,
    pub(super) node_table_lookups: u64,
    pub(super) node_table_hits: u64,
    pub(super) node_table_time_ns: u64,
    pub(super) nodes_created: u64,
}

impl LogCounters {
    pub(super) const fn zero() -> Self {
        Self {
            iterations: 0,
            expansions: 0,
            children_generated: 0,
            expand_ns: 0,
            movegen_ns: 0,
            move_make_ns: 0,
            move_undo_ns: 0,
            hash_ns: 0,
            eval_ns: 0,
            eval_calls: 0,
            tt_lookups: 0,
            tt_hits: 0,
            node_table_lookups: 0,
            node_table_hits: 0,
            node_table_time_ns: 0,
            nodes_created: 0,
        }
    }

    pub(super) fn from_tree(tree: &SharedTree) -> Self {
        Self {
            iterations: tree.get_iterations(),
            expansions: tree.get_expansions(),
            children_generated: tree.get_children_generated(),
            expand_ns: tree.get_expand_time_ns(),
            movegen_ns: tree.get_movegen_time_ns(),
            move_make_ns: tree.get_move_make_time_ns(),
            move_undo_ns: tree.get_move_undo_time_ns(),
            hash_ns: tree.get_hash_time_ns(),
            eval_ns: tree.get_eval_time_ns(),
            eval_calls: tree.get_eval_calls(),
            tt_lookups: tree.get_tt_lookups(),
            tt_hits: tree.get_tt_hits(),
            node_table_lookups: tree.get_node_table_lookups(),
            node_table_hits: tree.get_node_table_hits(),
            node_table_time_ns: tree.get_node_table_time_ns(),
            nodes_created: tree.get_nodes_created(),
        }
    }

    pub(super) const fn diff(current: &Self, previous: &Self) -> Self {
        Self {
            iterations: saturating_diff(current.iterations, previous.iterations),
            expansions: saturating_diff(current.expansions, previous.expansions),
            children_generated: saturating_diff(
                current.children_generated,
                previous.children_generated,
            ),
            expand_ns: saturating_diff(current.expand_ns, previous.expand_ns),
            movegen_ns: saturating_diff(current.movegen_ns, previous.movegen_ns),
            move_make_ns: saturating_diff(current.move_make_ns, previous.move_make_ns),
            move_undo_ns: saturating_diff(current.move_undo_ns, previous.move_undo_ns),
            hash_ns: saturating_diff(current.hash_ns, previous.hash_ns),
            eval_ns: saturating_diff(current.eval_ns, previous.eval_ns),
            eval_calls: saturating_diff(current.eval_calls, previous.eval_calls),
            tt_lookups: saturating_diff(current.tt_lookups, previous.tt_lookups),
            tt_hits: saturating_diff(current.tt_hits, previous.tt_hits),
            node_table_lookups: saturating_diff(
                current.node_table_lookups,
                previous.node_table_lookups,
            ),
            node_table_hits: saturating_diff(current.node_table_hits, previous.node_table_hits),
            node_table_time_ns: saturating_diff(
                current.node_table_time_ns,
                previous.node_table_time_ns,
            ),
            nodes_created: saturating_diff(current.nodes_created, previous.nodes_created),
        }
    }

    pub(super) const fn timing_input(&self) -> TimingInput {
        TimingInput {
            expansions: self.expansions,
            children_generated: self.children_generated,
            expand_ns: self.expand_ns,
            movegen_ns: self.movegen_ns,
            move_make_ns: self.move_make_ns,
            move_undo_ns: self.move_undo_ns,
            hash_ns: self.hash_ns,
            node_table_ns: self.node_table_time_ns,
            eval_ns: self.eval_ns,
            eval_calls: self.eval_calls,
        }
    }
}

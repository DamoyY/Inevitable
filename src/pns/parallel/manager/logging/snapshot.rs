use super::counters::LogCounters;
use crate::pns::parallel::SharedTree;

pub(super) struct LogSnapshot {
    pub(super) counters: LogCounters,
    pub(super) tt_stores: u64,
    pub(super) tt_size: usize,
    pub(super) node_table_size: usize,
    pub(super) depth_limit: Option<usize>,
    pub(super) depth_cutoffs: u64,
    pub(super) early_cutoffs: u64,
}

pub(super) fn capture_snapshot(tree: &SharedTree) -> LogSnapshot {
    LogSnapshot {
        counters: LogCounters::from_tree(tree),
        tt_stores: tree.get_tt_stores(),
        tt_size: tree.get_tt_size(),
        node_table_size: tree.get_node_table_size(),
        depth_limit: tree.depth_limit,
        depth_cutoffs: tree.get_depth_cutoffs(),
        early_cutoffs: tree.get_early_cutoffs(),
    }
}

use crate::{
    alloc_stats,
    pns::parallel::{SharedTree, TreeStatsSnapshot},
};

pub(super) struct LogSnapshot {
    pub(super) stats: TreeStatsSnapshot,
    pub(super) alloc_free_ns: u64,
    pub(super) tt_size: usize,
    pub(super) node_table_size: usize,
    pub(super) depth_limit: Option<usize>,
}

pub(super) fn capture_snapshot(tree: &SharedTree) -> LogSnapshot {
    LogSnapshot {
        stats: tree.stats_snapshot(),
        alloc_free_ns: alloc_stats::alloc_free_time_ns(),
        tt_size: tree.get_tt_size(),
        node_table_size: tree.get_node_table_size(),
        depth_limit: tree.depth_limit,
    }
}

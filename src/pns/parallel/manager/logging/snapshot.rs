use crate::pns::parallel::{SharedTree, TreeStatsSnapshot};

pub(super) struct LogSnapshot {
    pub(super) stats: TreeStatsSnapshot,
    pub(super) tt_size: usize,
    pub(super) node_table_size: usize,
    pub(super) depth_limit: Option<usize>,
}

pub(super) fn capture_snapshot(tree: &SharedTree) -> LogSnapshot {
    LogSnapshot {
        stats: tree.stats_snapshot(),
        tt_size: tree.get_tt_size(),
        node_table_size: tree.get_node_table_size(),
        depth_limit: tree.depth_limit,
    }
}

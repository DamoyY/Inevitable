use std::time::Instant;

use super::counters::LogCounters;
use crate::pns::parallel::SharedTree;

pub(super) struct LogSnapshot {
    pub(super) counters: LogCounters,
    pub(super) tt_stores: u64,
    pub(super) root_pn: u64,
    pub(super) root_dn: u64,
    pub(super) tt_size: usize,
    pub(super) node_table_size: usize,
    pub(super) depth_limit: Option<usize>,
    pub(super) depth_cutoffs: u64,
    pub(super) early_cutoffs: u64,
    pub(super) timestamp: Instant,
}

pub(super) struct LogDelta {
    pub(super) counters: LogCounters,
    pub(super) elapsed_secs: f64,
}

impl LogSnapshot {
    pub(super) fn zero() -> Self {
        Self {
            counters: LogCounters::zero(),
            tt_stores: 0,
            root_pn: 0,
            root_dn: 0,
            tt_size: 0,
            node_table_size: 0,
            depth_limit: None,
            depth_cutoffs: 0,
            early_cutoffs: 0,
            timestamp: Instant::now(),
        }
    }
}

pub(super) fn capture_snapshot(tree: &SharedTree) -> LogSnapshot {
    LogSnapshot {
        counters: LogCounters::from_tree(tree),
        tt_stores: tree.get_tt_stores(),
        root_pn: tree.root.get_pn(),
        root_dn: tree.root.get_dn(),
        tt_size: tree.get_tt_size(),
        node_table_size: tree.get_node_table_size(),
        depth_limit: tree.depth_limit,
        depth_cutoffs: tree.get_depth_cutoffs(),
        early_cutoffs: tree.get_early_cutoffs(),
        timestamp: Instant::now(),
    }
}

pub(super) fn compute_delta(current: &LogSnapshot, previous: &LogSnapshot) -> LogDelta {
    LogDelta {
        counters: LogCounters::diff(&current.counters, &previous.counters),
        elapsed_secs: current
            .timestamp
            .duration_since(previous.timestamp)
            .as_secs_f64(),
    }
}

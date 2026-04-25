mod context;
mod manager;
mod node;
mod shared_tree;
mod stats_def;
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct TTEntry {
    pub pn: u64,
    pub dn: u64,
    pub win_len: u64,
}
pub type ParallelSolver = manager::ParallelSolver;
pub type SearchParams = manager::SearchParams;
pub type NodeTable = shared_tree::NodeTable;
pub(crate) type SharedTree = shared_tree::SharedTree;
pub type TranspositionTable = shared_tree::TranspositionTable;
pub(crate) type TimingStats = stats_def::TimingStats;
pub(crate) type TreeStatsAccumulator = stats_def::TreeStatsAccumulator;
pub(crate) type TreeStatsAtomic = stats_def::TreeStatsAtomic;
pub(crate) type TreeStatsSnapshot = stats_def::TreeStatsSnapshot;

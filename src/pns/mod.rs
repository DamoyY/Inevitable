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
pub use manager::{ParallelSolver, SearchParams};
pub use node::{NodeRef, ParallelNode, Worker};
pub use shared_tree::{NodeTable, SharedTree, TranspositionTable};
pub(crate) use stats_def::{TimingStats, TreeStatsAccumulator, TreeStatsAtomic, TreeStatsSnapshot};

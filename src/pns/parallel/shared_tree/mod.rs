use std::{collections::HashMap, sync::Arc};

use parking_lot::RwLock;

use super::node::NodeRef;
use crate::pns::TTEntry;

mod evaluation;
mod expansion;
mod stats;
mod tree;
mod update;

pub use tree::SharedTree;

pub type TranspositionTable = Arc<RwLock<HashMap<(u64, u8), TTEntry>>>;
pub type NodeTable = Arc<RwLock<HashMap<(u64, usize), NodeRef>>>;

pub(super) fn duration_to_ns(duration: std::time::Duration) -> u64 {
    let nanos = duration.as_nanos();
    u64::try_from(nanos).unwrap_or(u64::MAX)
}

use std::{collections::HashMap, sync::Arc};

use parking_lot::RwLock;

use super::node::NodeRef;
use crate::pns::TTEntry;
pub(super) use crate::utils::duration_to_ns;

mod evaluation;
mod expansion;
mod stats;
mod tree;
mod update;

pub use tree::SharedTree;

pub type TranspositionTable = Arc<RwLock<HashMap<(u64, u8), TTEntry>>>;
pub type NodeTable = Arc<RwLock<HashMap<(u64, usize), NodeRef>>>;

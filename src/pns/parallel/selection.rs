use std::sync::Arc;

use super::{node::NodeRef, shared_tree::SharedTree};
impl SharedTree {
    pub fn select_best_child(&self, node: &NodeRef) -> Option<NodeRef> {
        let children = {
            let children_guard = node.children.read();
            children_guard.as_ref().cloned()?
        };
        if children.is_empty() {
            return None;
        }
        let is_or_node = node.is_or_node();
        let best = if is_or_node {
            children
                .iter()
                .min_by_key(|c| (c.node.get_effective_pn(), c.node.get_win_len()))
        } else {
            children
                .iter()
                .min_by_key(|c| (c.node.get_effective_dn(), c.node.get_win_len()))
        };
        best.map(|c| Arc::clone(&c.node))
    }
}

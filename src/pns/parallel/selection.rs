use super::{node::NodeRef, shared_tree::SharedTree};
use std::sync::Arc;
impl SharedTree {
    pub fn select_best_child(&self, node: &NodeRef) -> Option<NodeRef> {
        let children_guard = node.children.read();
        let children = children_guard.as_ref()?;
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

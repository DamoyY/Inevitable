use super::{node::NodeRef, shared_tree::SharedTree};
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
                .min_by_key(|c| (c.get_effective_pn(), c.get_win_len()))
        } else {
            children
                .iter()
                .min_by_key(|c| (c.get_effective_dn(), c.get_win_len()))
        };
        best.cloned()
    }
}

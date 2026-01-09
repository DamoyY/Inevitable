use super::{
    node::{ChildRef, NodeRef},
    shared_tree::SharedTree,
};
impl SharedTree {
    pub fn select_best_child(&self, node: &NodeRef) -> Option<ChildRef> {
        let children = {
            let children_guard = node.children.read();
            children_guard.as_ref().cloned()?
        };
        let is_or_node = node.is_or_node();
        children.into_iter().min_by_key(|c| {
            if is_or_node {
                (c.node.get_effective_pn(), c.node.get_win_len())
            } else {
                (c.node.get_effective_dn(), c.node.get_win_len())
            }
        })
    }
}

use super::{
    super::node::{ChildRef, NodeRef, ParallelNode},
    arena::SharedTree,
};
use alloc::{collections::VecDeque, sync::Arc};
use core::sync::atomic::Ordering;
use std::collections::HashSet;
impl SharedTree {
    fn push_unvisited_children<F>(
        node: &NodeRef,
        visited: &mut HashSet<*const ParallelNode>,
        mut push: F,
    ) where
        F: FnMut(NodeRef),
    {
        if let Some(children) = node.children.get() {
            for child in children {
                let ptr = Arc::as_ptr(&child.node);
                if visited.insert(ptr) {
                    push(Arc::clone(&child.node));
                }
            }
        }
    }
    #[inline]
    pub fn increase_depth_limit(&mut self, new_depth_limit: usize) {
        if let Some(current_limit) = self.depth_limit
            && new_depth_limit <= current_limit
        {
            return;
        }
        self.depth_limit = Some(new_depth_limit);
        self.solved.store(false, Ordering::Release);
        let mut queue_visited = HashSet::new();
        let mut queue = VecDeque::new();
        queue.push_back(Arc::clone(&self.root));
        queue_visited.insert(Arc::as_ptr(&self.root));
        while let Some(node) = queue.pop_front() {
            node.set_is_depth_limited(node.depth >= new_depth_limit);
            if node.is_depth_cutoff() && node.depth < new_depth_limit {
                node.set_depth_cutoff(false);
                node.set_pn(1);
                node.set_dn(1);
                node.set_win_len(u64::MAX);
            }
            Self::push_unvisited_children(&node, &mut queue_visited, |child| {
                queue.push_back(child);
            });
        }
        let mut stack = Vec::new();
        let mut postorder_visited = HashSet::new();
        let mut postorder = Vec::new();
        stack.push((Arc::clone(&self.root), false));
        postorder_visited.insert(Arc::as_ptr(&self.root));
        while let Some((node, processed)) = stack.pop() {
            if processed {
                postorder.push(node);
                continue;
            }
            stack.push((Arc::clone(&node), true));
            Self::push_unvisited_children(&node, &mut postorder_visited, |child| {
                stack.push((child, false));
            });
        }
        for node in postorder {
            self.update_node_pdn(&node);
        }
    }
    #[inline]
    pub fn select_best_child(node: &NodeRef) -> Option<ChildRef> {
        let children = node.children.get()?;
        let is_or_node = node.is_or_node();
        children
            .iter()
            .min_by_key(|child_ref| {
                if is_or_node {
                    (
                        child_ref.node.get_effective_pn(),
                        child_ref.node.get_win_len(),
                    )
                } else {
                    (
                        child_ref.node.get_effective_dn(),
                        child_ref.node.get_win_len(),
                    )
                }
            })
            .cloned()
    }
}

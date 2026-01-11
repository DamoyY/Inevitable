use std::sync::Arc;

use super::{
    context::ThreadLocalContext,
    node::{ChildRef, NodeRef},
    shared_tree::SharedTree,
};
const VIRTUAL_PRESSURE: u64 = 1;
pub struct Worker {
    pub tree: Arc<SharedTree>,
    pub ctx: ThreadLocalContext,
}
impl Worker {
    pub const fn new(tree: Arc<SharedTree>, ctx: ThreadLocalContext) -> Self {
        Self { tree, ctx }
    }

    pub fn run(&mut self) {
        while !self.tree.should_stop() {
            if self.tree.root.get_pn() == u64::MAX {
                self.tree.mark_solved();
                break;
            }
            self.tree.increment_iterations();
            self.one_iteration();
            let root = &self.tree.root;
            let pn = root.get_pn();
            let dn = root.get_dn();
            if pn == 0 || dn == 0 {
                self.tree.mark_solved();
                break;
            }
        }
    }

    fn one_iteration(&mut self) {
        self.ctx.clear_path();
        let root = Arc::clone(&self.tree.root);
        let leaf = self.select(root);
        if self.tree.should_stop() {
            self.backpropagate();
            return;
        }
        if let Some(leaf_node) = leaf
            && !leaf_node.is_terminal()
            && !leaf_node.is_expanded()
        {
            self.tree.expand_node(&leaf_node, &mut self.ctx);
            self.tree.update_node_pdn(&leaf_node);
        }
        self.backpropagate();
    }

    fn select(&mut self, start: NodeRef) -> Option<NodeRef> {
        let mut current = start;
        loop {
            if self.tree.should_stop() {
                return None;
            }
            if current.is_terminal() {
                return Some(current);
            }
            if !current.is_expanded() {
                return Some(current);
            }
            let Some(ChildRef {
                node: best_child,
                mov,
            }) = self.tree.select_best_child(&current)
            else {
                return Some(current);
            };
            if best_child.is_terminal() {
                return Some(best_child);
            }
            let player = current.player;
            best_child.add_virtual_pressure(VIRTUAL_PRESSURE, VIRTUAL_PRESSURE);
            self.ctx.make_move(mov, player);
            self.ctx.push_path(
                Arc::clone(&best_child),
                mov,
                player,
                VIRTUAL_PRESSURE,
                VIRTUAL_PRESSURE,
            );
            current = best_child;
        }
    }

    fn backpropagate(&mut self) {
        while let Some(entry) = self.ctx.pop_path() {
            self.ctx.undo_move(entry.mov, entry.player);
            entry
                .node
                .remove_virtual_pressure(entry.virtual_pn_added, entry.virtual_dn_added);
            self.tree.update_node_pdn(&entry.node);
        }
        self.tree.update_node_pdn(&self.tree.root);
    }
}

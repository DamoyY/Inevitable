use std::sync::Arc;

use super::{context::ThreadLocalContext, node::NodeRef, shared_tree::SharedTree};
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
        while !self.tree.is_solved() {
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
        if self.tree.is_solved() {
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
            if self.tree.is_solved() {
                return None;
            }

            if current.is_terminal() {
                return Some(current);
            }

            if !current.is_expanded() {
                return Some(current);
            }
            let selection = {
                let children_guard = current.children.read();
                children_guard.as_ref().map_or(None, |children| {
                    if children.is_empty() {
                        None
                    } else {
                        let is_or_node = current.is_or_node();
                        let best_child = if is_or_node {
                            children
                                .iter()
                                .min_by_key(|c| (c.node.get_effective_pn(), c.node.get_win_len()))
                        } else {
                            children
                                .iter()
                                .min_by_key(|c| (c.node.get_effective_dn(), c.node.get_win_len()))
                        };
                        best_child.map(|c| (Arc::clone(&c.node), c.mov))
                    }
                })
            };
            let Some((best_child, mov)) = selection else {
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
            self.ctx.undo_move(entry.mov);
            entry
                .node
                .remove_virtual_pressure(entry.virtual_pn_added, entry.virtual_dn_added);
            self.tree.update_node_pdn(&entry.node);
        }
        self.tree.update_node_pdn(&self.tree.root);
    }
}

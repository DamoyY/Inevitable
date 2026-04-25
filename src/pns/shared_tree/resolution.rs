use super::{super::node::NodeRef, arena::SharedTree};
use crate::{checked, pns::TTEntry};
impl SharedTree {
    #[inline]
    pub fn update_node_pdn(&self, node: &NodeRef) {
        let prev_proof = node.get_pn();
        let prev_disproof = node.get_dn();
        let prev_win_len = node.get_win_len();
        let Some(children) = node.children.get() else {
            if node.is_depth_limited() && node.is_depth_cutoff() {
                node.set_pn(u64::MAX);
                node.set_dn(u64::MAX);
                node.set_win_len(u64::MAX);
                self.store_tt_if_changed(node, prev_proof, prev_disproof, prev_win_len);
            }
            return;
        };
        if node.is_depth_limited() && children.is_empty() {
            node.set_pn(u64::MAX);
            node.set_dn(u64::MAX);
            node.set_win_len(u64::MAX);
            self.store_tt_if_changed(node, prev_proof, prev_disproof, prev_win_len);
            return;
        }
        if children.is_empty() {
            if node.is_or_node() {
                node.set_pn(u64::MAX);
                node.set_dn(0);
                node.set_win_len(u64::MAX);
            } else {
                node.set_pn(0);
                node.set_dn(u64::MAX);
                node.set_win_len(0);
            }
            self.store_tt_if_changed(node, prev_proof, prev_disproof, prev_win_len);
            return;
        }
        let is_or_node = node.is_or_node();
        let mut pn_min = u64::MAX;
        let mut pn_sum = 0_u64;
        let mut dn_min = u64::MAX;
        let mut dn_sum = 0_u64;
        let mut min_proven_win_len = u64::MAX;
        let mut max_proven_win_len = 0_u64;
        let mut all_children_proven = true;
        for child in children {
            let cpn = child.node.get_pn();
            let cdn = child.node.get_dn();
            let cwl = child.node.get_win_len();
            pn_min = pn_min.min(cpn);
            pn_sum = sum_with_infinity(pn_sum, cpn, "SharedTree::update_node_pdn::pn_sum");
            dn_min = dn_min.min(cdn);
            dn_sum = sum_with_infinity(dn_sum, cdn, "SharedTree::update_node_pdn::dn_sum");
            if cpn == 0 {
                min_proven_win_len = min_proven_win_len.min(cwl);
                max_proven_win_len = max_proven_win_len.max(cwl);
            } else {
                all_children_proven = false;
            }
        }
        if is_or_node {
            node.set_pn(pn_min);
            node.set_dn(dn_sum);
            if min_proven_win_len < u64::MAX {
                node.set_win_len(next_win_len(
                    min_proven_win_len,
                    "SharedTree::update_node_pdn::or_win_len",
                ));
            } else {
                node.set_win_len(u64::MAX);
            }
        } else {
            node.set_pn(pn_sum);
            node.set_dn(dn_min);
            if dn_min == 0 {
                node.set_win_len(u64::MAX);
            } else if all_children_proven {
                node.set_win_len(next_win_len(
                    max_proven_win_len,
                    "SharedTree::update_node_pdn::and_win_len",
                ));
            } else {
                node.set_win_len(u64::MAX);
            }
        }
        self.store_tt_if_changed(node, prev_proof, prev_disproof, prev_win_len);
    }
    fn store_tt_if_changed(
        &self,
        node: &NodeRef,
        prev_proof: u64,
        prev_disproof: u64,
        prev_win_len: u64,
    ) {
        if node.is_depth_limited() {
            return;
        }
        let pn = node.get_pn();
        let dn = node.get_dn();
        if pn == u64::MAX && dn == u64::MAX {
            return;
        }
        let win_len = node.get_win_len();
        if pn == prev_proof && dn == prev_disproof && win_len == prev_win_len {
            return;
        }
        self.store_tt(node.hash, node.player, TTEntry { pn, dn, win_len });
    }
}
fn sum_with_infinity(left: u64, right: u64, context: &str) -> u64 {
    if left == u64::MAX || right == u64::MAX {
        return u64::MAX;
    }
    checked::add_u64(left, right, context)
}
fn next_win_len(current: u64, context: &str) -> u64 {
    checked::add_u64(1_u64, current, context)
}

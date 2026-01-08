use crate::pns::parallel::node::NodeRef;

use super::SharedTree;

impl SharedTree {
    pub fn update_node_pdn(&self, node: &NodeRef) {
        let old_pn = node.get_pn();
        let old_dn = node.get_dn();
        let old_win_len = node.get_win_len();
        let children_guard = node.children.read();
        let children = match children_guard.as_ref() {
            Some(c) => c,
            None => return,
        };

        if node.is_depth_limited() && children.is_empty() {
            node.set_pn(1);
            node.set_dn(1);
            node.set_win_len(u64::MAX);
            self.store_tt_if_changed(node, old_pn, old_dn, old_win_len);
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
            self.store_tt_if_changed(node, old_pn, old_dn, old_win_len);
            return;
        }

        let is_or_node = node.is_or_node();

        if is_or_node {
            let mut min_pn = u64::MAX;
            let mut sum_dn = 0u64;
            let mut min_proven_win_len = u64::MAX;

            for child in children.iter() {
                let cpn = child.node.get_pn();
                let cdn = child.node.get_dn();
                let cwl = child.node.get_win_len();

                if cpn < min_pn {
                    min_pn = cpn;
                }
                sum_dn = sum_dn.saturating_add(cdn);

                if cpn == 0 && cwl < min_proven_win_len {
                    min_proven_win_len = cwl;
                }
            }

            node.set_pn(min_pn);
            node.set_dn(sum_dn);

            if min_proven_win_len < u64::MAX {
                node.set_win_len(1u64.saturating_add(min_proven_win_len));
            } else {
                node.set_win_len(u64::MAX);
            }
        } else {
            let mut sum_pn = 0u64;
            let mut min_dn = u64::MAX;
            let mut all_proven = true;
            let mut max_win_len = 0u64;

            for child in children.iter() {
                let cpn = child.node.get_pn();
                let cdn = child.node.get_dn();
                let cwl = child.node.get_win_len();

                sum_pn = sum_pn.saturating_add(cpn);
                if cdn < min_dn {
                    min_dn = cdn;
                }

                if cpn != 0 {
                    all_proven = false;
                } else if cwl > max_win_len {
                    max_win_len = cwl;
                }
            }

            node.set_pn(sum_pn);
            node.set_dn(min_dn);

            if min_dn == 0 {
                node.set_win_len(u64::MAX);
            } else if all_proven {
                node.set_win_len(1u64.saturating_add(max_win_len));
            } else {
                node.set_win_len(u64::MAX);
            }
        }

        self.store_tt_if_changed(node, old_pn, old_dn, old_win_len);
    }

    fn store_tt_if_changed(
        &self,
        node: &NodeRef,
        old_pn: u64,
        old_dn: u64,
        old_win_len: u64,
    ) {
        if node.is_depth_limited() {
            return;
        }
        let pn = node.get_pn();
        let dn = node.get_dn();
        let win_len = node.get_win_len();
        if pn == old_pn && dn == old_dn && win_len == old_win_len {
            return;
        }
        self.store_tt(
            node.hash,
            node.player,
            crate::pns::TTEntry {
                pn,
                dn,
                win_len,
            },
        );
    }
}

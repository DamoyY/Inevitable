use super::{PNSSolver, TTEntry};

impl PNSSolver {
    pub(crate) fn update_node_pdn(&mut self, node_idx: usize) {
        let node = &self.nodes[node_idx];
        let old_pn = node.pn;
        let old_dn = node.dn;
        let old_win_len = node.win_len;

        if node.is_depth_limited && node.children.is_empty() {
            self.nodes[node_idx].pn = 1;
            self.nodes[node_idx].dn = 1;
            self.nodes[node_idx].win_len = u64::MAX;
            self.store_tt_if_changed(node_idx, old_pn, old_dn, old_win_len);
            return;
        }

        if !node.is_expanded {
            return;
        }

        if node.children.is_empty() {
            if node.is_or_node() {
                self.nodes[node_idx].pn = u64::MAX;
                self.nodes[node_idx].dn = 0;
                self.nodes[node_idx].win_len = u64::MAX;
            } else {
                self.nodes[node_idx].pn = 0;
                self.nodes[node_idx].dn = u64::MAX;
                self.nodes[node_idx].win_len = 0;
            }
            self.store_tt_if_changed(node_idx, old_pn, old_dn, old_win_len);
            return;
        }

        let children = self.nodes[node_idx].children.clone();
        let is_or_node = self.nodes[node_idx].is_or_node();

        if is_or_node {
            let min_pn = children
                .iter()
                .map(|&idx| self.nodes[idx].pn)
                .min()
                .unwrap_or(u64::MAX);
            let sum_dn = children
                .iter()
                .map(|&idx| self.nodes[idx].dn)
                .fold(0u64, |acc, x| acc.saturating_add(x));

            self.nodes[node_idx].pn = min_pn;
            self.nodes[node_idx].dn = sum_dn;

            let proven_children: Vec<usize> = children
                .iter()
                .filter(|&&idx| self.nodes[idx].pn == 0)
                .copied()
                .collect();

            if !proven_children.is_empty() {
                let min_win_len = proven_children
                    .iter()
                    .map(|&idx| self.nodes[idx].win_len)
                    .min()
                    .unwrap_or(u64::MAX);
                self.nodes[node_idx].win_len = 1u64.saturating_add(min_win_len);
            } else {
                self.nodes[node_idx].win_len = u64::MAX;
            }
        } else {
            let sum_pn = children
                .iter()
                .map(|&idx| self.nodes[idx].pn)
                .fold(0u64, |acc, x| acc.saturating_add(x));
            let min_dn = children
                .iter()
                .map(|&idx| self.nodes[idx].dn)
                .min()
                .unwrap_or(u64::MAX);

            self.nodes[node_idx].pn = sum_pn;
            self.nodes[node_idx].dn = min_dn;

            if min_dn == 0 {
                self.nodes[node_idx].win_len = u64::MAX;
            } else if children.iter().all(|&idx| self.nodes[idx].pn == 0) {
                let max_win_len = children
                    .iter()
                    .map(|&idx| self.nodes[idx].win_len)
                    .max()
                    .unwrap_or(0);
                self.nodes[node_idx].win_len = 1u64.saturating_add(max_win_len);
            } else {
                self.nodes[node_idx].win_len = u64::MAX;
            }
        }

        self.store_tt_if_changed(node_idx, old_pn, old_dn, old_win_len);
    }

    fn store_tt_if_changed(
        &mut self,
        node_idx: usize,
        old_pn: u64,
        old_dn: u64,
        old_win_len: u64,
    ) {
        let node = &self.nodes[node_idx];
        if node.is_depth_limited {
            return;
        }
        if node.pn == old_pn && node.dn == old_dn && node.win_len == old_win_len {
            return;
        }
        let tt_key = (node.hash, node.player);
        self.transposition_table.insert(
            tt_key,
            TTEntry {
                pn: node.pn,
                dn: node.dn,
                win_len: node.win_len,
            },
        );
        self.tt_stores = self.tt_stores.saturating_add(1);
    }
}

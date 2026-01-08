use std::collections::hash_map::Entry;
use std::time::Instant;

use crate::pns::{PNSSolver, TTEntry};
impl PNSSolver {
    pub fn dfpn_search(&mut self, node_idx: usize, pn_threshold: u64, dn_threshold: u64) {
        self.iterations += 1;

        let node = &self.nodes[node_idx];
        if node.pn == 0 || node.dn == 0 {
            return;
        }
        let tt_key = (node.hash, node.player);
        self.tt_lookups = self.tt_lookups.saturating_add(1);
        if let Some(entry) = self.transposition_table.get(&tt_key) {
            self.tt_hits = self.tt_hits.saturating_add(1);
            self.nodes[node_idx].pn = entry.pn;
            self.nodes[node_idx].dn = entry.dn;
            self.nodes[node_idx].win_len = entry.win_len;
            return;
        }
        if !self.nodes[node_idx].is_expanded {
            self.expand_node(node_idx);
            self.update_node_pdn(node_idx);
            if self.nodes[node_idx].pn == 0 || self.nodes[node_idx].dn == 0 {
                return;
            }
        }
        loop {
            if self.nodes[node_idx].children.is_empty() {
                break;
            }
            let children = self.nodes[node_idx].children.clone();
            let is_or_node = self.nodes[node_idx].is_or_node();
            let player = self.nodes[node_idx].player;
            let best_child_idx = if is_or_node {
                *children
                    .iter()
                    .min_by_key(|&&idx| (self.nodes[idx].pn, self.nodes[idx].win_len))
                    .unwrap()
            } else {
                *children
                    .iter()
                    .min_by_key(|&&idx| (self.nodes[idx].dn, self.nodes[idx].win_len))
                    .unwrap()
            };
            let best_move = self.nodes[best_child_idx].mov.unwrap();
            self.game_state.make_move(best_move, player);
            let (new_pn_thresh, new_dn_thresh) = if is_or_node {
                (
                    pn_threshold.min(self.nodes[best_child_idx].pn.saturating_add(1)),
                    dn_threshold,
                )
            } else {
                (
                    pn_threshold,
                    dn_threshold.min(self.nodes[best_child_idx].dn.saturating_add(1)),
                )
            };
            self.dfpn_search(best_child_idx, new_pn_thresh, new_dn_thresh);
            self.game_state.undo_move(best_move);
            let old_pn = self.nodes[node_idx].pn;
            let old_dn = self.nodes[node_idx].dn;
            self.update_node_pdn(node_idx);
            if self.nodes[node_idx].pn == old_pn && self.nodes[node_idx].dn == old_dn {
                break;
            }
            if self.nodes[node_idx].pn >= pn_threshold || self.nodes[node_idx].dn >= dn_threshold {
                break;
            }
        }
        let node = &self.nodes[node_idx];
        if (node.pn == 0 || node.dn == 0) && !node.is_depth_limited {
            let tt_key = (node.hash, node.player);
            match self.transposition_table.entry(tt_key) {
                Entry::Vacant(entry) => {
                    entry.insert(TTEntry {
                        pn: node.pn,
                        dn: node.dn,
                        win_len: node.win_len,
                    });
                    self.tt_stores = self.tt_stores.saturating_add(1);
                }
                Entry::Occupied(_) => {}
            }
        }
    }

    pub fn solve_within_depth_limit(&mut self, verbose: bool) -> bool {
        let start_time = Instant::now();
        self.iterations = 0;
        self.nodes_processed = 0;
        self.tt_lookups = 0;
        self.tt_hits = 0;
        self.tt_stores = 0;
        self.eval_calls = 0;
        self.eval_time_ns = 0;
        self.expand_time_ns = 0;
        self.movegen_time_ns = 0;
        self.children_generated = 0;
        self.depth_cutoffs = 0;
        self.early_cutoffs = 0;
        while self.nodes[self.root].pn != 0 && self.nodes[self.root].dn != 0 {
            self.dfpn_search(self.root, u64::MAX, u64::MAX);
            if verbose && self.iterations.is_multiple_of(100000) {
                let elapsed = start_time.elapsed().as_secs_f64();
                let ips = if elapsed > 0.0 {
                    self.iterations as f64 / elapsed
                } else {
                    0.0
                };
                let tt_size = self.transposition_table.len();
                let expansions = self.nodes_processed;
                let eps = if elapsed > 0.0 {
                    expansions as f64 / elapsed
                } else {
                    0.0
                };
                let tt_hit_rate = if self.tt_lookups > 0 {
                    self.tt_hits as f64 / self.tt_lookups as f64 * 100.0
                } else {
                    0.0
                };
                let avg_branch = if expansions > 0 {
                    self.children_generated as f64 / expansions as f64
                } else {
                    0.0
                };
                let avg_expand_ms = if expansions > 0 {
                    self.expand_time_ns as f64 / expansions as f64 / 1_000_000.0
                } else {
                    0.0
                };
                let avg_movegen_ms = if expansions > 0 {
                    self.movegen_time_ns as f64 / expansions as f64 / 1_000_000.0
                } else {
                    0.0
                };
                let avg_eval_us = if self.eval_calls > 0 {
                    self.eval_time_ns as f64 / self.eval_calls as f64 / 1_000.0
                } else {
                    0.0
                };
                println!(
                    "迭代: {}, 扩展: {}, 根节点 PN/DN: {}/{}, TT大小: {}, TT命中率: {:.1}%, TT写入: {}, 速度: {:.0} iter/s, 扩展: {:.0}/s, 平均分支: {:.2}, 扩展均耗时: {:.3} ms, 走子生成均耗时: {:.3} ms, 评估均耗时: {:.3} us, 深度截断: {}, 提前剪枝: {}",
                    self.iterations,
                    expansions,
                    self.nodes[self.root].pn,
                    self.nodes[self.root].dn,
                    tt_size,
                    tt_hit_rate,
                    self.tt_stores,
                    ips,
                    eps,
                    avg_branch,
                    avg_expand_ms,
                    avg_movegen_ms,
                    avg_eval_us,
                    self.depth_cutoffs,
                    self.early_cutoffs
                );
            }
        }
        let end_time = Instant::now();
        if verbose {
            let elapsed = (end_time - start_time).as_secs_f64();
            let tt_hit_rate = if self.tt_lookups > 0 {
                self.tt_hits as f64 / self.tt_lookups as f64 * 100.0
            } else {
                0.0
            };
            let avg_branch = if self.nodes_processed > 0 {
                self.children_generated as f64 / self.nodes_processed as f64
            } else {
                0.0
            };
            let avg_expand_ms = if self.nodes_processed > 0 {
                self.expand_time_ns as f64 / self.nodes_processed as f64 / 1_000_000.0
            } else {
                0.0
            };
            let avg_movegen_ms = if self.nodes_processed > 0 {
                self.movegen_time_ns as f64 / self.nodes_processed as f64 / 1_000_000.0
            } else {
                0.0
            };
            let avg_eval_us = if self.eval_calls > 0 {
                self.eval_time_ns as f64 / self.eval_calls as f64 / 1_000.0
            } else {
                0.0
            };
            println!(
                "用时 {:.2} 秒，总迭代次数: {}, 总扩展节点数: {}, TT命中率: {:.1}%, TT写入: {}, 平均分支: {:.2}",
                elapsed,
                self.iterations,
                self.nodes_processed,
                tt_hit_rate,
                self.tt_stores,
                avg_branch
            );
            println!(
                "扩展均耗时: {:.3} ms，走子生成均耗时: {:.3} ms，评估均耗时: {:.3} us，深度截断: {}，提前剪枝: {}",
                avg_expand_ms,
                avg_movegen_ms,
                avg_eval_us,
                self.depth_cutoffs,
                self.early_cutoffs
            );
        }
        if self.nodes[self.root].pn == 0 {
            let root_win_len = self.nodes[self.root].win_len;
            let children = self.nodes[self.root].children.clone();
            let winning_children: Vec<usize> = children
                .iter()
                .filter(|&&idx| {
                    self.nodes[idx].pn == 0
                        && (1u64.saturating_add(self.nodes[idx].win_len)) == root_win_len
                })
                .copied()
                .collect();
            if !winning_children.is_empty() {
                let best_child_idx = *winning_children
                    .iter()
                    .min_by_key(|&&idx| (self.nodes[idx].win_len, self.nodes[idx].mov))
                    .unwrap();
                self.best_move = self.nodes[best_child_idx].mov;
            } else if !children.is_empty() {
                let best_child_idx = *children
                    .iter()
                    .min_by_key(|&&idx| (self.nodes[idx].pn, self.nodes[idx].win_len))
                    .unwrap();
                self.best_move = self.nodes[best_child_idx].mov;
            }
            return true;
        }
        false
    }

    pub fn get_best_move(&self) -> Option<(usize, usize)> {
        self.best_move
    }

    pub fn find_best_move_iterative_deepening(&mut self, verbose: bool) -> Option<(usize, usize)> {
        let mut d = 1;
        loop {
            if verbose {
                println!("尝试搜索深度 D={}", d);
            }
            self.increase_depth_limit(d);
            let found = self.solve_within_depth_limit(verbose);
            if found {
                if verbose {
                    println!(
                        "在 {} 步内找到路径，最佳首步: {:?}",
                        self.nodes[self.root].win_len,
                        self.get_best_move()
                    );
                }
                return self.get_best_move();
            }
            d += 1;
        }
    }
}

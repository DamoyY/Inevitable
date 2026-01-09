use std::{sync::Arc, thread, time::Instant};

use super::{ParallelSolver, logging::spawn_logger};
use crate::pns::parallel::{context::ThreadLocalContext, worker::Worker};

fn to_f64(value: u64) -> f64 {
    let value_u32 = u32::try_from(value).unwrap_or(u32::MAX);
    f64::from(value_u32)
}

fn percentage(part: u64, total: u64) -> f64 {
    if total > 0 {
        to_f64(part) / to_f64(total) * 100.0
    } else {
        0.0
    }
}

fn avg_us(total_ns: u64, count: u64) -> f64 {
    if count > 0 {
        to_f64(total_ns) / to_f64(count) / 1_000.0
    } else {
        0.0
    }
}

impl ParallelSolver {
    #[must_use]
    pub fn solve(&self, verbose: bool) -> bool {
        let start_time = Instant::now();
        let tree = Arc::clone(&self.tree);
        if tree.root.is_terminal() {
            if verbose {
                println!(
                    "根节点已是终端状态: PN={}, DN={}",
                    tree.root.get_pn(),
                    tree.root.get_dn()
                );
            }
            if tree.root.get_pn() == 0 && !tree.root.is_expanded() {
                let mut ctx = ThreadLocalContext::new(self.clone_game_state(), 0);
                tree.expand_node(&tree.root, &mut ctx);
                tree.update_node_pdn(&tree.root);
            }
            return tree.root.get_pn() == 0;
        }
        let handles: Vec<_> = (0..self.num_threads)
            .map(|thread_id| {
                let tree = Arc::clone(&tree);
                let game_state = self.clone_game_state();

                thread::spawn(move || {
                    let ctx = ThreadLocalContext::new(game_state, thread_id);
                    let mut worker = Worker::new(tree, ctx);
                    worker.run();
                })
            })
            .collect();

        let logger = if verbose {
            Some(spawn_logger(Arc::clone(&self.tree), self.log_interval_ms))
        } else {
            None
        };

        for handle in handles {
            let _ = handle.join();
        }

        if let Some((log_tx, log_handle)) = logger {
            let _ = log_tx.send(());
            let _ = log_handle.join();
        }

        let elapsed = start_time.elapsed().as_secs_f64();
        if verbose {
            self.print_summary(elapsed);
        }

        self.tree.root.get_pn() == 0
    }

    fn avg_expand_other_us(&self, expansions: u64) -> f64 {
        if expansions == 0 {
            return 0.0;
        }
        let other_ns = self.tree.get_expand_time_ns().saturating_sub(
            self.tree
                .get_movegen_time_ns()
                .saturating_add(self.tree.get_move_apply_time_ns())
                .saturating_add(self.tree.get_hash_time_ns())
                .saturating_add(self.tree.get_node_table_time_ns())
                .saturating_add(self.tree.get_eval_time_ns()),
        );
        to_f64(other_ns) / to_f64(expansions) / 1_000.0
    }

    fn print_summary(&self, elapsed: f64) {
        let iterations = self.tree.get_iterations();
        let expansions = self.tree.get_expansions();
        let tt_lookups = self.tree.get_tt_lookups();
        let tt_hits = self.tree.get_tt_hits();
        let tt_stores = self.tree.get_tt_stores();
        let node_table_lookups = self.tree.get_node_table_lookups();
        let node_table_hits = self.tree.get_node_table_hits();
        let nodes_created = self.tree.get_nodes_created();
        let node_table_size = self.tree.get_node_table_size();
        let children_generated = self.tree.get_children_generated();
        let eval_calls = self.tree.get_eval_calls();

        let avg_branch = if expansions > 0 {
            to_f64(children_generated) / to_f64(expansions)
        } else {
            0.0
        };
        let avg_movegen_us = avg_us(self.tree.get_movegen_time_ns(), expansions);
        let avg_move_apply_us = avg_us(self.tree.get_move_apply_time_ns(), expansions);
        let avg_hash_us = avg_us(self.tree.get_hash_time_ns(), expansions);
        let avg_node_table_us = avg_us(self.tree.get_node_table_time_ns(), expansions);
        let avg_eval_us_per_expand = avg_us(self.tree.get_eval_time_ns(), expansions);
        let avg_expand_other_us = self.avg_expand_other_us(expansions);
        let avg_eval_us = avg_us(self.tree.get_eval_time_ns(), eval_calls);
        let tt_hit_rate = percentage(tt_hits, tt_lookups);
        let node_table_hit_rate = percentage(node_table_hits, node_table_lookups);

        println!(
            "用时 {:.2} 秒，总迭代次数: {}, 总扩展节点数: {}, TT命中率: {:.1}%, TT写入: {}, \
             复用表大小: {}, 复用命中率: {:.1}%, 复用节点: {}, 新建节点: {}, 平均分支: {:.2}, \
             走子生成: {:.3} us，落子/撤销: {:.3} us，哈希: {:.3} us，复用表: {:.3} us，评估: \
             {:.3} us，其他: {:.3} us，评估均耗时: {:.3} us，深度截断: {}，提前剪枝: {}",
            elapsed,
            iterations,
            expansions,
            tt_hit_rate,
            tt_stores,
            node_table_size,
            node_table_hit_rate,
            node_table_hits,
            nodes_created,
            avg_branch,
            avg_movegen_us,
            avg_move_apply_us,
            avg_hash_us,
            avg_node_table_us,
            avg_eval_us_per_expand,
            avg_expand_other_us,
            avg_eval_us,
            self.tree.get_depth_cutoffs(),
            self.tree.get_early_cutoffs()
        );
    }
}

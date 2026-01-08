use std::sync::Arc;
use std::thread;
use std::time::Instant;

use crate::pns::parallel::context::ThreadLocalContext;

use super::logging::spawn_logger;
use crate::pns::parallel::worker::Worker;
use super::ParallelSolver;

impl ParallelSolver {
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
        let iterations = self.tree.get_iterations();
        let expansions = self.tree.get_expansions();
        let tt_lookups = self.tree.get_tt_lookups();
        let tt_hits = self.tree.get_tt_hits();
        let tt_stores = self.tree.get_tt_stores();
        let node_table_lookups = self.tree.get_node_table_lookups();
        let node_table_hits = self.tree.get_node_table_hits();
        let nodes_created = self.tree.get_nodes_created();
        let node_table_size = self.tree.node_table.len();
        let children_generated = self.tree.get_children_generated();
        let avg_branch = if expansions > 0 {
            children_generated as f64 / expansions as f64
        } else {
            0.0
        };
        let avg_expand_ms = if expansions > 0 {
            self.tree.get_expand_time_ns() as f64 / expansions as f64 / 1_000_000.0
        } else {
            0.0
        };
        let avg_movegen_ms = if expansions > 0 {
            self.tree.get_movegen_time_ns() as f64 / expansions as f64 / 1_000_000.0
        } else {
            0.0
        };
        let avg_eval_us = if self.tree.get_eval_calls() > 0 {
            self.tree.get_eval_time_ns() as f64 / self.tree.get_eval_calls() as f64 / 1_000.0
        } else {
            0.0
        };
        let tt_hit_rate = if tt_lookups > 0 {
            tt_hits as f64 / tt_lookups as f64 * 100.0
        } else {
            0.0
        };
        let node_table_hit_rate = if node_table_lookups > 0 {
            node_table_hits as f64 / node_table_lookups as f64 * 100.0
        } else {
            0.0
        };

        if verbose {
            println!(
                "用时 {:.2} 秒，总迭代次数: {}, 总扩展节点数: {}, TT命中率: {:.1}%, TT写入: {}, 复用表大小: {}, 复用命中率: {:.1}%, 复用节点: {}, 新建节点: {},  平均分支: {:.2}",
                elapsed,
                iterations,
                expansions,
                tt_hit_rate,
                tt_stores,
                node_table_size,
                node_table_hit_rate,
                node_table_hits,
                nodes_created,
                avg_branch
            );
            println!(
                "扩展均耗时: {:.3} ms，走子生成均耗时: {:.3} ms，评估均耗时: {:.3} us，深度截断: {}，提前剪枝: {}",
                avg_expand_ms,
                avg_movegen_ms,
                avg_eval_us,
                self.tree.get_depth_cutoffs(),
                self.tree.get_early_cutoffs()
            );
        }

        self.tree.root.get_pn() == 0
    }
}

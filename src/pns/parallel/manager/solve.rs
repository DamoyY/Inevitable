use std::{
    sync::{Arc, mpsc},
    thread,
    time::Instant,
};

use super::{
    ParallelSolver,
    logging::spawn_logger,
    metrics::{SummaryBuildInput, TimingInput, build_summary_line},
};
use crate::pns::parallel::{context::ThreadLocalContext, shared_tree::SharedTree, worker::Worker};

type LoggerHandle = (mpsc::Sender<()>, thread::JoinHandle<()>);

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
        let handles = self.spawn_workers(&tree);
        let logger = self.maybe_spawn_logger(verbose);

        Self::wait_for_workers(handles);
        Self::stop_logger(logger);
        let elapsed = start_time.elapsed().as_secs_f64();
        if verbose {
            self.print_summary(elapsed);
        }
        self.tree.root.get_pn() == 0
    }

    fn spawn_workers(&self, tree: &Arc<SharedTree>) -> Vec<thread::JoinHandle<()>> {
        (0..self.num_threads)
            .map(|thread_id| {
                let tree = Arc::clone(tree);
                let game_state = self.clone_game_state();
                thread::spawn(move || {
                    let ctx = ThreadLocalContext::new(game_state, thread_id);
                    let mut worker = Worker::new(tree, ctx);
                    worker.run();
                })
            })
            .collect()
    }

    fn wait_for_workers(handles: Vec<thread::JoinHandle<()>>) {
        for handle in handles {
            let _ = handle.join();
        }
    }

    fn maybe_spawn_logger(&self, verbose: bool) -> Option<LoggerHandle> {
        verbose.then(|| spawn_logger(Arc::clone(&self.tree), self.log_interval_ms))
    }

    fn stop_logger(logger: Option<LoggerHandle>) {
        if let Some((log_tx, log_handle)) = logger {
            let _ = log_tx.send(());
            let _ = log_handle.join();
        }
    }

    fn print_summary(&self, elapsed: f64) {
        let iterations = self.tree.get_iterations();
        let expansions = self.tree.get_expansions();
        let line = build_summary_line(&SummaryBuildInput {
            elapsed_secs: Some(elapsed),
            iterations,
            expansions,
            root_pn_dn: None,
            tt_size: self.tree.get_tt_size(),
            tt_stores: self.tree.get_tt_stores(),
            node_table_size: self.tree.get_node_table_size(),
            node_table_hits: self.tree.get_node_table_hits(),
            nodes_created: self.tree.get_nodes_created(),
            tt_hits: self.tree.get_tt_hits(),
            tt_lookups: self.tree.get_tt_lookups(),
            node_table_lookups: self.tree.get_node_table_lookups(),
            timing: TimingInput {
                expansions,
                children_generated: self.tree.get_children_generated(),
                expand_ns: self.tree.get_expand_time_ns(),
                movegen_ns: self.tree.get_movegen_time_ns(),
                move_apply_ns: self.tree.get_move_apply_time_ns(),
                hash_ns: self.tree.get_hash_time_ns(),
                node_table_ns: self.tree.get_node_table_time_ns(),
                eval_ns: self.tree.get_eval_time_ns(),
                eval_calls: self.tree.get_eval_calls(),
            },
            depth_cutoffs: self.tree.get_depth_cutoffs(),
            early_cutoffs: self.tree.get_early_cutoffs(),
            speed: None,
        });
        println!("{line}");
    }
}

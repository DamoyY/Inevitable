use std::{sync::Arc, thread, time::Instant};

use super::{ParallelSolver, logging::write_csv_log, metrics::format_sci_u64};
use crate::{
    alloc_stats::AllocTrackingGuard,
    pns::parallel::{context::ThreadLocalContext, shared_tree::SharedTree, worker::Worker},
};
impl ParallelSolver {
    #[must_use]
    pub fn solve(&self, verbose: bool) -> bool {
        let start_time = Instant::now();
        let _alloc_guard = AllocTrackingGuard::new();
        let tree = Arc::clone(&self.tree);
        if tree.stop_requested() {
            return false;
        }
        if tree.root.is_terminal() {
            if verbose {
                println!(
                    "根节点已是终端状态: PN={}, DN={}",
                    format_sci_u64(tree.root.get_pn()),
                    format_sci_u64(tree.root.get_dn())
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
        Self::wait_for_workers(handles);
        let elapsed = start_time.elapsed().as_secs_f64();
        if verbose {
            write_csv_log(&self.tree, self.current_turn(), elapsed);
        }
        self.tree.root.get_pn() == 0
    }

    fn spawn_workers(&self, tree: &Arc<SharedTree>) -> Vec<thread::JoinHandle<()>> {
        (0..self.num_threads)
            .map(|thread_id| {
                let tree = Arc::clone(tree);
                let game_state = self.clone_game_state();
                thread::spawn(move || {
                    let _alloc_guard = AllocTrackingGuard::new();
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

    fn current_turn(&self) -> usize {
        self.base_game_state
            .board
            .iter()
            .fold(0usize, |count, &cell| count + usize::from(cell == 2))
    }
}

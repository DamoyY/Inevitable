use super::context::ThreadLocalContext;
use super::shared_tree::SharedTree;
use super::worker::Worker;
use crate::game_state::{GomokuGameState, ZobristHasher};
use std::sync::Arc;
use std::thread;
use std::time::Instant;

pub struct ParallelSolver {
    pub tree: Arc<SharedTree>,
    pub base_game_state: GomokuGameState,
    pub num_threads: usize,
    board_size: usize,
    win_len: usize,
}

impl ParallelSolver {
    pub fn new(
        initial_board: Vec<Vec<u8>>,
        board_size: usize,
        win_len: usize,
        depth_limit: Option<usize>,
        num_threads: Option<usize>,
    ) -> Self {
        let hasher = Arc::new(ZobristHasher::new(board_size));
        let game_state = GomokuGameState::new(initial_board, hasher, 1, win_len);
        let root_hash = game_state.get_canonical_hash();

        let tree = Arc::new(SharedTree::new(1, root_hash, depth_limit));

        tree.evaluate_node(&tree.root, &ThreadLocalContext::new(game_state.clone(), 0));

        let num_threads = num_threads.unwrap_or_else(|| {
            thread::available_parallelism()
                .map(|n| n.get())
                .unwrap_or(4)
        });

        Self {
            tree,
            base_game_state: game_state,
            num_threads,
            board_size,
            win_len,
        }
    }

    fn clone_game_state(&self) -> GomokuGameState {
        self.base_game_state.clone()
    }

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

        if verbose {
            let tree = Arc::clone(&self.tree);
            let log_handle = thread::spawn(move || {
                let mut last_iterations = 0u64;
                let mut last_time = Instant::now();

                while !tree.is_solved() {
                    thread::sleep(std::time::Duration::from_secs(1));

                    let iterations = tree.get_iterations();
                    let expansions = tree.get_expansions();
                    let root_pn = tree.root.get_pn();
                    let root_dn = tree.root.get_dn();
                    let tt_size = tree.transposition_table.len();

                    let now = Instant::now();
                    let elapsed_since_last = now.duration_since(last_time).as_secs_f64();
                    let ips = if elapsed_since_last > 0.0 {
                        (iterations - last_iterations) as f64 / elapsed_since_last
                    } else {
                        0.0
                    };

                    println!(
                        "迭代: {}, 扩展: {}, 根节点 PN/DN: {}/{}, TT大小: {}, 速度: {:.0} iter/s",
                        iterations, expansions, root_pn, root_dn, tt_size, ips
                    );

                    last_iterations = iterations;
                    last_time = now;
                }
            });

            for handle in handles {
                let _ = handle.join();
            }
            let _ = log_handle.join();
        } else {
            for handle in handles {
                let _ = handle.join();
            }
        }

        let elapsed = start_time.elapsed().as_secs_f64();
        let iterations = self.tree.get_iterations();
        let expansions = self.tree.get_expansions();

        if verbose {
            println!(
                "用时 {:.2} 秒，总迭代次数: {}, 总扩展节点数: {}",
                elapsed, iterations, expansions
            );
        }

        self.tree.root.get_pn() == 0
    }

    pub fn find_best_move_iterative_deepening(
        initial_board: Vec<Vec<u8>>,
        board_size: usize,
        win_len: usize,
        num_threads: Option<usize>,
        verbose: bool,
    ) -> Option<(usize, usize)> {
        let mut depth = 1usize;

        loop {
            if verbose {
                println!("尝试搜索深度 D={}", depth);
            }

            let solver = ParallelSolver::new(
                initial_board.clone(),
                board_size,
                win_len,
                Some(depth),
                num_threads,
            );

            let found = solver.solve(verbose);

            if found {
                let best_move = solver.get_best_move();
                if verbose {
                    println!(
                        "在 {} 步内找到路径，最佳首步: {:?}",
                        solver.root_win_len(),
                        best_move
                    );
                }
                return best_move;
            }

            depth += 1;
        }
    }

    pub fn get_best_move(&self) -> Option<(usize, usize)> {
        let root = &self.tree.root;

        if root.get_pn() != 0 {
            return None;
        }

        let children_guard = root.children.read();
        let children = children_guard.as_ref()?;

        if children.is_empty() {
            return None;
        }

        let root_win_len = root.get_win_len();

        let winning_children: Vec<_> = children
            .iter()
            .filter(|c| c.get_pn() == 0 && 1u64.saturating_add(c.get_win_len()) == root_win_len)
            .collect();

        if !winning_children.is_empty() {
            winning_children
                .iter()
                .min_by_key(|c| (c.get_win_len(), c.mov))
                .and_then(|c| c.mov)
        } else {
            children
                .iter()
                .filter(|c| c.get_pn() == 0)
                .min_by_key(|c| (c.get_win_len(), c.mov))
                .and_then(|c| c.mov)
        }
    }

    pub fn root_pn(&self) -> u64 {
        self.tree.root.get_pn()
    }

    pub fn root_dn(&self) -> u64 {
        self.tree.root.get_dn()
    }

    pub fn root_player(&self) -> u8 {
        self.tree.root.player
    }

    pub fn root_win_len(&self) -> u64 {
        self.tree.root.get_win_len()
    }

    pub fn game_state(&self) -> &GomokuGameState {
        &self.base_game_state
    }

    pub fn board_size(&self) -> usize {
        self.board_size
    }

    pub fn win_len(&self) -> usize {
        self.win_len
    }
}

impl Clone for GomokuGameState {
    fn clone(&self) -> Self {
        let hasher = Arc::clone(&self.hasher);
        let mut state = GomokuGameState::new(self.board.clone(), hasher, 1, self.win_len);

        state.hashes = self.hashes;

        state
    }
}

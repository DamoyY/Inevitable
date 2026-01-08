use super::context::ThreadLocalContext;
use super::shared_tree::SharedTree;
use super::worker::Worker;
use crate::game_state::{GomokuGameState, ZobristHasher};
use std::sync::{mpsc, Arc};
use std::thread;
use std::time::Instant;

pub struct ParallelSolver {
    pub tree: Arc<SharedTree>,
    pub base_game_state: GomokuGameState,
    pub num_threads: usize,
    pub log_interval_ms: u64,
    board_size: usize,
    win_len: usize,
}

impl ParallelSolver {
    pub fn new(
        initial_board: Vec<Vec<u8>>,
        board_size: usize,
        win_len: usize,
        depth_limit: Option<usize>,
        num_threads: usize,
        log_interval_ms: u64,
    ) -> Self {
        let hasher = Arc::new(ZobristHasher::new(board_size));
        let game_state = GomokuGameState::new(initial_board, hasher, 1, win_len);
        let root_hash = game_state.get_canonical_hash();
        let root_pos_hash = game_state.get_hash();

        let tree = Arc::new(SharedTree::new(1, root_hash, root_pos_hash, depth_limit));

        tree.evaluate_node(&tree.root, &ThreadLocalContext::new(game_state.clone(), 0));

        Self {
            tree,
            base_game_state: game_state,
            num_threads,
            log_interval_ms,
            board_size,
            win_len,
        }
    }

    fn clone_game_state(&self) -> GomokuGameState {
        self.base_game_state.clone()
    }

    pub fn increase_depth_limit(&mut self, new_limit: usize) {
        let tree = Arc::get_mut(&mut self.tree)
            .expect("无法取得 SharedTree 的可变引用");
        tree.increase_depth_limit(new_limit);
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
            let log_interval_ms = self.log_interval_ms;
            let (log_tx, log_rx) = mpsc::channel::<()>();
            let log_handle = thread::spawn(move || {
                let mut last_iterations = 0u64;
                let mut last_expansions = 0u64;
                let mut last_children = 0u64;
                let mut last_expand_ns = 0u64;
                let mut last_movegen_ns = 0u64;
                let mut last_eval_ns = 0u64;
                let mut last_eval_calls = 0u64;
                let mut last_tt_lookups = 0u64;
                let mut last_tt_hits = 0u64;
                let mut last_node_table_lookups = 0u64;
                let mut last_node_table_hits = 0u64;
                let mut last_nodes_created = 0u64;
                let mut last_time = Instant::now();

                while !tree.is_solved() {
                    if log_rx
                        .recv_timeout(std::time::Duration::from_millis(
                            log_interval_ms,
                        ))
                        .is_ok()
                    {
                        break;
                    }
                    if tree.is_solved() {
                        break;
                    }

                    let iterations = tree.get_iterations();
                    let expansions = tree.get_expansions();
                    let children_generated = tree.get_children_generated();
                    let expand_ns = tree.get_expand_time_ns();
                    let movegen_ns = tree.get_movegen_time_ns();
                    let eval_ns = tree.get_eval_time_ns();
                    let eval_calls = tree.get_eval_calls();
                    let tt_lookups = tree.get_tt_lookups();
                    let tt_hits = tree.get_tt_hits();
                    let tt_stores = tree.get_tt_stores();
                    let node_table_lookups = tree.get_node_table_lookups();
                    let node_table_hits = tree.get_node_table_hits();
                    let nodes_created = tree.get_nodes_created();
                    let root_pn = tree.root.get_pn();
                    let root_dn = tree.root.get_dn();
                    let tt_size = tree.transposition_table.len();
                    let node_table_size = tree.node_table.len();
                    let depth_cutoffs = tree.get_depth_cutoffs();
                    let early_cutoffs = tree.get_early_cutoffs();

                    let now = Instant::now();
                    let elapsed_since_last = now.duration_since(last_time).as_secs_f64();
                    let delta_iterations = iterations - last_iterations;
                    let delta_expansions = expansions - last_expansions;
                    let delta_children = children_generated - last_children;
                    let delta_expand_ns = expand_ns - last_expand_ns;
                    let delta_movegen_ns = movegen_ns - last_movegen_ns;
                    let delta_eval_ns = eval_ns - last_eval_ns;
                    let delta_eval_calls = eval_calls - last_eval_calls;
                    let delta_tt_lookups = tt_lookups - last_tt_lookups;
                    let delta_tt_hits = tt_hits - last_tt_hits;
                    let delta_node_table_lookups =
                        node_table_lookups - last_node_table_lookups;
                    let delta_node_table_hits = node_table_hits - last_node_table_hits;
                    let delta_nodes_created = nodes_created - last_nodes_created;
                    let ips = if elapsed_since_last > 0.0 {
                        delta_iterations as f64 / elapsed_since_last
                    } else {
                        0.0
                    };
                    let eps = if elapsed_since_last > 0.0 {
                        delta_expansions as f64 / elapsed_since_last
                    } else {
                        0.0
                    };
                    let tt_hit_rate = if delta_tt_lookups > 0 {
                        delta_tt_hits as f64 / delta_tt_lookups as f64 * 100.0
                    } else {
                        0.0
                    };
                    let node_table_hit_rate = if delta_node_table_lookups > 0 {
                        delta_node_table_hits as f64
                            / delta_node_table_lookups as f64
                            * 100.0
                    } else {
                        0.0
                    };
                    let avg_branch = if delta_expansions > 0 {
                        delta_children as f64 / delta_expansions as f64
                    } else {
                        0.0
                    };
                    let avg_expand_ms = if delta_expansions > 0 {
                        delta_expand_ns as f64 / delta_expansions as f64 / 1_000_000.0
                    } else {
                        0.0
                    };
                    let avg_movegen_ms = if delta_expansions > 0 {
                        delta_movegen_ns as f64 / delta_expansions as f64 / 1_000_000.0
                    } else {
                        0.0
                    };
                    let avg_eval_us = if delta_eval_calls > 0 {
                        delta_eval_ns as f64 / delta_eval_calls as f64 / 1_000.0
                    } else {
                        0.0
                    };

                    println!(
                        "迭代: {}, 扩展: {}, 根节点 PN/DN: {}/{}, TT大小: {}, TT命中率: {:.1}%, TT写入: {}, 复用表大小: {}, 复用命中率: {:.1}%, 复用节点: {}, 新建节点: {}, 速度: {:.0} iter/s, 扩展: {:.0}/s, 平均分支: {:.2}, 扩展均耗时: {:.3} ms, 走子生成均耗时: {:.3} ms, 评估均耗时: {:.3} us, 深度截断: {}, 提前剪枝: {}",
                        iterations,
                        expansions,
                        root_pn,
                        root_dn,
                        tt_size,
                        tt_hit_rate,
                        tt_stores,
                        node_table_size,
                        node_table_hit_rate,
                        delta_node_table_hits,
                        delta_nodes_created,
                        ips,
                        eps,
                        avg_branch,
                        avg_expand_ms,
                        avg_movegen_ms,
                        avg_eval_us,
                        depth_cutoffs,
                        early_cutoffs
                    );

                    last_iterations = iterations;
                    last_expansions = expansions;
                    last_children = children_generated;
                    last_expand_ns = expand_ns;
                    last_movegen_ns = movegen_ns;
                    last_eval_ns = eval_ns;
                    last_eval_calls = eval_calls;
                    last_tt_lookups = tt_lookups;
                    last_tt_hits = tt_hits;
                    last_node_table_lookups = node_table_lookups;
                    last_node_table_hits = node_table_hits;
                    last_nodes_created = nodes_created;
                    last_time = now;
                }
            });

            for handle in handles {
                let _ = handle.join();
            }
            let _ = log_tx.send(());
            let _ = log_handle.join();
        } else {
            for handle in handles {
                let _ = handle.join();
            }
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
                "用时 {:.2} 秒，总迭代次数: {}, 总扩展节点数: {}, TT命中率: {:.1}%, TT写入: {}, 复用表大小: {}, 复用命中率: {:.1}%, 复用节点: {}, 新建节点: {}, 平均分支: {:.2}",
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

    pub fn find_best_move_iterative_deepening(
        initial_board: Vec<Vec<u8>>,
        board_size: usize,
        win_len: usize,
        num_threads: usize,
        log_interval_ms: u64,
        verbose: bool,
    ) -> Option<(usize, usize)> {
        let mut depth = 1usize;
        let mut solver = ParallelSolver::new(
            initial_board.clone(),
            board_size,
            win_len,
            Some(depth),
            num_threads,
            log_interval_ms,
        );

        loop {
            if verbose {
                println!("尝试搜索深度 D={}", depth);
            }

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
            solver.increase_depth_limit(depth);
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
            .filter(|c| {
                c.node.get_pn() == 0
                    && 1u64.saturating_add(c.node.get_win_len()) == root_win_len
            })
            .collect();

        if !winning_children.is_empty() {
            winning_children
                .iter()
                .min_by_key(|c| (c.node.get_win_len(), c.mov))
                .map(|c| c.mov)
        } else {
            children
                .iter()
                .filter(|c| c.node.get_pn() == 0)
                .min_by_key(|c| (c.node.get_win_len(), c.mov))
                .map(|c| c.mov)
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

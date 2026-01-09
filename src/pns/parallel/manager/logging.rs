use std::{
    sync::{Arc, mpsc},
    thread,
    time::Instant,
};

use super::SharedTree;

pub(super) fn spawn_logger(
    tree: Arc<SharedTree>,
    log_interval_ms: u64,
) -> (mpsc::Sender<()>, thread::JoinHandle<()>) {
    let (log_tx, log_rx) = mpsc::channel::<()>();
    let handle = thread::spawn(move || {
        let mut last_iterations = 0u64;
        let mut last_expansions = 0u64;
        let mut last_children = 0u64;
        let mut last_expand_ns = 0u64;
        let mut last_movegen_ns = 0u64;
        let mut last_move_apply_ns = 0u64;
        let mut last_hash_ns = 0u64;
        let mut last_eval_ns = 0u64;
        let mut last_eval_calls = 0u64;
        let mut last_tt_lookups = 0u64;
        let mut last_tt_hits = 0u64;
        let mut last_node_table_lookups = 0u64;
        let mut last_node_table_hits = 0u64;
        let mut last_node_table_time_ns = 0u64;
        let mut last_nodes_created = 0u64;
        let mut last_time = Instant::now();

        while !tree.is_solved() {
            if log_rx
                .recv_timeout(std::time::Duration::from_millis(log_interval_ms))
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
            let move_apply_ns = tree.get_move_apply_time_ns();
            let hash_ns = tree.get_hash_time_ns();
            let eval_ns = tree.get_eval_time_ns();
            let eval_calls = tree.get_eval_calls();
            let tt_lookups = tree.get_tt_lookups();
            let tt_hits = tree.get_tt_hits();
            let tt_stores = tree.get_tt_stores();
            let node_table_lookups = tree.get_node_table_lookups();
            let node_table_hits = tree.get_node_table_hits();
            let node_table_time_ns = tree.get_node_table_time_ns();
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
            let delta_move_apply_ns = move_apply_ns - last_move_apply_ns;
            let delta_hash_ns = hash_ns - last_hash_ns;
            let delta_eval_ns = eval_ns - last_eval_ns;
            let delta_eval_calls = eval_calls - last_eval_calls;
            let delta_tt_lookups = tt_lookups - last_tt_lookups;
            let delta_tt_hits = tt_hits - last_tt_hits;
            let delta_node_table_lookups = node_table_lookups - last_node_table_lookups;
            let delta_node_table_hits = node_table_hits - last_node_table_hits;
            let delta_node_table_time_ns = node_table_time_ns - last_node_table_time_ns;
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
                delta_node_table_hits as f64 / delta_node_table_lookups as f64 * 100.0
            } else {
                0.0
            };
            let avg_branch = if delta_expansions > 0 {
                delta_children as f64 / delta_expansions as f64
            } else {
                0.0
            };
            let avg_movegen_us = if delta_expansions > 0 {
                delta_movegen_ns as f64 / delta_expansions as f64 / 1_000.0
            } else {
                0.0
            };
            let avg_move_apply_us = if delta_expansions > 0 {
                delta_move_apply_ns as f64 / delta_expansions as f64 / 1_000.0
            } else {
                0.0
            };
            let avg_hash_us = if delta_expansions > 0 {
                delta_hash_ns as f64 / delta_expansions as f64 / 1_000.0
            } else {
                0.0
            };
            let avg_node_table_us = if delta_expansions > 0 {
                delta_node_table_time_ns as f64 / delta_expansions as f64 / 1_000.0
            } else {
                0.0
            };
            let avg_eval_us_per_expand = if delta_expansions > 0 {
                delta_eval_ns as f64 / delta_expansions as f64 / 1_000.0
            } else {
                0.0
            };
            let avg_expand_other_us = if delta_expansions > 0 {
                let other_ns = delta_expand_ns.saturating_sub(
                    delta_movegen_ns
                        .saturating_add(delta_move_apply_ns)
                        .saturating_add(delta_hash_ns)
                        .saturating_add(delta_node_table_time_ns)
                        .saturating_add(delta_eval_ns),
                );
                other_ns as f64 / delta_expansions as f64 / 1_000.0
            } else {
                0.0
            };
            let avg_eval_us = if delta_eval_calls > 0 {
                delta_eval_ns as f64 / delta_eval_calls as f64 / 1_000.0
            } else {
                0.0
            };

            println!(
                "迭代: {iterations}, 扩展: {expansions}, 根节点 PN/DN: {root_pn}/{root_dn}, TT大小: {tt_size}, TT命中率: {tt_hit_rate:.1}%, TT写入: \
                 {tt_stores}, 复用表大小: {node_table_size}, 复用命中率: {node_table_hit_rate:.1}%, 复用节点: {delta_node_table_hits}, 新建节点: {delta_nodes_created}, 速度: {ips:.0} \
                 iter/s, 扩展: {eps:.0}/s, 平均分支: {avg_branch:.2}, 走子生成: {avg_movegen_us:.3} us, 落子/撤销: {avg_move_apply_us:.3} us, \
                 哈希: {avg_hash_us:.3} us, 复用表: {avg_node_table_us:.3} us, 评估: {avg_eval_us_per_expand:.3} us, 其他: {avg_expand_other_us:.3} us, 评估均耗时: \
                 {avg_eval_us:.3} us, 深度截断: {depth_cutoffs}, 提前剪枝: {early_cutoffs}"
            );

            last_iterations = iterations;
            last_expansions = expansions;
            last_children = children_generated;
            last_expand_ns = expand_ns;
            last_movegen_ns = movegen_ns;
            last_move_apply_ns = move_apply_ns;
            last_hash_ns = hash_ns;
            last_eval_ns = eval_ns;
            last_eval_calls = eval_calls;
            last_tt_lookups = tt_lookups;
            last_tt_hits = tt_hits;
            last_node_table_lookups = node_table_lookups;
            last_node_table_hits = node_table_hits;
            last_node_table_time_ns = node_table_time_ns;
            last_nodes_created = nodes_created;
            last_time = now;
        }
    });

    (log_tx, handle)
}

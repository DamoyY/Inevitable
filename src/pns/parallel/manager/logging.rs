use std::{
    sync::{Arc, mpsc},
    thread,
    time::Instant,
};

use super::SharedTree;

struct LogSnapshot {
    iterations: u64,
    expansions: u64,
    children_generated: u64,
    expand_ns: u64,
    movegen_ns: u64,
    move_apply_ns: u64,
    hash_ns: u64,
    eval_ns: u64,
    eval_calls: u64,
    tt_lookups: u64,
    tt_hits: u64,
    tt_stores: u64,
    node_table_lookups: u64,
    node_table_hits: u64,
    node_table_time_ns: u64,
    nodes_created: u64,
    root_pn: u64,
    root_dn: u64,
    tt_size: usize,
    node_table_size: usize,
    depth_cutoffs: u64,
    early_cutoffs: u64,
    timestamp: Instant,
}

struct LogDelta {
    iterations: u64,
    expansions: u64,
    children_generated: u64,
    expand_ns: u64,
    movegen_ns: u64,
    move_apply_ns: u64,
    hash_ns: u64,
    eval_ns: u64,
    eval_calls: u64,
    tt_lookups: u64,
    tt_hits: u64,
    node_table_lookups: u64,
    node_table_hits: u64,
    node_table_time_ns: u64,
    nodes_created: u64,
    elapsed_secs: f64,
}

impl LogSnapshot {
    fn zero() -> Self {
        Self {
            iterations: 0,
            expansions: 0,
            children_generated: 0,
            expand_ns: 0,
            movegen_ns: 0,
            move_apply_ns: 0,
            hash_ns: 0,
            eval_ns: 0,
            eval_calls: 0,
            tt_lookups: 0,
            tt_hits: 0,
            tt_stores: 0,
            node_table_lookups: 0,
            node_table_hits: 0,
            node_table_time_ns: 0,
            nodes_created: 0,
            root_pn: 0,
            root_dn: 0,
            tt_size: 0,
            node_table_size: 0,
            depth_cutoffs: 0,
            early_cutoffs: 0,
            timestamp: Instant::now(),
        }
    }
}

fn capture_snapshot(tree: &SharedTree) -> LogSnapshot {
    LogSnapshot {
        iterations: tree.get_iterations(),
        expansions: tree.get_expansions(),
        children_generated: tree.get_children_generated(),
        expand_ns: tree.get_expand_time_ns(),
        movegen_ns: tree.get_movegen_time_ns(),
        move_apply_ns: tree.get_move_apply_time_ns(),
        hash_ns: tree.get_hash_time_ns(),
        eval_ns: tree.get_eval_time_ns(),
        eval_calls: tree.get_eval_calls(),
        tt_lookups: tree.get_tt_lookups(),
        tt_hits: tree.get_tt_hits(),
        tt_stores: tree.get_tt_stores(),
        node_table_lookups: tree.get_node_table_lookups(),
        node_table_hits: tree.get_node_table_hits(),
        node_table_time_ns: tree.get_node_table_time_ns(),
        nodes_created: tree.get_nodes_created(),
        root_pn: tree.root.get_pn(),
        root_dn: tree.root.get_dn(),
        tt_size: tree.get_tt_size(),
        node_table_size: tree.get_node_table_size(),
        depth_cutoffs: tree.get_depth_cutoffs(),
        early_cutoffs: tree.get_early_cutoffs(),
        timestamp: Instant::now(),
    }
}

fn compute_delta(current: &LogSnapshot, previous: &LogSnapshot) -> LogDelta {
    LogDelta {
        iterations: current.iterations.saturating_sub(previous.iterations),
        expansions: current.expansions.saturating_sub(previous.expansions),
        children_generated: current
            .children_generated
            .saturating_sub(previous.children_generated),
        expand_ns: current.expand_ns.saturating_sub(previous.expand_ns),
        movegen_ns: current.movegen_ns.saturating_sub(previous.movegen_ns),
        move_apply_ns: current.move_apply_ns.saturating_sub(previous.move_apply_ns),
        hash_ns: current.hash_ns.saturating_sub(previous.hash_ns),
        eval_ns: current.eval_ns.saturating_sub(previous.eval_ns),
        eval_calls: current.eval_calls.saturating_sub(previous.eval_calls),
        tt_lookups: current.tt_lookups.saturating_sub(previous.tt_lookups),
        tt_hits: current.tt_hits.saturating_sub(previous.tt_hits),
        node_table_lookups: current
            .node_table_lookups
            .saturating_sub(previous.node_table_lookups),
        node_table_hits: current
            .node_table_hits
            .saturating_sub(previous.node_table_hits),
        node_table_time_ns: current
            .node_table_time_ns
            .saturating_sub(previous.node_table_time_ns),
        nodes_created: current.nodes_created.saturating_sub(previous.nodes_created),
        elapsed_secs: current
            .timestamp
            .duration_since(previous.timestamp)
            .as_secs_f64(),
    }
}

fn to_f64(value: u64) -> f64 {
    let value_u32 = u32::try_from(value).unwrap_or(u32::MAX);
    f64::from(value_u32)
}

fn per_second(delta: u64, elapsed_secs: f64) -> f64 {
    if elapsed_secs > 0.0 {
        to_f64(delta) / elapsed_secs
    } else {
        0.0
    }
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

fn avg_expand_other_us(delta: &LogDelta) -> f64 {
    if delta.expansions == 0 {
        return 0.0;
    }
    let other_ns = delta.expand_ns.saturating_sub(
        delta
            .movegen_ns
            .saturating_add(delta.move_apply_ns)
            .saturating_add(delta.hash_ns)
            .saturating_add(delta.node_table_time_ns)
            .saturating_add(delta.eval_ns),
    );
    to_f64(other_ns) / to_f64(delta.expansions) / 1_000.0
}

fn print_log(current: &LogSnapshot, delta: &LogDelta) {
    let ips = per_second(delta.iterations, delta.elapsed_secs);
    let eps = per_second(delta.expansions, delta.elapsed_secs);
    let tt_hit_rate = percentage(delta.tt_hits, delta.tt_lookups);
    let node_table_hit_rate = percentage(delta.node_table_hits, delta.node_table_lookups);
    let avg_branch = if delta.expansions > 0 {
        to_f64(delta.children_generated) / to_f64(delta.expansions)
    } else {
        0.0
    };
    let avg_movegen_us = avg_us(delta.movegen_ns, delta.expansions);
    let avg_move_apply_us = avg_us(delta.move_apply_ns, delta.expansions);
    let avg_hash_us = avg_us(delta.hash_ns, delta.expansions);
    let avg_node_table_us = avg_us(delta.node_table_time_ns, delta.expansions);
    let avg_eval_us_per_expand = avg_us(delta.eval_ns, delta.expansions);
    let avg_expand_other_us = avg_expand_other_us(delta);
    let avg_eval_us = avg_us(delta.eval_ns, delta.eval_calls);
    println!(
        "迭代: {iterations}, 扩展: {expansions}, 根节点 PN/DN: {root_pn}/{root_dn}, TT大小: \
         {tt_size}, TT命中率: {tt_hit_rate:.1}%, TT写入: {tt_stores}, 复用表大小: \
         {node_table_size}, 复用命中率: {node_table_hit_rate:.1}%, 复用节点: {node_table_hits}, \
         新建节点: {nodes_created}, 速度: {ips:.0} iter/s, 扩展: {eps:.0}/s, 平均分支: \
         {avg_branch:.2}, 走子生成: {avg_movegen_us:.3} us, 落子/撤销: {avg_move_apply_us:.3} us, \
         哈希: {avg_hash_us:.3} us, 复用表: {avg_node_table_us:.3} us, 评估: \
         {avg_eval_us_per_expand:.3} us, 其他: {avg_expand_other_us:.3} us, 评估均耗时: \
         {avg_eval_us:.3} us, 深度截断: {depth_cutoffs}, 提前剪枝: {early_cutoffs}",
        iterations = current.iterations,
        expansions = current.expansions,
        root_pn = current.root_pn,
        root_dn = current.root_dn,
        tt_size = current.tt_size,
        tt_hit_rate = tt_hit_rate,
        tt_stores = current.tt_stores,
        node_table_size = current.node_table_size,
        node_table_hit_rate = node_table_hit_rate,
        node_table_hits = delta.node_table_hits,
        nodes_created = delta.nodes_created,
        ips = ips,
        eps = eps,
        avg_branch = avg_branch,
        avg_movegen_us = avg_movegen_us,
        avg_move_apply_us = avg_move_apply_us,
        avg_hash_us = avg_hash_us,
        avg_node_table_us = avg_node_table_us,
        avg_eval_us_per_expand = avg_eval_us_per_expand,
        avg_expand_other_us = avg_expand_other_us,
        avg_eval_us = avg_eval_us,
        depth_cutoffs = current.depth_cutoffs,
        early_cutoffs = current.early_cutoffs,
    );
}

pub(super) fn spawn_logger(
    tree: Arc<SharedTree>,
    log_interval_ms: u64,
) -> (mpsc::Sender<()>, thread::JoinHandle<()>) {
    let (log_tx, log_rx) = mpsc::channel::<()>();
    let handle = thread::spawn(move || {
        let mut last_snapshot = LogSnapshot::zero();

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

            let current = capture_snapshot(&tree);
            let delta = compute_delta(&current, &last_snapshot);
            print_log(&current, &delta);
            last_snapshot = current;
        }
    });

    (log_tx, handle)
}

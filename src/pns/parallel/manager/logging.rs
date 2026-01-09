use std::{
    sync::{Arc, mpsc},
    thread,
    time::Instant,
};

use super::{
    SharedTree,
    metrics::{SummaryBuildInput, TimingInput, build_summary_line, to_f64},
};

const fn saturating_diff(current: u64, previous: u64) -> u64 {
    current.saturating_sub(previous)
}

struct LogCounters {
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
}

impl LogCounters {
    const fn zero() -> Self {
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
            node_table_lookups: 0,
            node_table_hits: 0,
            node_table_time_ns: 0,
            nodes_created: 0,
        }
    }

    fn from_tree(tree: &SharedTree) -> Self {
        Self {
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
            node_table_lookups: tree.get_node_table_lookups(),
            node_table_hits: tree.get_node_table_hits(),
            node_table_time_ns: tree.get_node_table_time_ns(),
            nodes_created: tree.get_nodes_created(),
        }
    }

    const fn diff(current: &Self, previous: &Self) -> Self {
        Self {
            iterations: saturating_diff(current.iterations, previous.iterations),
            expansions: saturating_diff(current.expansions, previous.expansions),
            children_generated: saturating_diff(
                current.children_generated,
                previous.children_generated,
            ),
            expand_ns: saturating_diff(current.expand_ns, previous.expand_ns),
            movegen_ns: saturating_diff(current.movegen_ns, previous.movegen_ns),
            move_apply_ns: saturating_diff(current.move_apply_ns, previous.move_apply_ns),
            hash_ns: saturating_diff(current.hash_ns, previous.hash_ns),
            eval_ns: saturating_diff(current.eval_ns, previous.eval_ns),
            eval_calls: saturating_diff(current.eval_calls, previous.eval_calls),
            tt_lookups: saturating_diff(current.tt_lookups, previous.tt_lookups),
            tt_hits: saturating_diff(current.tt_hits, previous.tt_hits),
            node_table_lookups: saturating_diff(
                current.node_table_lookups,
                previous.node_table_lookups,
            ),
            node_table_hits: saturating_diff(current.node_table_hits, previous.node_table_hits),
            node_table_time_ns: saturating_diff(
                current.node_table_time_ns,
                previous.node_table_time_ns,
            ),
            nodes_created: saturating_diff(current.nodes_created, previous.nodes_created),
        }
    }

    const fn timing_input(&self) -> TimingInput {
        TimingInput {
            expansions: self.expansions,
            children_generated: self.children_generated,
            expand_ns: self.expand_ns,
            movegen_ns: self.movegen_ns,
            move_apply_ns: self.move_apply_ns,
            hash_ns: self.hash_ns,
            node_table_ns: self.node_table_time_ns,
            eval_ns: self.eval_ns,
            eval_calls: self.eval_calls,
        }
    }
}

struct LogSnapshot {
    counters: LogCounters,
    tt_stores: u64,
    root_pn: u64,
    root_dn: u64,
    tt_size: usize,
    node_table_size: usize,
    depth_cutoffs: u64,
    early_cutoffs: u64,
    timestamp: Instant,
}

struct LogDelta {
    counters: LogCounters,
    elapsed_secs: f64,
}

impl LogSnapshot {
    fn zero() -> Self {
        Self {
            counters: LogCounters::zero(),
            tt_stores: 0,
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
        counters: LogCounters::from_tree(tree),
        tt_stores: tree.get_tt_stores(),
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
        counters: LogCounters::diff(&current.counters, &previous.counters),
        elapsed_secs: current
            .timestamp
            .duration_since(previous.timestamp)
            .as_secs_f64(),
    }
}

fn per_second(delta: u64, elapsed_secs: f64) -> f64 {
    if elapsed_secs > 0.0 {
        to_f64(delta) / elapsed_secs
    } else {
        0.0
    }
}

fn print_log(current: &LogSnapshot, delta: &LogDelta) {
    let ips = per_second(delta.counters.iterations, delta.elapsed_secs);
    let eps = per_second(delta.counters.expansions, delta.elapsed_secs);
    let line = build_summary_line(&SummaryBuildInput {
        elapsed_secs: None,
        iterations: current.counters.iterations,
        expansions: current.counters.expansions,
        root_pn_dn: Some((current.root_pn, current.root_dn)),
        tt_size: current.tt_size,
        tt_stores: current.tt_stores,
        node_table_size: current.node_table_size,
        node_table_hits: delta.counters.node_table_hits,
        nodes_created: delta.counters.nodes_created,
        tt_hits: delta.counters.tt_hits,
        tt_lookups: delta.counters.tt_lookups,
        node_table_lookups: delta.counters.node_table_lookups,
        timing: delta.counters.timing_input(),
        depth_cutoffs: current.depth_cutoffs,
        early_cutoffs: current.early_cutoffs,
        speed: Some((ips, eps)),
    });
    println!("{line}");
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
            let current = capture_snapshot(&tree);
            let delta = compute_delta(&current, &last_snapshot);
            print_log(&current, &delta);
            last_snapshot = current;
        }
    });
    (log_tx, handle)
}

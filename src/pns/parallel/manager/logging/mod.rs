use std::{
    fs::{File, OpenOptions},
    io::{self, BufWriter, Write},
    sync::{
        Mutex,
        atomic::{AtomicBool, Ordering},
    },
};

use super::{
    super::{TimingStats, TreeStatsSnapshot},
    SharedTree,
    metrics::{calc_hit_rates, format_sci_f64, format_sci_u64, format_sci_usize},
};
const LOG_FILE_NAME: &str = "log.csv";
static LOG_FILE_TRUNCATED: AtomicBool = AtomicBool::new(false);
static LAST_LOG_STATE: Mutex<Option<LastLogState>> = Mutex::new(None);
struct LogSnapshot {
    stats: TreeStatsSnapshot,
    tt_size: usize,
    node_table_size: usize,
    depth_limit: Option<usize>,
}
fn capture_snapshot(tree: &SharedTree) -> LogSnapshot {
    LogSnapshot {
        stats: tree.stats_snapshot(),
        tt_size: tree.get_tt_size(),
        node_table_size: tree.get_node_table_size(),
        depth_limit: tree.depth_limit,
    }
}
#[derive(Clone, Copy)]
struct LastLogState {
    stats: TreeStatsSnapshot,
    elapsed_secs: f64,
}
fn delta_since_last(stats: TreeStatsSnapshot, elapsed_secs: f64) -> (TreeStatsSnapshot, f64) {
    let (delta_stats, delta_elapsed) = {
        let mut guard = match LAST_LOG_STATE.lock() {
            Ok(guard) => guard,
            Err(err) => err.into_inner(),
        };
        let prev = *guard;
        *guard = Some(LastLogState {
            stats,
            elapsed_secs,
        });
        drop(guard);
        prev.map_or((stats, elapsed_secs), |prev| {
            (
                stats.delta_since(&prev.stats),
                (elapsed_secs - prev.elapsed_secs).max(0.0),
            )
        })
    };
    (delta_stats, delta_elapsed)
}
fn open_log_writer() -> io::Result<BufWriter<File>> {
    let truncate = !LOG_FILE_TRUNCATED.swap(true, Ordering::AcqRel);
    let mut options = OpenOptions::new();
    options.create(true).write(true);
    if truncate {
        options.truncate(true);
    } else {
        options.append(true);
    }
    let file = options.open(LOG_FILE_NAME)?;
    let mut writer = BufWriter::new(file);
    if truncate {
        let _ = writer.write_all(&[0xEF, 0xBB, 0xBF]);
        write_csv_header(&mut writer)?;
        writer.flush()?;
    }
    Ok(writer)
}
fn write_csv_header(writer: &mut impl Write) -> io::Result<()> {
    let mut headers = Vec::new();
    headers.extend([
        "回合",
        "深度",
        "总耗时",
        "迭代次数",
        "扩展节点数",
        "TranspositionTable大小",
        "TranspositionTable命中率",
        "TranspositionTable写入数",
        "NodeTable大小",
        "NodeTable命中率",
        "NodeTable命中数",
        "NodeTable写入数",
    ]);
    headers.extend(TimingStats::csv_headers());
    headers.push("其他耗时");
    headers.extend(["深度截断数", "提前剪枝数"]);
    writeln!(writer, "{}", headers.join(","))
}
fn write_log(
    writer: &mut impl Write,
    turn: usize,
    elapsed_secs: f64,
    snapshot: &LogSnapshot,
    stats: TreeStatsSnapshot,
) -> io::Result<()> {
    let hit_rates = calc_hit_rates(
        stats.tt_hits,
        stats.tt_lookups,
        stats.node_table_hits,
        stats.node_table_lookups,
    );
    let timing_stats = TimingStats::from_snapshot(&stats);
    let depth = snapshot.depth_limit.unwrap_or(0);
    let mut fields = vec![
        turn.to_string(),
        format_sci_usize(depth),
        format_sci_f64(elapsed_secs),
        format_sci_u64(stats.iterations),
        format_sci_u64(stats.expansions),
        format_sci_usize(snapshot.tt_size),
        format_sci_f64(hit_rates.tt),
        format_sci_u64(stats.tt_stores),
        format_sci_usize(snapshot.node_table_size),
        format_sci_f64(hit_rates.node_table),
        format_sci_u64(stats.node_table_hits),
        format_sci_u64(stats.nodes_created),
    ];
    for &value in timing_stats.csv_values() {
        fields.push(format_sci_f64(value));
    }
    let elapsed_us = elapsed_secs * 1_000_000.0;
    let other_us = (elapsed_us - timing_stats.sum_us()).max(0.0);
    fields.push(format_sci_f64(other_us));
    fields.push(format_sci_u64(stats.depth_cutoffs));
    fields.push(format_sci_u64(stats.early_cutoffs));
    writeln!(writer, "{}", fields.join(","))
}
pub(super) fn write_csv_log(tree: &SharedTree, turn: usize, elapsed_secs: f64) {
    let Ok(mut writer) = open_log_writer() else {
        return;
    };
    let snapshot = capture_snapshot(tree);
    let (delta_stats, delta_elapsed_secs) = delta_since_last(snapshot.stats, elapsed_secs);
    if write_log(
        &mut writer,
        turn,
        delta_elapsed_secs,
        &snapshot,
        delta_stats,
    )
    .is_ok()
    {
        let _ = writer.flush();
    }
}

pub(super) fn write_csv_log_snapshot(
    turn: usize,
    elapsed_secs: f64,
    stats: TreeStatsSnapshot,
    tt_size: usize,
    node_table_size: usize,
    depth_limit: Option<usize>,
) {
    let Ok(mut writer) = open_log_writer() else {
        return;
    };
    let snapshot = LogSnapshot {
        stats,
        tt_size,
        node_table_size,
        depth_limit,
    };
    if write_log(&mut writer, turn, elapsed_secs, &snapshot, stats).is_ok() {
        let _ = writer.flush();
    }
}

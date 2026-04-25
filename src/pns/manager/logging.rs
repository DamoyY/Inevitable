use super::super::{SharedTree, TimingStats, TreeStatsSnapshot, stats_def::to_f64};
use crate::checked;
use core::sync::atomic::{AtomicBool, Ordering};
use std::{
    fs::{File, OpenOptions},
    io::{self, BufWriter, Write},
    sync::Mutex,
};
const LOG_FILE_NAME: &str = "log.csv";
static LOG_FILE_TRUNCATED: AtomicBool = AtomicBool::new(false);
static LAST_LOG_STATE: Mutex<Option<LastLogState>> = Mutex::new(None);
fn trim_sci(value: String) -> String {
    if let Some(pos) = value.find('e') {
        let (mantissa_text, exp) = value.split_at(pos);
        let mut trimmed_mantissa = String::from(mantissa_text);
        while trimmed_mantissa.ends_with('0') {
            trimmed_mantissa.pop();
        }
        if trimmed_mantissa.ends_with('.') {
            trimmed_mantissa.pop();
        }
        return format!("{trimmed_mantissa}{exp}");
    }
    value
}
pub(super) fn format_sci_f64(value: f64) -> String {
    trim_sci(format!("{value:.2e}"))
}
pub(super) fn format_sci_u64(value: u64) -> String {
    if value == 0 {
        return String::from("0");
    }
    let decimal_text = value.to_string();
    let mut exponent = checked::sub_usize(decimal_text.len(), 1_usize, "format_sci_u64::exponent");
    let mut sig = decimal_text.chars().take(3).collect::<String>();
    while sig.len() < 3 {
        sig.push('0');
    }
    let mut sig_val = match sig.parse::<u32>() {
        Ok(parsed_sig) => parsed_sig,
        Err(err) => {
            eprintln!("解析科学计数法有效数字失败: {sig}, 错误: {err}");
            panic!("解析科学计数法有效数字失败");
        }
    };
    if decimal_text.len() > 3
        && decimal_text
            .as_bytes()
            .get(3)
            .is_some_and(|digit| *digit >= b'5')
    {
        sig_val = checked::add_u32(sig_val, 1_u32, "format_sci_u64::round_significand");
    }
    if sig_val >= 1000 {
        sig_val = 100;
        exponent = checked::add_usize(exponent, 1_usize, "format_sci_u64::round_exponent");
    }
    let leading = checked::div_u32(sig_val, 100_u32, "format_sci_u64::leading");
    let remainder = checked::rem_u32(sig_val, 100_u32, "format_sci_u64::remainder");
    trim_sci(format!("{leading}.{remainder:02}e{exponent}"))
}
pub(super) fn format_sci_usize(value: usize) -> String {
    let value_u64 = u64::try_from(value).unwrap_or(u64::MAX);
    format_sci_u64(value_u64)
}
fn percentage(part: u64, total: u64) -> f64 {
    if total > 0 {
        to_f64(part) / to_f64(total) * 100.0
    } else {
        0.0
    }
}
struct HitRates {
    tt: f64,
    node_table: f64,
}
fn calc_hit_rates(
    tt_hits: u64,
    tt_lookups: u64,
    node_table_hits: u64,
    node_table_lookups: u64,
) -> HitRates {
    HitRates {
        tt: percentage(tt_hits, tt_lookups),
        node_table: percentage(node_table_hits, node_table_lookups),
    }
}
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
        depth_limit: tree.depth_limit(),
    }
}
#[derive(Clone, Copy)]
struct LastLogState {
    session_id: u64,
    stats: TreeStatsSnapshot,
    elapsed_secs: f64,
}
fn delta_since_last(
    session_id: u64,
    stats: TreeStatsSnapshot,
    elapsed_secs: f64,
) -> (TreeStatsSnapshot, f64) {
    let (delta_stats, delta_elapsed) = {
        let mut guard = match LAST_LOG_STATE.lock() {
            Ok(guard) => guard,
            Err(err) => err.into_inner(),
        };
        let prev = *guard;
        *guard = Some(LastLogState {
            session_id,
            stats,
            elapsed_secs,
        });
        drop(guard);
        match prev {
            Some(last) if last.session_id == session_id => (
                stats.delta_since(&last.stats),
                (elapsed_secs - last.elapsed_secs).max(0.0_f64),
            ),
            _ => (stats, elapsed_secs),
        }
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
        writer.write_all(&[0xEF, 0xBB, 0xBF])?;
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
    let elapsed_us = elapsed_secs * 1_000_000.0_f64;
    let other_us = (elapsed_us - timing_stats.sum_us()).max(0.0_f64);
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
    let (delta_stats, delta_elapsed_secs) =
        delta_since_last(tree.stats_session_id(), snapshot.stats, elapsed_secs);
    match write_log(
        &mut writer,
        turn,
        delta_elapsed_secs,
        &snapshot,
        delta_stats,
    ) {
        Ok(()) => {
            if let Err(err) = writer.flush() {
                eprintln!("刷新日志文件失败: {err}");
            }
        }
        Err(err) => {
            eprintln!("写入日志失败: {err}");
        }
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
    match write_log(&mut writer, turn, elapsed_secs, &snapshot, stats) {
        Ok(()) => {
            if let Err(err) = writer.flush() {
                eprintln!("刷新日志文件失败: {err}");
            }
        }
        Err(err) => {
            eprintln!("写入日志快照失败: {err}");
        }
    }
}

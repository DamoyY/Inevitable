use std::{
    fs::{File, OpenOptions},
    io::{self, BufWriter, Write},
    sync::atomic::{AtomicBool, Ordering},
};

use snapshot::{LogSnapshot, capture_snapshot};

use super::{
    super::TimingStats,
    SharedTree,
    metrics::{calc_hit_rates, format_sci_f64, format_sci_u64, format_sci_usize},
};

mod snapshot;

const LOG_FILE_NAME: &str = "log.csv";
static LOG_FILE_TRUNCATED: AtomicBool = AtomicBool::new(false);

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
        "用时",
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
    headers.extend(["深度截断数", "提前剪枝数"]);
    writeln!(writer, "{}", headers.join(","))
}

fn write_log(
    writer: &mut impl Write,
    turn: usize,
    elapsed_secs: f64,
    snapshot: &LogSnapshot,
) -> io::Result<()> {
    let stats = snapshot.stats;
    let hit_rates = calc_hit_rates(
        stats.tt_hits,
        stats.tt_lookups,
        stats.node_table_hits,
        stats.node_table_lookups,
    );
    let timing_stats = TimingStats::from_snapshot(&stats, snapshot.alloc_timing);
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
    for value in timing_stats.csv_values() {
        fields.push(format_sci_f64(value));
    }
    fields.push(format_sci_u64(stats.depth_cutoffs));
    fields.push(format_sci_u64(stats.early_cutoffs));
    writeln!(writer, "{}", fields.join(","))
}

pub(super) fn write_csv_log(tree: &SharedTree, turn: usize, elapsed_secs: f64) {
    let Ok(mut writer) = open_log_writer() else {
        return;
    };
    let snapshot = capture_snapshot(tree);
    if write_log(&mut writer, turn, elapsed_secs, &snapshot).is_ok() {
        let _ = writer.flush();
    }
}

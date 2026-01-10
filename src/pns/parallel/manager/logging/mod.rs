use std::{
    fs::{File, OpenOptions},
    io::{self, BufWriter, Write},
    sync::atomic::{AtomicBool, Ordering},
};

use snapshot::{LogSnapshot, capture_snapshot};

use super::{
    SharedTree,
    metrics::{calc_hit_rates, calc_timing_stats, format_sci_f64, format_sci_u64, format_sci_usize},
};

mod counters;
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
        write_csv_header(&mut writer)?;
        writer.flush()?;
    }
    Ok(writer)
}

fn write_csv_header(writer: &mut impl Write) -> io::Result<()> {
    writeln!(
        writer,
        "回合,深度,用时,迭代次数,扩展节点数,TT大小,TT命中率,TT写入数,复用表大小,复用命中率,复用节点数,新建节点数,平均分支数,平均走子耗时,平均落子耗时,平均撤销耗时,平均哈希耗时,平均复用表耗时,平均评估耗时,平均其他耗时,评估均耗时,深度截断数,提前剪枝数"
    )
}

fn write_log(
    writer: &mut impl Write,
    turn: usize,
    elapsed_secs: f64,
    snapshot: &LogSnapshot,
) -> io::Result<()> {
    let timing = snapshot.counters.timing_input();
    let hit_rates = calc_hit_rates(
        snapshot.counters.tt_hits,
        snapshot.counters.tt_lookups,
        snapshot.counters.node_table_hits,
        snapshot.counters.node_table_lookups,
    );
    let timing_stats = calc_timing_stats(&timing);
    let depth = snapshot.depth_limit.unwrap_or(0);
    writeln!(
        writer,
        "{turn},{depth},{elapsed},{iterations},{expansions},{tt_size},{tt_hit},{tt_stores},{node_table_size},{node_hit_rate},{node_hits},{nodes_created},{branch},{movegen},{move_make},{move_undo},{hash},{node_table},{eval_per_expand},{expand_other},{eval_avg},{depth_cutoffs},{early_cutoffs}",
        depth = format_sci_usize(depth),
        elapsed = format_sci_f64(elapsed_secs),
        iterations = format_sci_u64(snapshot.counters.iterations),
        expansions = format_sci_u64(snapshot.counters.expansions),
        tt_size = format_sci_usize(snapshot.tt_size),
        tt_hit = format_sci_f64(hit_rates.tt),
        tt_stores = format_sci_u64(snapshot.tt_stores),
        node_table_size = format_sci_usize(snapshot.node_table_size),
        node_hit_rate = format_sci_f64(hit_rates.node_table),
        node_hits = format_sci_u64(snapshot.counters.node_table_hits),
        nodes_created = format_sci_u64(snapshot.counters.nodes_created),
        branch = format_sci_f64(timing_stats.branch),
        movegen = format_sci_f64(timing_stats.movegen_us),
        move_make = format_sci_f64(timing_stats.move_make_us),
        move_undo = format_sci_f64(timing_stats.move_undo_us),
        hash = format_sci_f64(timing_stats.hash_us),
        node_table = format_sci_f64(timing_stats.node_table_us),
        eval_per_expand = format_sci_f64(timing_stats.eval_us_per_expand),
        expand_other = format_sci_f64(timing_stats.expand_other_us),
        eval_avg = format_sci_f64(timing_stats.eval_us),
        depth_cutoffs = format_sci_u64(snapshot.depth_cutoffs),
        early_cutoffs = format_sci_u64(snapshot.early_cutoffs),
    )
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

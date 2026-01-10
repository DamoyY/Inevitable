use std::{
    fs::{File, OpenOptions},
    io::{self, BufWriter, Write},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread,
};

use snapshot::{LogDelta, LogSnapshot, capture_snapshot, compute_delta};

use super::{
    SharedTree,
    metrics::{
        calc_hit_rates, calc_timing_stats, format_sci_f64, format_sci_u64, format_sci_usize, to_f64,
    },
};

mod counters;
mod snapshot;

const LOG_FILE_NAME: &str = "log.csv";
static LOG_FILE_TRUNCATED: AtomicBool = AtomicBool::new(false);

fn per_second(delta: u64, elapsed_secs: f64) -> f64 {
    if elapsed_secs > 0.0 {
        to_f64(delta) / elapsed_secs
    } else {
        0.0
    }
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
        write_csv_header(&mut writer)?;
        writer.flush()?;
    }
    Ok(writer)
}

fn write_csv_header(writer: &mut impl Write) -> io::Result<()> {
    writeln!(
        writer,
        "回合,深度,迭代,扩展,根PN,根DN,速度_iter/s,扩展/s,TT大小,TT命中率,TT写入,复用表大小,\
         复用命中率,复用节点,新建节点,平均分支,走子生成_us,落子_us,撤销_us,哈希_us,复用表_us,\
         评估_us,其他_us,评估均耗时_us,深度截断,提前剪枝"
    )
}

fn write_log(
    writer: &mut impl Write,
    turn: usize,
    current: &LogSnapshot,
    delta: &LogDelta,
) -> io::Result<()> {
    let ips = per_second(delta.counters.iterations, delta.elapsed_secs);
    let eps = per_second(delta.counters.expansions, delta.elapsed_secs);
    let timing = delta.counters.timing_input();
    let hit_rates = calc_hit_rates(
        delta.counters.tt_hits,
        delta.counters.tt_lookups,
        delta.counters.node_table_hits,
        delta.counters.node_table_lookups,
    );
    let timing_stats = calc_timing_stats(&timing);
    let depth = current.depth_limit.unwrap_or(0);
    writeln!(
        writer,
        "{turn},{depth},{iterations},{expansions},{root_pn},{root_dn},{ips},{eps},{tt_size},\
         {tt_hit},{tt_stores},{node_table_size},{node_hit_rate},{node_hits},{nodes_created},\
         {branch},{movegen},{move_make},{move_undo},{hash},{node_table},{eval_per_expand},\
         {expand_other},{eval_avg},{depth_cutoffs},{early_cutoffs}",
        iterations = format_sci_u64(current.counters.iterations),
        expansions = format_sci_u64(current.counters.expansions),
        root_pn = format_sci_u64(current.root_pn),
        root_dn = format_sci_u64(current.root_dn),
        ips = format_sci_f64(ips),
        eps = format_sci_f64(eps),
        tt_size = format_sci_usize(current.tt_size),
        tt_hit = format_sci_f64(hit_rates.tt),
        tt_stores = format_sci_u64(current.tt_stores),
        node_table_size = format_sci_usize(current.node_table_size),
        node_hit_rate = format_sci_f64(hit_rates.node_table),
        node_hits = format_sci_u64(delta.counters.node_table_hits),
        nodes_created = format_sci_u64(delta.counters.nodes_created),
        branch = format_sci_f64(timing_stats.branch),
        movegen = format_sci_f64(timing_stats.movegen_us),
        move_make = format_sci_f64(timing_stats.move_make_us),
        move_undo = format_sci_f64(timing_stats.move_undo_us),
        hash = format_sci_f64(timing_stats.hash_us),
        node_table = format_sci_f64(timing_stats.node_table_us),
        eval_per_expand = format_sci_f64(timing_stats.eval_us_per_expand),
        expand_other = format_sci_f64(timing_stats.expand_other_us),
        eval_avg = format_sci_f64(timing_stats.eval_us),
        depth_cutoffs = format_sci_u64(current.depth_cutoffs),
        early_cutoffs = format_sci_u64(current.early_cutoffs),
    )
}

pub(super) fn spawn_logger(
    tree: Arc<SharedTree>,
    log_interval_ms: u64,
    turn: usize,
) -> (mpsc::Sender<()>, thread::JoinHandle<()>) {
    let (log_tx, log_rx) = mpsc::channel::<()>();
    let handle = thread::spawn(move || {
        let Ok(mut writer) = open_log_writer() else {
            return;
        };
        let mut last_snapshot = LogSnapshot::zero();
        while !tree.should_stop() {
            if log_rx
                .recv_timeout(std::time::Duration::from_millis(log_interval_ms))
                .is_ok()
            {
                break;
            }
            let current = capture_snapshot(&tree);
            let delta = compute_delta(&current, &last_snapshot);
            if write_log(&mut writer, turn, &current, &delta).is_err() {
                break;
            }
            let _ = writer.flush();
            last_snapshot = current;
        }
    });
    (log_tx, handle)
}

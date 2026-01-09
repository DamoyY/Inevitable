pub(super) fn to_f64(value: u64) -> f64 {
    let value_u32 = u32::try_from(value).unwrap_or(u32::MAX);
    f64::from(value_u32)
}

fn trim_sci(value: String) -> String {
    if let Some(pos) = value.find('e') {
        let (mantissa, exp) = value.split_at(pos);
        let mut mantissa = mantissa.to_string();
        while mantissa.ends_with('0') {
            mantissa.pop();
        }
        if mantissa.ends_with('.') {
            mantissa.pop();
        }
        return format!("{mantissa}{exp}");
    }
    value
}

pub(super) fn format_sci_f64(value: f64) -> String {
    trim_sci(format!("{value:.2e}"))
}

pub(super) fn format_sci_u64(value: u64) -> String {
    if value == 0 {
        return "0".to_string();
    }
    let s = value.to_string();
    let mut exponent = s.len().saturating_sub(1);
    let mut sig = s.chars().take(3).collect::<String>();
    while sig.len() < 3 {
        sig.push('0');
    }
    let mut sig_val = sig.parse::<u32>().unwrap_or(0);
    if s.len() > 3 && s.as_bytes()[3] >= b'5' {
        sig_val = sig_val.saturating_add(1);
    }
    if sig_val >= 1000 {
        sig_val = 100;
        exponent = exponent.saturating_add(1);
    }
    let leading = sig_val / 100;
    let remainder = sig_val % 100;
    trim_sci(format!("{leading}.{remainder:02}e{exponent:02}"))
}

pub(super) fn format_sci_usize(value: usize) -> String {
    let value_u64 = u64::try_from(value).unwrap_or(u64::MAX);
    format_sci_u64(value_u64)
}

pub(super) fn percentage(part: u64, total: u64) -> f64 {
    if total > 0 {
        to_f64(part) / to_f64(total) * 100.0
    } else {
        0.0
    }
}

pub(super) fn avg_us(total_ns: u64, count: u64) -> f64 {
    if count > 0 {
        to_f64(total_ns) / to_f64(count) / 1_000.0
    } else {
        0.0
    }
}

pub(super) fn avg_expand_other_us(input: &TimingInput) -> f64 {
    if input.expansions == 0 {
        return 0.0;
    }
    let other_ns = input
        .expand_ns
        .saturating_sub(input.movegen_ns)
        .saturating_sub(input.move_make_ns)
        .saturating_sub(input.move_undo_ns)
        .saturating_sub(input.hash_ns)
        .saturating_sub(input.node_table_ns)
        .saturating_sub(input.eval_ns);
    to_f64(other_ns) / to_f64(input.expansions) / 1_000.0
}

pub(super) struct TimingInput {
    pub expansions: u64,
    pub children_generated: u64,
    pub expand_ns: u64,
    pub movegen_ns: u64,
    pub move_make_ns: u64,
    pub move_undo_ns: u64,
    pub hash_ns: u64,
    pub node_table_ns: u64,
    pub eval_ns: u64,
    pub eval_calls: u64,
}

pub(super) struct TimingStats {
    pub branch: f64,
    pub movegen_us: f64,
    pub move_make_us: f64,
    pub move_undo_us: f64,
    pub hash_us: f64,
    pub node_table_us: f64,
    pub eval_us_per_expand: f64,
    pub expand_other_us: f64,
    pub eval_us: f64,
}

pub(super) struct HitRates {
    pub tt: f64,
    pub node_table: f64,
}

pub(super) fn calc_hit_rates(
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

pub(super) fn format_timing_summary(
    stats: &TimingStats,
    depth_cutoffs: u64,
    early_cutoffs: u64,
) -> String {
    format!(
        "平均分支: {branch}, 走子生成: {movegen} us，落子: {move_make} us，撤销: {move_undo} \
         us，哈希: {hash} us，复用表: {node_table} us，评估: {eval_per_expand} us，其他: \
         {expand_other} us，评估均耗时: {eval_avg} us，深度截断: {depth_cutoffs}，提前剪枝: \
         {early_cutoffs}",
        branch = format_sci_f64(stats.branch),
        movegen = format_sci_f64(stats.movegen_us),
        move_make = format_sci_f64(stats.move_make_us),
        move_undo = format_sci_f64(stats.move_undo_us),
        hash = format_sci_f64(stats.hash_us),
        node_table = format_sci_f64(stats.node_table_us),
        eval_per_expand = format_sci_f64(stats.eval_us_per_expand),
        expand_other = format_sci_f64(stats.expand_other_us),
        eval_avg = format_sci_f64(stats.eval_us),
        depth_cutoffs = format_sci_u64(depth_cutoffs),
        early_cutoffs = format_sci_u64(early_cutoffs),
    )
}

pub(super) struct SummaryLineInput<'a> {
    pub elapsed_secs: Option<f64>,
    pub iterations: u64,
    pub expansions: u64,
    pub root_pn_dn: Option<(u64, u64)>,
    pub tt_size: usize,
    pub tt_stores: u64,
    pub node_table_size: usize,
    pub node_table_hits: u64,
    pub nodes_created: u64,
    pub hit_rates: HitRates,
    pub timing_summary: &'a str,
    pub speed: Option<(f64, f64)>,
}

pub(super) struct SummaryBuildInput {
    pub elapsed_secs: Option<f64>,
    pub iterations: u64,
    pub expansions: u64,
    pub root_pn_dn: Option<(u64, u64)>,
    pub tt_size: usize,
    pub tt_stores: u64,
    pub node_table_size: usize,
    pub node_table_hits: u64,
    pub nodes_created: u64,
    pub tt_hits: u64,
    pub tt_lookups: u64,
    pub node_table_lookups: u64,
    pub timing: TimingInput,
    pub depth_cutoffs: u64,
    pub early_cutoffs: u64,
    pub speed: Option<(f64, f64)>,
}

pub(super) fn build_summary_line(input: &SummaryBuildInput) -> String {
    let hit_rates = calc_hit_rates(
        input.tt_hits,
        input.tt_lookups,
        input.node_table_hits,
        input.node_table_lookups,
    );
    let stats = calc_timing_stats(&input.timing);
    let timing_summary = format_timing_summary(&stats, input.depth_cutoffs, input.early_cutoffs);
    format_summary_line(&SummaryLineInput {
        elapsed_secs: input.elapsed_secs,
        iterations: input.iterations,
        expansions: input.expansions,
        root_pn_dn: input.root_pn_dn,
        tt_size: input.tt_size,
        tt_stores: input.tt_stores,
        node_table_size: input.node_table_size,
        node_table_hits: input.node_table_hits,
        nodes_created: input.nodes_created,
        hit_rates,
        timing_summary: &timing_summary,
        speed: input.speed,
    })
}

pub(super) fn format_summary_line(input: &SummaryLineInput<'_>) -> String {
    let iterations = format_sci_u64(input.iterations);
    let expansions = format_sci_u64(input.expansions);
    let mut prefix = input.elapsed_secs.map_or_else(
        || {
            input.root_pn_dn.map_or_else(
                || format!("迭代: {iterations}，扩展: {expansions}， ",),
                |(pn, dn)| {
                    let pn = format_sci_u64(pn);
                    let dn = format_sci_u64(dn);
                    format!("迭代: {iterations}，扩展: {expansions}，根节点 PN/DN: {pn}/{dn}, ",)
                },
            )
        },
        |elapsed| {
            let elapsed = format_sci_f64(elapsed);
            format!("用时 {elapsed} 秒，总迭代次数: {iterations}，总扩展节点数: {expansions}， ",)
        },
    );

    if let Some((ips, eps)) = input.speed {
        let ips = format_sci_f64(ips);
        let eps = format_sci_f64(eps);
        prefix = format!("{prefix}速度: {ips} iter/s, 扩展: {eps}/s, ",);
    }

    let tt_size = format_sci_usize(input.tt_size);
    let tt_hit = format_sci_f64(input.hit_rates.tt);
    let tt_stores = format_sci_u64(input.tt_stores);
    let node_table_size = format_sci_usize(input.node_table_size);
    let node_hit_rate = format_sci_f64(input.hit_rates.node_table);
    let node_hits = format_sci_u64(input.node_table_hits);
    let nodes_created = format_sci_u64(input.nodes_created);
    let timing_summary = input.timing_summary;
    format!(
        "{prefix}TT大小: {tt_size}, TT命中率: {tt_hit}%, TT写入: {tt_stores}, 复用表大小: \
         {node_table_size}, 复用命中率: {node_hit_rate}%, 复用节点: {node_hits}, 新建节点: \
         {nodes_created}, {timing_summary}",
    )
}

pub(super) fn calc_timing_stats(input: &TimingInput) -> TimingStats {
    let branch = if input.expansions > 0 {
        to_f64(input.children_generated) / to_f64(input.expansions)
    } else {
        0.0
    };

    TimingStats {
        branch,
        movegen_us: avg_us(input.movegen_ns, input.expansions),
        move_make_us: avg_us(input.move_make_ns, input.expansions),
        move_undo_us: avg_us(input.move_undo_ns, input.expansions),
        hash_us: avg_us(input.hash_ns, input.expansions),
        node_table_us: avg_us(input.node_table_ns, input.expansions),
        eval_us_per_expand: avg_us(input.eval_ns, input.expansions),
        expand_other_us: avg_expand_other_us(input),
        eval_us: avg_us(input.eval_ns, input.eval_calls),
    }
}

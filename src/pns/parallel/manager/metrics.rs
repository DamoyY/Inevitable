pub(super) fn to_f64(value: u64) -> f64 {
    let value_u32 = u32::try_from(value).unwrap_or(u32::MAX);
    f64::from(value_u32)
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

pub(super) fn avg_expand_other_us(
    expansions: u64,
    expand_ns: u64,
    movegen_ns: u64,
    move_apply_ns: u64,
    hash_ns: u64,
    node_table_ns: u64,
    eval_ns: u64,
) -> f64 {
    if expansions == 0 {
        return 0.0;
    }
    let other_ns = expand_ns
        .saturating_sub(movegen_ns)
        .saturating_sub(move_apply_ns)
        .saturating_sub(hash_ns)
        .saturating_sub(node_table_ns)
        .saturating_sub(eval_ns);
    to_f64(other_ns) / to_f64(expansions) / 1_000.0
}

pub(super) struct TimingInput {
    pub expansions: u64,
    pub children_generated: u64,
    pub expand_ns: u64,
    pub movegen_ns: u64,
    pub move_apply_ns: u64,
    pub hash_ns: u64,
    pub node_table_ns: u64,
    pub eval_ns: u64,
    pub eval_calls: u64,
}

pub(super) struct TimingStats {
    pub branch: f64,
    pub movegen_us: f64,
    pub move_apply_us: f64,
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
        "平均分支: {branch:.2}, 走子生成: {movegen:.3} us，落子/撤销: {move_apply:.3} us，哈希: \
         {hash:.3} us，复用表: {node_table:.3} us，评估: {eval_per_expand:.3} us，其他: \
         {expand_other:.3} us，评估均耗时: {eval_avg:.3} us，深度截断: {depth_cutoffs}，提前剪枝: \
         {early_cutoffs}",
        branch = stats.branch,
        movegen = stats.movegen_us,
        move_apply = stats.move_apply_us,
        hash = stats.hash_us,
        node_table = stats.node_table_us,
        eval_per_expand = stats.eval_us_per_expand,
        expand_other = stats.expand_other_us,
        eval_avg = stats.eval_us,
        depth_cutoffs = depth_cutoffs,
        early_cutoffs = early_cutoffs,
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
    let mut prefix = input.elapsed_secs.map_or_else(
        || {
            input.root_pn_dn.map_or_else(
                || {
                    format!(
                        "迭代: {iterations}，扩展: {expansions}， ",
                        iterations = input.iterations,
                        expansions = input.expansions,
                    )
                },
                |(pn, dn)| {
                    format!(
                        "迭代: {iterations}，扩展: {expansions}，根节点 PN/DN: {pn}/{dn}, ",
                        iterations = input.iterations,
                        expansions = input.expansions,
                    )
                },
            )
        },
        |elapsed| {
            format!(
                "用时 {elapsed:.2} 秒，总迭代次数: {iterations}，总扩展节点数: {expansions}， ",
                iterations = input.iterations,
                expansions = input.expansions,
            )
        },
    );

    if let Some((ips, eps)) = input.speed {
        prefix = format!("{prefix}速度: {ips:.0} iter/s, 扩展: {eps:.0}/s, ");
    }

    format!(
        "{prefix}TT大小: {tt_size}, TT命中率: {tt_hit:.1}%, TT写入: {tt_stores}, 复用表大小: \
         {node_table_size}, 复用命中率: {node_hit_rate:.1}%, 复用节点: {node_hits}, 新建节点: \
         {nodes_created}, {timing_summary}",
        prefix = prefix,
        tt_size = input.tt_size,
        tt_hit = input.hit_rates.tt,
        tt_stores = input.tt_stores,
        node_table_size = input.node_table_size,
        node_hit_rate = input.hit_rates.node_table,
        node_hits = input.node_table_hits,
        nodes_created = input.nodes_created,
        timing_summary = input.timing_summary,
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
        move_apply_us: avg_us(input.move_apply_ns, input.expansions),
        hash_us: avg_us(input.hash_ns, input.expansions),
        node_table_us: avg_us(input.node_table_ns, input.expansions),
        eval_us_per_expand: avg_us(input.eval_ns, input.expansions),
        expand_other_us: avg_expand_other_us(
            input.expansions,
            input.expand_ns,
            input.movegen_ns,
            input.move_apply_ns,
            input.hash_ns,
            input.node_table_ns,
            input.eval_ns,
        ),
        eval_us: avg_us(input.eval_ns, input.eval_calls),
    }
}

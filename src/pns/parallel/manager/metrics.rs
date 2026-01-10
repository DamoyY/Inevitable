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
    trim_sci(format!("{leading}.{remainder:02}e{exponent}"))
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
        .saturating_sub(input.board_update_ns)
        .saturating_sub(input.bitboard_update_ns)
        .saturating_sub(input.threat_index_update_ns)
        .saturating_sub(input.candidate_update_ns)
        .saturating_sub(input.hash_update_ns)
        .saturating_sub(input.move_undo_ns)
        .saturating_sub(input.hash_ns)
        .saturating_sub(input.node_table_lookup_ns)
        .saturating_sub(input.node_table_write_ns)
        .saturating_sub(input.eval_ns);
    to_f64(other_ns) / to_f64(input.expansions) / 1_000.0
}

pub(super) struct TimingInput {
    pub expansions: u64,
    pub children_generated: u64,
    pub expand_ns: u64,
    pub movegen_ns: u64,
    pub board_update_ns: u64,
    pub bitboard_update_ns: u64,
    pub threat_index_update_ns: u64,
    pub candidate_update_ns: u64,
    pub hash_update_ns: u64,
    pub move_undo_ns: u64,
    pub hash_ns: u64,
    pub node_table_lookup_ns: u64,
    pub node_table_write_ns: u64,
    pub eval_ns: u64,
    pub eval_calls: u64,
}

pub(super) struct TimingStats {
    pub branch: f64,
    pub movegen_us: f64,
    pub board_update_us: f64,
    pub bitboard_update_us: f64,
    pub threat_index_update_us: f64,
    pub candidate_update_us: f64,
    pub hash_update_us: f64,
    pub move_undo_us: f64,
    pub hash_us: f64,
    pub node_table_lookup_us: f64,
    pub node_table_write_us: f64,
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

pub(super) fn calc_timing_stats(input: &TimingInput) -> TimingStats {
    let branch = if input.expansions > 0 {
        to_f64(input.children_generated) / to_f64(input.expansions)
    } else {
        0.0
    };

    TimingStats {
        branch,
        movegen_us: avg_us(input.movegen_ns, input.expansions),
        board_update_us: avg_us(input.board_update_ns, input.expansions),
        bitboard_update_us: avg_us(input.bitboard_update_ns, input.expansions),
        threat_index_update_us: avg_us(input.threat_index_update_ns, input.expansions),
        candidate_update_us: avg_us(input.candidate_update_ns, input.expansions),
        hash_update_us: avg_us(input.hash_update_ns, input.expansions),
        move_undo_us: avg_us(input.move_undo_ns, input.expansions),
        hash_us: avg_us(input.hash_ns, input.expansions),
        node_table_lookup_us: avg_us(input.node_table_lookup_ns, input.expansions),
        node_table_write_us: avg_us(input.node_table_write_ns, input.expansions),
        eval_us_per_expand: avg_us(input.eval_ns, input.expansions),
        expand_other_us: avg_expand_other_us(input),
        eval_us: avg_us(input.eval_ns, input.eval_calls),
    }
}

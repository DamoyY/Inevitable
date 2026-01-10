use super::super::stats_def::to_f64;

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

use std::sync::atomic::{AtomicU64, Ordering};

use serde::Serialize;

use crate::game_state::MoveApplyTiming;
pub fn to_f64(value: u64) -> f64 {
    let value_u32 = u32::try_from(value).unwrap_or(u32::MAX);
    f64::from(value_u32)
}
fn total_us(total_ns: u64) -> f64 {
    to_f64(total_ns) / 1_000.0
}
macro_rules! add_move_apply_timing {
    ( $( $field:ident => $stat_field:ident ),* $(,)? ) => {
        pub const fn add_move_apply_timing(&mut self, timing: &MoveApplyTiming) {
            $(self.$stat_field = self.$stat_field.wrapping_add(timing.$field);)*
        }
    };
}
macro_rules! define_metrics {
    (
        counts: { $( $cname:ident => $cdesc:expr ),* $(,)? }
        timings: { $( $tname:ident => $tdesc:expr ),* $(,)? }
        timing_log: { $( $lname:ident => ($ldesc:expr, $calc:expr) ),* $(,)? }
    ) => {
        pub struct TreeStatsAtomic {
            $(pub $cname: AtomicU64,)*
            $(pub $tname: AtomicU64,)*
        }
        impl TreeStatsAtomic {
            #[must_use]
            pub const fn new() -> Self {
                Self {
                    $($cname: AtomicU64::new(0),)*
                    $($tname: AtomicU64::new(0),)*
                }
            }
            #[must_use]
            pub fn snapshot(&self) -> TreeStatsSnapshot {
                TreeStatsSnapshot {
                    $($cname: self.$cname.load(Ordering::Relaxed),)*
                    $($tname: self.$tname.load(Ordering::Relaxed),)*
                }
            }
            pub fn merge(&self, acc: &TreeStatsAccumulator) {
                $(self.$cname.fetch_add(acc.$cname, Ordering::Relaxed);)*
                $(self.$tname.fetch_add(acc.$tname, Ordering::Relaxed);)*
            }
        }
        #[derive(Clone, Copy, Default, Serialize)]
        pub struct TreeStatsSnapshot {
            $(pub $cname: u64,)*
            $(pub $tname: u64,)*
        }
        impl TreeStatsSnapshot {
            #[must_use]
            pub const fn delta_since(&self, prev: &Self) -> Self {
                Self {
                    $($cname: self.$cname.saturating_sub(prev.$cname),)*
                    $($tname: self.$tname.saturating_sub(prev.$tname),)*
                }
            }
        }
        #[derive(Default)]
        pub struct TreeStatsAccumulator {
            $(pub $cname: u64,)*
            $(pub $tname: u64,)*
        }
        impl TreeStatsAccumulator {
            crate::for_each_move_apply_timing!(add_move_apply_timing);
        }
        pub struct TimingStats {
            values: Vec<f64>,
        }
        impl TimingStats {
            #[must_use]
            pub fn from_snapshot(snapshot: &TreeStatsSnapshot) -> Self {
                let values = vec![$(($calc)(snapshot),)*];
                Self { values }
            }
            pub const fn csv_headers() -> &'static [&'static str] {
                &[$($ldesc,)*]
            }
            #[must_use]
            pub fn csv_values(&self) -> &[f64] {
                &self.values
            }
            #[must_use]
            pub fn sum_us(&self) -> f64 {
                Self::csv_headers()
                    .iter()
                    .zip(self.values.iter())
                    .filter(|(header, _)| header.contains("耗时"))
                    .map(|(_, value)| *value)
                    .sum()
            }
        }
    };
}
define_metrics! {
    counts: {
        iterations => "迭代次数",
        expansions => "扩展节点数",
        children_generated => "生成子节点数",
        tt_lookups => "TranspositionTable查找次数",
        tt_hits => "TranspositionTable命中次数",
        tt_stores => "TranspositionTable写入次数",
        eval_calls => "评估调用数",
        node_table_lookups => "NodeTable查找次数",
        node_table_hits => "NodeTable命中次数",
        nodes_created => "NodeTable节点数",
        depth_cutoffs => "深度截断数",
        early_cutoffs => "提前剪枝数",
    }
    timings: {
        eval_time_ns => "评估耗时",
        expand_time_ns => "扩展耗时",
        movegen_time_ns => "走子生成耗时",
        board_update_time_ns => "基础棋盘更新耗时",
        bitboard_update_time_ns => "位棋盘更新耗时",
        threat_index_update_time_ns => "威胁索引更新耗时",
        candidate_remove_time_ns => "候选着法移除耗时",
        candidate_neighbor_time_ns => "邻居空位计算耗时",
        candidate_insert_time_ns => "候选着法更新耗时",
        candidate_newly_added_time_ns => "新增候选着法耗时",
        candidate_history_time_ns => "候选着法保存耗时",
        hash_update_time_ns => "Zobrist哈希更新耗时",
        move_undo_time_ns => "撤销耗时",
        hash_time_ns => "哈希耗时",
        children_lock_time_ns => "子节点锁耗时",
        node_table_lookup_time_ns => "NodeTable检索耗时",
        node_table_write_time_ns => "NodeTable写入耗时",
    }
    timing_log: {
        branch => ("平均分支数", |snapshot: &TreeStatsSnapshot| {
            if snapshot.expansions > 0 {
                to_f64(snapshot.children_generated) / to_f64(snapshot.expansions)
            } else {
                0.0
            }
        }),
        movegen_us => ("走子生成耗时", |snapshot: &TreeStatsSnapshot| {
            total_us(snapshot.movegen_time_ns)
        }),
        board_update_us => ("基础棋盘状态更新耗时", |snapshot: &TreeStatsSnapshot| {
            total_us(snapshot.board_update_time_ns)
        }),
        bitboard_update_us => ("位棋盘更新耗时", |snapshot: &TreeStatsSnapshot| {
            total_us(snapshot.bitboard_update_time_ns)
        }),
        threat_index_update_us => ("威胁索引更新耗时", |snapshot: &TreeStatsSnapshot| {
            total_us(snapshot.threat_index_update_time_ns)
        }),
        candidate_remove_us => ("候选着法移除耗时", |snapshot: &TreeStatsSnapshot| {
            total_us(snapshot.candidate_remove_time_ns)
        }),
        candidate_neighbor_us => ("邻居空位计算耗时", |snapshot: &TreeStatsSnapshot| {
            total_us(snapshot.candidate_neighbor_time_ns)
        }),
        candidate_insert_us => ("候选着法更新耗时", |snapshot: &TreeStatsSnapshot| {
            total_us(snapshot.candidate_insert_time_ns)
        }),
        candidate_newly_added_us => ("新增候选着法记录耗时", |snapshot: &TreeStatsSnapshot| {
            total_us(snapshot.candidate_newly_added_time_ns)
        }),
        candidate_history_us => ("候选着法历史保存耗时", |snapshot: &TreeStatsSnapshot| {
            total_us(snapshot.candidate_history_time_ns)
        }),
        hash_update_us => ("Zobrist哈希增量更新耗时", |snapshot: &TreeStatsSnapshot| {
            total_us(snapshot.hash_update_time_ns)
        }),
        move_undo_us => ("撤销耗时", |snapshot: &TreeStatsSnapshot| {
            total_us(snapshot.move_undo_time_ns)
        }),
        hash_us => ("哈希耗时", |snapshot: &TreeStatsSnapshot| {
            total_us(snapshot.hash_time_ns)
        }),
        node_table_write_us => ("NodeTable写入耗时", |snapshot: &TreeStatsSnapshot| {
            total_us(snapshot.node_table_write_time_ns)
        }),
        node_table_lookup_us => ("NodeTable检索耗时", |snapshot: &TreeStatsSnapshot| {
            total_us(snapshot.node_table_lookup_time_ns)
        }),
        eval_us => ("评估耗时", |snapshot: &TreeStatsSnapshot| {
            total_us(snapshot.eval_time_ns)
        }),
        children_lock_us => ("子节点锁耗时", |snapshot: &TreeStatsSnapshot| {
            total_us(snapshot.children_lock_time_ns)
        }),
    }
}

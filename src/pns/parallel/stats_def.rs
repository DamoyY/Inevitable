use std::sync::atomic::{AtomicU64, Ordering};

use serde::Serialize;

use crate::{alloc_stats::AllocTimingSnapshot, game_state::MoveApplyTiming};

fn to_f64(value: u64) -> f64 {
    let value_u32 = u32::try_from(value).unwrap_or(u32::MAX);
    f64::from(value_u32)
}

fn avg_us(total_ns: u64, count: u64) -> f64 {
    if count > 0 {
        to_f64(total_ns) / to_f64(count) / 1_000.0
    } else {
        0.0
    }
}

macro_rules! define_metrics {
    (
        counts: { $( $cname:ident => $cdesc:expr ),* $(,)? }
        timings: { $( $tname:ident => $tdesc:expr ),* $(,)? }
        move_apply: { $( $tfield:ident <= $mfield:ident ),* $(,)? }
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
            pub const fn expand_other_ns(&self, alloc_timing: AllocTimingSnapshot) -> u64 {
                self.expand_time_ns
                    .saturating_sub(alloc_timing.total_ns())
                    .saturating_sub(self.movegen_time_ns)
                    .saturating_sub(self.board_update_time_ns)
                    .saturating_sub(self.bitboard_update_time_ns)
                    .saturating_sub(self.threat_index_update_time_ns)
                    .saturating_sub(self.candidate_remove_time_ns)
                    .saturating_sub(self.candidate_neighbor_time_ns)
                    .saturating_sub(self.candidate_insert_time_ns)
                    .saturating_sub(self.candidate_newly_added_time_ns)
                    .saturating_sub(self.candidate_history_time_ns)
                    .saturating_sub(self.hash_update_time_ns)
                    .saturating_sub(self.move_undo_time_ns)
                    .saturating_sub(self.hash_time_ns)
                    .saturating_sub(self.children_lock_time_ns)
                    .saturating_sub(self.node_table_lookup_time_ns)
                    .saturating_sub(self.node_table_write_time_ns)
                    .saturating_sub(self.eval_time_ns)
            }
        }

        #[derive(Default)]
        pub struct TreeStatsAccumulator {
            $(pub $cname: u64,)*
            $(pub $tname: u64,)*
        }

        impl TreeStatsAccumulator {
            pub const fn add_move_apply_timing(&mut self, timing: &MoveApplyTiming) {
                $(self.$tfield = self.$tfield.wrapping_add(timing.$mfield);)*
            }
        }

        pub struct TimingStats {
            $(pub $lname: f64,)*
        }

        impl TimingStats {
            #[must_use]
            pub fn from_snapshot(
                snapshot: &TreeStatsSnapshot,
                alloc_timing: AllocTimingSnapshot,
            ) -> Self {
                Self {
                    $($lname: ($calc)(snapshot, alloc_timing),)*
                }
            }

            pub const fn csv_headers() -> &'static [&'static str] {
                &[$($ldesc,)*]
            }

            #[must_use]
            pub fn csv_values(&self) -> Vec<f64> {
                vec![$(self.$lname,)*]
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
        board_update_time_ns => "基础棋盘状态更新耗时",
        bitboard_update_time_ns => "位棋盘更新耗时",
        threat_index_update_time_ns => "威胁索引增量更新耗时",
        candidate_remove_time_ns => "候选着法移除耗时",
        candidate_neighbor_time_ns => "邻居空位计算耗时",
        candidate_insert_time_ns => "候选着法更新耗时",
        candidate_newly_added_time_ns => "新增候选着法记录耗时",
        candidate_history_time_ns => "候选着法历史保存耗时",
        hash_update_time_ns => "Zobrist哈希增量更新耗时",
        move_undo_time_ns => "撤销耗时",
        hash_time_ns => "哈希耗时",
        children_lock_time_ns => "子节点锁耗时",
        node_table_lookup_time_ns => "NodeTable检索耗时",
        node_table_write_time_ns => "NodeTable写入耗时",
    }
    move_apply: {
        board_update_time_ns <= board_update_ns,
        bitboard_update_time_ns <= bitboard_update_ns,
        threat_index_update_time_ns <= threat_index_update_ns,
        candidate_remove_time_ns <= candidate_remove_ns,
        candidate_neighbor_time_ns <= candidate_neighbor_ns,
        candidate_insert_time_ns <= candidate_insert_ns,
        candidate_newly_added_time_ns <= candidate_newly_added_ns,
        candidate_history_time_ns <= candidate_history_ns,
        hash_update_time_ns <= hash_update_ns,
    }
    timing_log: {
        branch => ("平均分支数", |snapshot: &TreeStatsSnapshot, _| {
            if snapshot.expansions > 0 {
                to_f64(snapshot.children_generated) / to_f64(snapshot.expansions)
            } else {
                0.0
            }
        }),
        alloc_us => (
            "内存分配耗时",
            |snapshot: &TreeStatsSnapshot, alloc_timing: AllocTimingSnapshot| {
                avg_us(alloc_timing.alloc_ns, snapshot.expansions)
            }
        ),
        dealloc_us => (
            "内存释放耗时",
            |snapshot: &TreeStatsSnapshot, alloc_timing: AllocTimingSnapshot| {
                avg_us(alloc_timing.dealloc_ns, snapshot.expansions)
            }
        ),
        realloc_us => (
            "内存重分配耗时",
            |snapshot: &TreeStatsSnapshot, alloc_timing: AllocTimingSnapshot| {
                avg_us(alloc_timing.realloc_ns, snapshot.expansions)
            }
        ),
        alloc_zeroed_us => (
            "内存归零耗时",
            |snapshot: &TreeStatsSnapshot, alloc_timing: AllocTimingSnapshot| {
                avg_us(alloc_timing.alloc_zeroed_ns, snapshot.expansions)
            }
        ),
        movegen_us => ("平均走子耗时", |snapshot: &TreeStatsSnapshot, _| {
            avg_us(snapshot.movegen_time_ns, snapshot.expansions)
        }),
        board_update_us => ("基础棋盘状态更新耗时", |snapshot: &TreeStatsSnapshot, _| {
            avg_us(snapshot.board_update_time_ns, snapshot.expansions)
        }),
        bitboard_update_us => ("位棋盘更新耗时", |snapshot: &TreeStatsSnapshot, _| {
            avg_us(snapshot.bitboard_update_time_ns, snapshot.expansions)
        }),
        threat_index_update_us => ("威胁索引增量更新耗时", |snapshot: &TreeStatsSnapshot, _| {
            avg_us(snapshot.threat_index_update_time_ns, snapshot.expansions)
        }),
        candidate_remove_us => ("候选着法移除耗时", |snapshot: &TreeStatsSnapshot, _| {
            avg_us(snapshot.candidate_remove_time_ns, snapshot.expansions)
        }),
        candidate_neighbor_us => ("邻居空位计算耗时", |snapshot: &TreeStatsSnapshot, _| {
            avg_us(snapshot.candidate_neighbor_time_ns, snapshot.expansions)
        }),
        candidate_insert_us => ("候选着法更新耗时", |snapshot: &TreeStatsSnapshot, _| {
            avg_us(snapshot.candidate_insert_time_ns, snapshot.expansions)
        }),
        candidate_newly_added_us => ("新增候选着法记录耗时", |snapshot: &TreeStatsSnapshot, _| {
            avg_us(snapshot.candidate_newly_added_time_ns, snapshot.expansions)
        }),
        candidate_history_us => ("候选着法历史保存耗时", |snapshot: &TreeStatsSnapshot, _| {
            avg_us(snapshot.candidate_history_time_ns, snapshot.expansions)
        }),
        hash_update_us => ("Zobrist哈希增量更新耗时", |snapshot: &TreeStatsSnapshot, _| {
            avg_us(snapshot.hash_update_time_ns, snapshot.expansions)
        }),
        move_undo_us => ("平均撤销耗时", |snapshot: &TreeStatsSnapshot, _| {
            avg_us(snapshot.move_undo_time_ns, snapshot.expansions)
        }),
        hash_us => ("平均哈希耗时", |snapshot: &TreeStatsSnapshot, _| {
            avg_us(snapshot.hash_time_ns, snapshot.expansions)
        }),
        node_table_write_us => ("平均NodeTable写入耗时", |snapshot: &TreeStatsSnapshot, _| {
            avg_us(snapshot.node_table_write_time_ns, snapshot.expansions)
        }),
        node_table_lookup_us => ("平均NodeTable检索耗时", |snapshot: &TreeStatsSnapshot, _| {
            avg_us(snapshot.node_table_lookup_time_ns, snapshot.expansions)
        }),
        eval_us_per_expand => ("每扩展评估总耗耗时", |snapshot: &TreeStatsSnapshot, _| {
            avg_us(snapshot.eval_time_ns, snapshot.expansions)
        }),
        children_lock_us => ("平均子节点锁耗时", |snapshot: &TreeStatsSnapshot, _| {
            avg_us(snapshot.children_lock_time_ns, snapshot.expansions)
        }),
        expand_other_us => (
            "平均其他耗时",
            |snapshot: &TreeStatsSnapshot, alloc_timing: AllocTimingSnapshot| {
                avg_us(snapshot.expand_other_ns(alloc_timing), snapshot.expansions)
            }
        ),
        eval_us => ("单次评估函数耗时", |snapshot: &TreeStatsSnapshot, _| {
            avg_us(snapshot.eval_time_ns, snapshot.eval_calls)
        }),
    }
}

#[macro_export]
macro_rules! for_each_move_apply_timing {
    ($macro:ident) => {
        $macro! {
            board_update_ns => board_update_time_ns,
            bitboard_update_ns => bitboard_update_time_ns,
            threat_index_update_ns => threat_index_update_time_ns,
            candidate_remove_ns => candidate_remove_time_ns,
            candidate_neighbor_ns => candidate_neighbor_time_ns,
            candidate_insert_ns => candidate_insert_time_ns,
            candidate_newly_added_ns => candidate_newly_added_time_ns,
            candidate_history_ns => candidate_history_time_ns,
            hash_update_ns => hash_update_time_ns,
        }
    };
}

pub mod alloc_stats;
pub mod config;
pub mod game_state;
pub mod pns;
pub mod ui;
pub(crate) mod utils;

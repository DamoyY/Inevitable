use super::{
    Bitboard, BitboardWorkspace, Coord, GomokuEvaluator, GomokuMoveCache, GomokuPosition,
    GomokuRules, MoveApplyTiming, MoveGenBuffers, MoveGenTiming, record_duration_add_ns,
    record_duration_ns,
};
use crate::{checked, utils::duration_to_ns};
use smallvec::SmallVec;
use std::time::Instant;
fn bit_word_mut<'bits>(bits: &'bits mut [u64], word_index: usize, context: &str) -> &'bits mut u64 {
    let Some(word) = bits.get_mut(word_index) else {
        eprintln!("{context} 候选位图索引越界: {word_index}");
        panic!("{context} 候选位图索引越界");
    };
    word
}
impl GomokuRules {
    fn sort_scored_moves(scored_moves: &mut [(Coord, f32)]) {
        scored_moves.sort_unstable_by(|left, right| right.1.total_cmp(&left.1));
    }
    fn fill_moves_from_scored(moves: &mut Vec<Coord>, scored_moves: &[(Coord, f32)]) {
        moves.clear();
        moves.extend(scored_moves.iter().map(|scored_move| scored_move.0));
    }
    fn score_and_sort_moves_in_place(
        evaluator: &GomokuEvaluator,
        position: &GomokuPosition,
        player: u8,
        moves: &mut Vec<Coord>,
        scored_moves: &mut Vec<(Coord, f32)>,
    ) {
        if moves.len() <= 1 {
            return;
        }
        evaluator.score_moves_into(position, player, moves, scored_moves);
        Self::sort_scored_moves(scored_moves);
        Self::fill_moves_from_scored(moves, scored_moves);
    }
    fn score_and_sort_moves_in_place_with_proximity(
        evaluator: &GomokuEvaluator,
        position: &GomokuPosition,
        player: u8,
        moves: &mut Vec<Coord>,
        proximity_scores: &[f32],
        scored_moves: &mut Vec<(Coord, f32)>,
    ) {
        if moves.len() <= 1 {
            return;
        }
        evaluator.score_moves_into_with_proximity(
            position,
            player,
            moves,
            proximity_scores,
            scored_moves,
        );
        Self::sort_scored_moves(scored_moves);
        Self::fill_moves_from_scored(moves, scored_moves);
    }
    pub(crate) fn rebuild_candidate_moves(
        position: &GomokuPosition,
        cache: &mut GomokuMoveCache,
        workspace: &mut BitboardWorkspace,
    ) {
        let [occupied, neighbors, masked_not_left, masked_not_right, temp] = workspace.pads_mut();
        position.bitboard.occupied_into(occupied);
        if Bitboard::is_all_zeros(occupied) {
            cache.candidate_moves.fill(0);
            let center = checked::div_usize(
                position.board_size,
                2_usize,
                "GomokuRules::rebuild_candidate_moves",
            );
            position
                .bitboard
                .set_in(&mut cache.candidate_moves, center, center);
            return;
        }
        position.bitboard.neighbors_into(
            occupied,
            neighbors,
            masked_not_left,
            masked_not_right,
            temp,
        );
        if cache.candidate_moves.len() != neighbors.len() {
            cache.candidate_moves.resize(neighbors.len(), 0);
        }
        cache.candidate_moves.copy_from_slice(neighbors);
    }
    pub fn check_win(position: &GomokuPosition, player: u8) -> bool {
        position
            .threat_index
            .get_pattern_windows(player, position.win_len, 0)
            .next()
            .is_some()
    }
    fn collect_forcing_moves_bits<I>(
        position: &GomokuPosition,
        window_indices: I,
        bits: &mut Vec<u64>,
    ) where
        I: IntoIterator<Item = usize>,
    {
        let num_words = position.bitboard.num_words();
        if bits.len() != num_words {
            bits.resize(num_words, 0);
        }
        bits.fill(0);
        for window_idx in window_indices {
            let window = position.threat_index.window(window_idx);
            for &(row_index, column_index) in &window.coords {
                if position.cell(row_index, column_index) == 0 {
                    position.bitboard.set_in(bits, row_index, column_index);
                }
            }
        }
    }
    pub fn make_move(
        position: &mut GomokuPosition,
        cache: &mut GomokuMoveCache,
        mov: Coord,
        player: u8,
    ) {
        Self::make_move_with_timing(position, cache, mov, player);
    }
    pub fn make_move_with_timing(
        position: &mut GomokuPosition,
        cache: &mut GomokuMoveCache,
        mov: Coord,
        player: u8,
    ) -> MoveApplyTiming {
        let (row_index, column_index) = mov;
        let mut timing = MoveApplyTiming::zero();
        record_duration_ns(&mut timing.board_update_ns, || {
            position.set_cell(row_index, column_index, player);
        });
        record_duration_ns(&mut timing.bitboard_update_ns, || {
            position.bitboard.set(row_index, column_index, player);
        });
        record_duration_ns(&mut timing.threat_index_update_ns, || {
            position.threat_index.update_on_move(mov, player);
        });
        let mut newly_added_candidates: SmallVec<[Coord; 8]> = SmallVec::new();
        record_duration_ns(&mut timing.candidate_remove_ns, || {
            position
                .bitboard
                .clear_in(&mut cache.candidate_moves, row_index, column_index);
        });
        let mut row_start = 0_usize;
        let mut row_end = 0_usize;
        let mut column_start = 0_usize;
        let mut column_end = 0_usize;
        record_duration_ns(&mut timing.candidate_neighbor_ns, || {
            let last_board_index = checked::sub_usize(
                position.board_size,
                1_usize,
                "GomokuRules::make_move_with_timing::last_board_index",
            );
            row_start = if row_index == 0 {
                0
            } else {
                checked::sub_usize(
                    row_index,
                    1_usize,
                    "GomokuRules::make_move_with_timing::row_start",
                )
            };
            row_end = checked::add_usize(
                row_index,
                1_usize,
                "GomokuRules::make_move_with_timing::row_end_candidate",
            )
            .min(last_board_index);
            column_start = if column_index == 0 {
                0
            } else {
                checked::sub_usize(
                    column_index,
                    1_usize,
                    "GomokuRules::make_move_with_timing::column_start",
                )
            };
            column_end = checked::add_usize(
                column_index,
                1_usize,
                "GomokuRules::make_move_with_timing::column_end_candidate",
            )
            .min(last_board_index);
        });
        let mut candidate_insert_ns = 0_u64;
        let mut candidate_newly_added_ns = 0_u64;
        for neighbor_row_index in row_start..=row_end {
            for neighbor_column_index in column_start..=column_end {
                if position.cell(neighbor_row_index, neighbor_column_index) != 0 {
                    continue;
                }
                let coord = (neighbor_row_index, neighbor_column_index);
                let (word_idx, mask) = position.bitboard.coord_to_bit(coord.0, coord.1);
                let mut inserted = false;
                record_duration_add_ns(&mut candidate_insert_ns, || {
                    let candidate_word = bit_word_mut(
                        &mut cache.candidate_moves,
                        word_idx,
                        "GomokuRules::make_move_with_timing::candidate_insert",
                    );
                    inserted = *candidate_word & mask == 0;
                    *candidate_word |= mask;
                });
                if inserted {
                    record_duration_add_ns(&mut candidate_newly_added_ns, || {
                        newly_added_candidates.push(coord);
                    });
                }
            }
        }
        timing.candidate_insert_ns = candidate_insert_ns;
        timing.candidate_newly_added_ns = candidate_newly_added_ns;
        record_duration_ns(&mut timing.candidate_history_ns, || {
            cache
                .candidate_move_history
                .push((mov, newly_added_candidates));
        });
        record_duration_ns(&mut timing.hash_update_ns, || {
            position.hash ^= position
                .hasher
                .get_hash(row_index, column_index, usize::from(player));
            position.hash ^= position.hasher.side_to_move_hash;
        });
        timing
    }
    pub fn undo_move(
        position: &mut GomokuPosition,
        cache: &mut GomokuMoveCache,
        mov: Coord,
        player: u8,
    ) {
        let Some((undone_move, added_by_this_move)) = cache.candidate_move_history.pop() else {
            eprintln!(
                "GomokuRules::undo_move 候选历史为空，无法撤销: ({}, {})",
                mov.0, mov.1
            );
            panic!("GomokuRules::undo_move 候选历史为空");
        };
        let (row_index, column_index) = mov;
        position.threat_index.update_on_undo(mov, player);
        position.set_cell(row_index, column_index, 0);
        position
            .bitboard
            .clear_player(row_index, column_index, player);
        if undone_move != mov {
            eprintln!(
                "GomokuRules::undo_move 撤销着法不匹配: 实际 ({}, {})，期望 ({}, {})",
                undone_move.0, undone_move.1, mov.0, mov.1
            );
            panic!("GomokuRules::undo_move 撤销着法不匹配");
        }
        let (word_idx, mask) = position.bitboard.coord_to_bit(undone_move.0, undone_move.1);
        *bit_word_mut(
            &mut cache.candidate_moves,
            word_idx,
            "GomokuRules::undo_move::candidate_restore",
        ) |= mask;
        for added_coord in added_by_this_move {
            position
                .bitboard
                .clear_in(&mut cache.candidate_moves, added_coord.0, added_coord.1);
        }
        position.hash ^= position.hasher.side_to_move_hash;
        position.hash ^= position
            .hasher
            .get_hash(row_index, column_index, usize::from(player));
    }
    pub fn get_legal_moves_into(
        position: &GomokuPosition,
        evaluator: &GomokuEvaluator,
        player: u8,
        workspace: &mut BitboardWorkspace,
        buffers: &mut MoveGenBuffers<'_>,
    ) -> MoveGenTiming {
        let forcing_bits = &mut *buffers.forcing_bits;
        let scored_moves = &mut *buffers.scored_moves;
        let out_moves = &mut *buffers.out_moves;
        let candidate_moves = buffers.candidate_moves;
        let proximity_scores = buffers.proximity_scores;
        let mut timing = MoveGenTiming::default();
        let opponent = checked::opponent_player(player, "GomokuRules::get_legal_moves_into");
        let start_candidate = Instant::now();
        let win_minus_one = checked::sub_usize(
            position.win_len,
            1_usize,
            "GomokuRules::get_legal_moves_into::win_minus_one",
        );
        Self::collect_forcing_moves_bits(
            position,
            position
                .threat_index
                .get_pattern_windows(player, win_minus_one, 0),
            forcing_bits,
        );
        let found_my_win = !Bitboard::is_all_zeros(forcing_bits);
        timing.candidate_gen_ns = duration_to_ns(start_candidate.elapsed());
        if found_my_win {
            let start_collect = Instant::now();
            out_moves.clear();
            out_moves.extend(position.bitboard.iter_bits(forcing_bits));
            timing.candidate_gen_ns = checked::add_u64(
                timing.candidate_gen_ns,
                duration_to_ns(start_collect.elapsed()),
                "GomokuRules::get_legal_moves_into::candidate_collect_my_win",
            );
            return timing;
        }
        let start_threat = Instant::now();
        Self::collect_forcing_moves_bits(
            position,
            position
                .threat_index
                .get_pattern_windows(opponent, win_minus_one, 0),
            forcing_bits,
        );
        let found_opponent_threat = !Bitboard::is_all_zeros(forcing_bits);
        timing.candidate_gen_ns = checked::add_u64(
            timing.candidate_gen_ns,
            duration_to_ns(start_threat.elapsed()),
            "GomokuRules::get_legal_moves_into::candidate_collect_opponent_threat",
        );
        if found_opponent_threat {
            let start_collect = Instant::now();
            out_moves.clear();
            out_moves.extend(position.bitboard.iter_bits(forcing_bits));
            timing.candidate_gen_ns = checked::add_u64(
                timing.candidate_gen_ns,
                duration_to_ns(start_collect.elapsed()),
                "GomokuRules::get_legal_moves_into::candidate_collect_forced_reply",
            );
            record_duration_ns(&mut timing.scoring_ns, || {
                if let Some(existing_proximity_scores) = proximity_scores {
                    Self::score_and_sort_moves_in_place_with_proximity(
                        evaluator,
                        position,
                        player,
                        out_moves,
                        existing_proximity_scores,
                        scored_moves,
                    );
                } else {
                    Self::score_and_sort_moves_in_place(
                        evaluator,
                        position,
                        player,
                        out_moves,
                        scored_moves,
                    );
                }
            });
            return timing;
        }
        let start_empty = Instant::now();
        let [empty_bits, candidate_bits, deferred_bits, ..] = workspace.pads_mut();
        position.bitboard.empty_into(empty_bits);
        if Bitboard::is_all_zeros(empty_bits) {
            out_moves.clear();
            timing.candidate_gen_ns = checked::add_u64(
                timing.candidate_gen_ns,
                duration_to_ns(start_empty.elapsed()),
                "GomokuRules::get_legal_moves_into::candidate_collect_empty_board",
            );
            return timing;
        }
        out_moves.clear();
        let use_priority_candidates = if let Some(candidate_words) = candidate_moves
            && candidate_words.len() == empty_bits.len()
        {
            if candidate_bits.len() != empty_bits.len() {
                candidate_bits.resize(empty_bits.len(), 0);
            }
            if deferred_bits.len() != empty_bits.len() {
                deferred_bits.resize(empty_bits.len(), 0);
            }
            for ((candidate_word, deferred_word), (empty_word, cached_candidate_word)) in
                candidate_bits
                    .iter_mut()
                    .zip(deferred_bits.iter_mut())
                    .zip(empty_bits.iter().zip(candidate_words.iter()))
            {
                *candidate_word = *empty_word & *cached_candidate_word;
                *deferred_word = *empty_word & !*candidate_word;
            }
            out_moves.extend(position.bitboard.iter_bits(candidate_bits));
            !out_moves.is_empty()
        } else {
            false
        };
        if !use_priority_candidates {
            out_moves.extend(position.bitboard.iter_bits(empty_bits));
        }
        timing.candidate_gen_ns = checked::add_u64(
            timing.candidate_gen_ns,
            duration_to_ns(start_empty.elapsed()),
            "GomokuRules::get_legal_moves_into::candidate_collect_all_empty",
        );
        record_duration_ns(&mut timing.scoring_ns, || {
            if let Some(existing_proximity_scores) = proximity_scores {
                Self::score_and_sort_moves_in_place_with_proximity(
                    evaluator,
                    position,
                    player,
                    out_moves,
                    existing_proximity_scores,
                    scored_moves,
                );
            } else {
                Self::score_and_sort_moves_in_place(
                    evaluator,
                    position,
                    player,
                    out_moves,
                    scored_moves,
                );
            }
        });
        if use_priority_candidates {
            let start_deferred = Instant::now();
            out_moves.extend(position.bitboard.iter_bits(deferred_bits));
            timing.candidate_gen_ns = checked::add_u64(
                timing.candidate_gen_ns,
                duration_to_ns(start_deferred.elapsed()),
                "GomokuRules::get_legal_moves_into::candidate_collect_deferred_empty",
            );
        }
        timing
    }
}

use std::time::Instant;

use super::{
    Bitboard, BitboardWorkspace, Coord, GomokuEvaluator, GomokuMoveCache, GomokuPosition,
    GomokuRules, MoveApplyTiming, MoveGenBuffers, MoveGenTiming,
};
use crate::utils::duration_to_ns;

fn record_duration_ns<F: FnOnce()>(field: &mut u64, f: F) {
    let start = Instant::now();
    f();
    *field = duration_to_ns(start.elapsed());
}

fn record_duration_add_ns<F: FnOnce()>(field: &mut u64, f: F) {
    let start = Instant::now();
    f();
    *field = field.saturating_add(duration_to_ns(start.elapsed()));
}

impl GomokuRules {
    pub(crate) fn rebuild_candidate_moves(
        position: &GomokuPosition,
        cache: &mut GomokuMoveCache,
        workspace: &mut BitboardWorkspace,
    ) {
        let (occupied, neighbors, masked_not_left, masked_not_right, temp) = workspace.pads_mut();
        position.bitboard.occupied_into(occupied);
        if Bitboard::is_all_zeros(occupied) {
            cache.candidate_moves.fill(0);
            let center = position.board_size / 2;
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
            let window = &position.threat_index.all_windows[window_idx];
            for &(r, c) in &window.coords {
                if position.board[position.board_index(r, c)] == 0 {
                    position.bitboard.set_in(bits, r, c);
                }
            }
        }
    }

    fn sort_scored_moves(scored_moves: &mut [(Coord, f32)]) {
        scored_moves.sort_unstable_by(|a, b| b.1.total_cmp(&a.1));
    }

    fn fill_moves_from_scored(moves: &mut Vec<Coord>, scored_moves: &[(Coord, f32)]) {
        moves.clear();
        moves.extend(scored_moves.iter().map(|(coord, _)| *coord));
    }

    fn score_and_sort_moves_in_place(
        evaluator: &GomokuEvaluator,
        position: &GomokuPosition,
        player: u8,
        moves: &mut Vec<Coord>,
        score_buffer: &mut Vec<f32>,
        scored_moves: &mut Vec<(Coord, f32)>,
    ) {
        evaluator.score_moves_into(position, player, moves, score_buffer, scored_moves);
        Self::sort_scored_moves(scored_moves);
        Self::fill_moves_from_scored(moves, scored_moves);
    }

    fn score_and_sort_moves_in_place_with_proximity(
        evaluator: &GomokuEvaluator,
        position: &GomokuPosition,
        player: u8,
        moves: &mut Vec<Coord>,
        proximity_scores: &[f32],
        score_buffer: &mut Vec<f32>,
        scored_moves: &mut Vec<(Coord, f32)>,
    ) {
        evaluator.score_moves_into_with_proximity(
            position,
            player,
            moves,
            proximity_scores,
            score_buffer,
            scored_moves,
        );
        Self::sort_scored_moves(scored_moves);
        Self::fill_moves_from_scored(moves, scored_moves);
    }

    pub fn make_move(
        position: &mut GomokuPosition,
        cache: &mut GomokuMoveCache,
        mov: Coord,
        player: u8,
    ) {
        let _ = Self::make_move_with_timing(position, cache, mov, player);
    }

    pub fn make_move_with_timing(
        position: &mut GomokuPosition,
        cache: &mut GomokuMoveCache,
        mov: Coord,
        player: u8,
    ) -> MoveApplyTiming {
        let (r, c) = mov;
        let mut timing = MoveApplyTiming::zero();
        record_duration_ns(&mut timing.board_update_ns, || {
            let board_idx = position.board_index(r, c);
            position.board[board_idx] = player;
        });
        record_duration_ns(&mut timing.bitboard_update_ns, || {
            position.bitboard.set(r, c, player);
        });
        record_duration_ns(&mut timing.threat_index_update_ns, || {
            position.threat_index.update_on_move(mov, player);
        });
        let mut newly_added_candidates = position.bitboard.empty_mask();
        let mut neighbor_coords = Vec::new();
        record_duration_ns(&mut timing.candidate_remove_ns, || {
            position.bitboard.clear_in(&mut cache.candidate_moves, r, c);
        });
        record_duration_ns(&mut timing.candidate_neighbor_ns, || {
            let row_start = r.saturating_sub(1);
            let row_end = (r + 1).min(position.board_size - 1);
            let col_start = c.saturating_sub(1);
            let col_end = (c + 1).min(position.board_size - 1);
            for nr in row_start..=row_end {
                for nc in col_start..=col_end {
                    if position.board[position.board_index(nr, nc)] == 0 {
                        neighbor_coords.push((nr, nc));
                    }
                }
            }
        });
        let mut candidate_insert_ns = 0u64;
        let mut candidate_newly_added_ns = 0u64;
        for coord in neighbor_coords {
            let (word_idx, mask) = position.bitboard.coord_to_bit(coord.0, coord.1);
            let mut inserted = false;
            record_duration_add_ns(&mut candidate_insert_ns, || {
                inserted = cache.candidate_moves[word_idx] & mask == 0;
                cache.candidate_moves[word_idx] |= mask;
            });
            if inserted {
                record_duration_add_ns(&mut candidate_newly_added_ns, || {
                    newly_added_candidates[word_idx] |= mask;
                });
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
            position.hash ^= position.hasher.get_hash(r, c, player as usize);
            position.hash ^= position.hasher.side_to_move_hash;
        });
        timing
    }

    pub fn undo_move(position: &mut GomokuPosition, cache: &mut GomokuMoveCache, mov: Coord) {
        let Some((undone_move, added_by_this_move)) = cache.candidate_move_history.pop() else {
            return;
        };
        let (r, c) = mov;
        let board_idx = position.board_index(r, c);
        let player = position.board[board_idx];
        position.threat_index.update_on_undo(mov, player);
        position.board[board_idx] = 0;
        position.bitboard.clear(r, c);
        debug_assert_eq!(undone_move, mov, "Undo mismatch");
        let (word_idx, mask) = position.bitboard.coord_to_bit(undone_move.0, undone_move.1);
        cache.candidate_moves[word_idx] |= mask;
        for (candidate_word, added_word) in cache
            .candidate_moves
            .iter_mut()
            .zip(added_by_this_move.iter())
        {
            *candidate_word &= !added_word;
        }
        position.hash ^= position.hasher.side_to_move_hash;
        position.hash ^= position.hasher.get_hash(r, c, player as usize);
    }

    pub fn get_legal_moves_into(
        position: &GomokuPosition,
        evaluator: &GomokuEvaluator,
        player: u8,
        workspace: &mut BitboardWorkspace,
        buffers: &mut MoveGenBuffers<'_>,
    ) -> MoveGenTiming {
        let score_buffer = &mut *buffers.score_buffer;
        let forcing_bits = &mut *buffers.forcing_bits;
        let scored_moves = &mut *buffers.scored_moves;
        let out_moves = &mut *buffers.out_moves;
        let proximity_scores = buffers.proximity_scores;
        let mut timing = MoveGenTiming::default();
        let opponent = 3 - player;
        let start_candidate = Instant::now();
        Self::collect_forcing_moves_bits(
            position,
            position
                .threat_index
                .get_pattern_windows(player, position.win_len - 1, 0),
            forcing_bits,
        );
        let found_my_win = !Bitboard::is_all_zeros(forcing_bits);
        timing.candidate_gen_ns = duration_to_ns(start_candidate.elapsed());
        if found_my_win {
            let start_collect = Instant::now();
            out_moves.clear();
            out_moves.extend(position.bitboard.iter_bits(forcing_bits));
            timing.candidate_gen_ns += duration_to_ns(start_collect.elapsed());
            return timing;
        }
        let start_threat = Instant::now();
        Self::collect_forcing_moves_bits(
            position,
            position
                .threat_index
                .get_pattern_windows(opponent, position.win_len - 1, 0),
            forcing_bits,
        );
        let found_opponent_threat = !Bitboard::is_all_zeros(forcing_bits);
        timing.candidate_gen_ns += duration_to_ns(start_threat.elapsed());
        if found_opponent_threat {
            let start_collect = Instant::now();
            out_moves.clear();
            out_moves.extend(position.bitboard.iter_bits(forcing_bits));
            timing.candidate_gen_ns += duration_to_ns(start_collect.elapsed());
            record_duration_ns(&mut timing.scoring_ns, || {
                if let Some(proximity_scores) = proximity_scores {
                    Self::score_and_sort_moves_in_place_with_proximity(
                        evaluator,
                        position,
                        player,
                        out_moves,
                        proximity_scores,
                        score_buffer,
                        scored_moves,
                    );
                } else {
                    Self::score_and_sort_moves_in_place(
                        evaluator,
                        position,
                        player,
                        out_moves,
                        score_buffer,
                        scored_moves,
                    );
                }
            });
            return timing;
        }
        let start_empty = Instant::now();
        let (empty_bits, ..) = workspace.pads_mut();
        position.bitboard.empty_into(empty_bits);
        if Bitboard::is_all_zeros(empty_bits) {
            out_moves.clear();
            timing.candidate_gen_ns += duration_to_ns(start_empty.elapsed());
            return timing;
        }
        out_moves.clear();
        out_moves.extend(position.bitboard.iter_bits(empty_bits));
        timing.candidate_gen_ns += duration_to_ns(start_empty.elapsed());
        record_duration_ns(&mut timing.scoring_ns, || {
            if let Some(proximity_scores) = proximity_scores {
                Self::score_and_sort_moves_in_place_with_proximity(
                    evaluator,
                    position,
                    player,
                    out_moves,
                    proximity_scores,
                    score_buffer,
                    scored_moves,
                );
            } else {
                Self::score_and_sort_moves_in_place(
                    evaluator,
                    position,
                    player,
                    out_moves,
                    score_buffer,
                    scored_moves,
                );
            }
        });
        timing
    }
}

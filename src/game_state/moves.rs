use std::{collections::HashSet, time::Instant};

use super::{
    Bitboard, BitboardWorkspace, Coord, ForcingMoves, GomokuGameState, MoveApplyTiming,
    MoveGenTiming,
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
impl GomokuGameState {
    fn collect_empty_cells<I>(&self, window_indices: I) -> HashSet<Coord>
    where
        I: IntoIterator<Item = usize>,
    {
        let mut cells = HashSet::new();
        for window_idx in window_indices {
            let window = &self.threat_index.all_windows[window_idx];
            for &(r, c) in &window.coords {
                if self.board[self.board_index(r, c)] == 0 {
                    cells.insert((r, c));
                }
            }
        }
        cells
    }

    fn collect_forcing_moves_bits<I>(&self, window_indices: I, bits: &mut Vec<u64>)
    where
        I: IntoIterator<Item = usize>,
    {
        let num_words = self.bitboard.num_words();
        if bits.len() != num_words {
            bits.resize(num_words, 0);
        }
        bits.fill(0);
        for window_idx in window_indices {
            let window = &self.threat_index.all_windows[window_idx];
            for &(r, c) in &window.coords {
                if self.board[self.board_index(r, c)] == 0 {
                    self.bitboard.set_in(bits, r, c);
                }
            }
        }
    }

    fn score_and_sort_moves(
        &self,
        player: u8,
        moves: &[Coord],
        score_buffer: &mut Vec<f32>,
    ) -> Vec<Coord> {
        let mut scored_moves = self.score_moves(player, moves, score_buffer);
        scored_moves.sort_unstable_by(|a, b| b.1.total_cmp(&a.1));
        scored_moves.into_iter().map(|(coord, _)| coord).collect()
    }

    fn score_and_sort_moves_in_place(
        &self,
        player: u8,
        moves: &mut Vec<Coord>,
        score_buffer: &mut Vec<f32>,
        scored_moves: &mut Vec<(Coord, f32)>,
    ) {
        self.score_moves_into(player, moves, score_buffer, scored_moves);
        scored_moves.sort_unstable_by(|a, b| b.1.total_cmp(&a.1));
        moves.clear();
        moves.extend(scored_moves.iter().map(|(coord, _)| *coord));
    }

    pub fn make_move(&mut self, mov: Coord, player: u8) {
        let _ = self.make_move_with_timing(mov, player);
    }

    pub fn make_move_with_timing(&mut self, mov: Coord, player: u8) -> MoveApplyTiming {
        let (r, c) = mov;
        let mut timing = MoveApplyTiming::zero();
        record_duration_ns(&mut timing.board_update_ns, || {
            let board_idx = self.board_index(r, c);
            self.board[board_idx] = player;
        });
        record_duration_ns(&mut timing.bitboard_update_ns, || {
            self.bitboard.set(r, c, player);
        });
        record_duration_ns(&mut timing.threat_index_update_ns, || {
            self.threat_index.update_on_move(mov, player);
        });
        let mut newly_added_candidates = self.bitboard.empty_mask();
        let mut neighbor_coords = Vec::new();
        record_duration_ns(&mut timing.candidate_remove_ns, || {
            self.bitboard.clear_in(&mut self.candidate_moves, r, c);
        });
        record_duration_ns(&mut timing.candidate_neighbor_ns, || {
            let row_start = r.saturating_sub(1);
            let row_end = (r + 1).min(self.board_size - 1);
            let col_start = c.saturating_sub(1);
            let col_end = (c + 1).min(self.board_size - 1);
            for nr in row_start..=row_end {
                for nc in col_start..=col_end {
                    if self.board[self.board_index(nr, nc)] == 0 {
                        neighbor_coords.push((nr, nc));
                    }
                }
            }
        });
        let mut candidate_insert_ns = 0u64;
        let mut candidate_newly_added_ns = 0u64;
        for coord in neighbor_coords {
            let (word_idx, mask) = self.bitboard.coord_to_bit(coord.0, coord.1);
            let mut inserted = false;
            record_duration_add_ns(&mut candidate_insert_ns, || {
                inserted = self.candidate_moves[word_idx] & mask == 0;
                self.candidate_moves[word_idx] |= mask;
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
            self.candidate_move_history
                .push((mov, newly_added_candidates));
        });
        record_duration_ns(&mut timing.hash_update_ns, || {
            self.hash ^= self.hasher.get_hash(r, c, player as usize);
            self.hash ^= self.hasher.side_to_move_hash;
        });
        timing
    }

    pub fn undo_move(&mut self, mov: Coord) {
        let Some((undone_move, added_by_this_move)) = self.candidate_move_history.pop() else {
            return;
        };
        let (r, c) = mov;
        let board_idx = self.board_index(r, c);
        let player = self.board[board_idx];
        self.threat_index.update_on_undo(mov, player);
        self.board[board_idx] = 0;
        self.bitboard.clear(r, c);
        debug_assert_eq!(undone_move, mov, "Undo mismatch");
        let (word_idx, mask) = self.bitboard.coord_to_bit(undone_move.0, undone_move.1);
        self.candidate_moves[word_idx] |= mask;
        for (candidate_word, added_word) in self
            .candidate_moves
            .iter_mut()
            .zip(added_by_this_move.iter())
        {
            *candidate_word &= !added_word;
        }
        self.hash ^= self.hasher.side_to_move_hash;
        self.hash ^= self.hasher.get_hash(r, c, player as usize);
    }

    #[must_use]
    pub fn find_forcing_moves(&self, player: u8) -> ForcingMoves {
        let opponent = 3 - player;
        let win_windows = self
            .threat_index
            .get_pattern_windows(player, self.win_len - 1, 0);
        let win_in_one_moves = self.collect_empty_cells(win_windows);
        let threat_windows = self
            .threat_index
            .get_pattern_windows(opponent, self.win_len - 1, 0);
        let threat_moves = self.collect_empty_cells(threat_windows);
        (
            win_in_one_moves.into_iter().collect(),
            threat_moves.into_iter().collect(),
        )
    }

    #[must_use]
    pub fn get_legal_moves(
        &self,
        player: u8,
        workspace: &mut BitboardWorkspace,
        score_buffer: &mut Vec<f32>,
    ) -> Vec<Coord> {
        let (win_moves, threat_moves) = self.find_forcing_moves(player);
        if !win_moves.is_empty() {
            return win_moves;
        }
        if !threat_moves.is_empty() {
            return self.score_and_sort_moves(player, &threat_moves, score_buffer);
        }
        let (empty_bits, ..) = workspace.pads_mut();
        self.bitboard.empty_into(empty_bits);
        if Bitboard::is_all_zeros(empty_bits) {
            return Vec::new();
        }
        let empties: Vec<Coord> = self.bitboard.iter_bits(empty_bits).collect();
        self.score_and_sort_moves(player, &empties, score_buffer)
    }

    pub fn get_legal_moves_into(
        &self,
        player: u8,
        workspace: &mut BitboardWorkspace,
        score_buffer: &mut Vec<f32>,
        forcing_bits: &mut Vec<u64>,
        scored_moves: &mut Vec<(Coord, f32)>,
        out_moves: &mut Vec<Coord>,
    ) -> MoveGenTiming {
        let mut timing = MoveGenTiming::default();
        let opponent = 3 - player;
        let start_candidate = Instant::now();
        self.collect_forcing_moves_bits(
            self.threat_index
                .get_pattern_windows(player, self.win_len - 1, 0),
            forcing_bits,
        );
        let found_my_win = !Bitboard::is_all_zeros(forcing_bits);
        timing.candidate_gen_ns = duration_to_ns(start_candidate.elapsed());
        if found_my_win {
            let start_collect = Instant::now();
            out_moves.clear();
            out_moves.extend(self.bitboard.iter_bits(forcing_bits));
            timing.candidate_gen_ns += duration_to_ns(start_collect.elapsed());
            return timing;
        }
        let start_threat = Instant::now();
        self.collect_forcing_moves_bits(
            self.threat_index
                .get_pattern_windows(opponent, self.win_len - 1, 0),
            forcing_bits,
        );
        let found_opponent_threat = !Bitboard::is_all_zeros(forcing_bits);
        timing.candidate_gen_ns += duration_to_ns(start_threat.elapsed());
        if found_opponent_threat {
            let start_collect = Instant::now();
            out_moves.clear();
            out_moves.extend(self.bitboard.iter_bits(forcing_bits));
            timing.candidate_gen_ns += duration_to_ns(start_collect.elapsed());
            record_duration_ns(&mut timing.scoring_ns, || {
                self.score_and_sort_moves_in_place(player, out_moves, score_buffer, scored_moves);
            });
            return timing;
        }
        let start_empty = Instant::now();
        let (empty_bits, ..) = workspace.pads_mut();
        self.bitboard.empty_into(empty_bits);
        if Bitboard::is_all_zeros(empty_bits) {
            out_moves.clear();
            timing.candidate_gen_ns += duration_to_ns(start_empty.elapsed());
            return timing;
        }
        out_moves.clear();
        out_moves.extend(self.bitboard.iter_bits(empty_bits));
        timing.candidate_gen_ns += duration_to_ns(start_empty.elapsed());
        record_duration_ns(&mut timing.scoring_ns, || {
            self.score_and_sort_moves_in_place(player, out_moves, score_buffer, scored_moves);
        });
        timing
    }
}

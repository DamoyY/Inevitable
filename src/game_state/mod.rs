use std::{collections::HashSet, sync::Arc, time::Instant};

use smallvec::SmallVec;

use crate::utils::{board_index, duration_to_ns};
mod bitboard;
mod evaluation;
mod threat_index;
mod zobrist;
pub use bitboard::{Bitboard, BitboardWorkspace};
pub use threat_index::ThreatIndex;
pub use zobrist::ZobristHasher;
pub type Coord = (usize, usize);
pub type MoveHistory = Vec<(Coord, SmallVec<[u64; 8]>)>;
pub type ForcingMoves = (Vec<Coord>, Vec<Coord>);
macro_rules! define_move_apply_timing {
    ( $( $field:ident => $stat_field:ident ),* $(,)? ) => {
        pub struct MoveApplyTiming {
            $(pub $field: u64,)*
        }

        impl MoveApplyTiming {
            #[must_use]
            pub const fn zero() -> Self {
                Self {
                    $($field: 0,)*
                }
            }
        }
    };
}

crate::for_each_move_apply_timing!(define_move_apply_timing);

#[derive(Clone)]
pub struct GomokuGameState {
    pub board: Vec<u8>,
    pub bitboard: Bitboard,
    pub board_size: usize,
    pub win_len: usize,
    pub hasher: Arc<ZobristHasher>,
    pub hash: u64,
    pub threat_index: ThreatIndex,
    pub candidate_moves: SmallVec<[u64; 8]>,
    pub(crate) candidate_move_history: MoveHistory,
    pub(crate) proximity_kernel: Vec<Vec<f32>>,
    pub(crate) proximity_scale: f32,
    pub(crate) positional_bonus: Vec<f32>,
    pub(crate) proximity_maps: [Vec<f32>; 2],
}

impl GomokuGameState {
    #[must_use]
    pub fn new(
        initial_board: Vec<u8>,
        board_size: usize,
        hasher: Arc<ZobristHasher>,
        current_player: u8,
        win_len: usize,
    ) -> Self {
        let board = initial_board;
        debug_assert_eq!(board.len(), board_size.saturating_mul(board_size));
        let bitboard = Bitboard::from_board(&board, board_size);
        let candidate_moves = bitboard.empty_mask();
        let (proximity_kernel, proximity_scale) = Self::init_proximity_kernel(board_size);
        let positional_bonus = Self::init_positional_bonus(board_size);
        let mut state = Self {
            board,
            bitboard,
            board_size,
            win_len,
            hasher,
            hash: 0u64,
            threat_index: ThreatIndex::new(board_size, win_len),
            candidate_moves,
            candidate_move_history: Vec::new(),
            proximity_kernel,
            proximity_scale,
            positional_bonus,
            proximity_maps: [Vec::new(), Vec::new()],
        };
        state.rebuild_hashes(current_player);
        state.threat_index.initialize_from_board(&state.board);
        let mut workspace = BitboardWorkspace::new(state.bitboard.num_words());
        state.rebuild_candidate_moves(&mut workspace);
        state.rebuild_proximity_maps();
        state
    }

    #[inline]
    const fn board_index(&self, r: usize, c: usize) -> usize {
        board_index(self.board_size, r, c)
    }

    pub(crate) fn rebuild_candidate_moves(&mut self, workspace: &mut BitboardWorkspace) {
        let (occupied, neighbors, masked_not_left, masked_not_right, temp) = workspace.pads_mut();
        self.bitboard.occupied_into(occupied);
        if Bitboard::is_all_zeros(occupied) {
            self.candidate_moves.fill(0);
            let center = self.board_size / 2;
            self.bitboard
                .set_in(&mut self.candidate_moves, center, center);
            return;
        }
        self.bitboard
            .neighbors_into(occupied, neighbors, masked_not_left, masked_not_right, temp);
        if self.candidate_moves.len() != neighbors.len() {
            self.candidate_moves.resize(neighbors.len(), 0);
        }
        self.candidate_moves.copy_from_slice(neighbors);
    }

    pub(crate) fn rebuild_hashes(&mut self, player: u8) {
        self.hash = 0;
        for r in 0..self.board_size {
            for c in 0..self.board_size {
                let piece = self.board[self.board_index(r, c)];
                if piece != 0 {
                    self.hash ^= self.hasher.get_hash(r, c, piece as usize);
                }
            }
        }
        if player == 2 {
            self.hash ^= self.hasher.side_to_move_hash;
        }
    }

    #[must_use]
    pub fn get_canonical_hash(&self) -> u64 {
        let mut hashes = [0u64; 8];
        for r in 0..self.board_size {
            for c in 0..self.board_size {
                let piece = self.board[self.board_index(r, c)];
                if piece != 0 {
                    let symmetric_coords = self.hasher.get_symmetric_coords(r, c);
                    for (i, (sr, sc)) in symmetric_coords.iter().enumerate() {
                        hashes[i] ^= self.hasher.get_hash(*sr, *sc, piece as usize);
                    }
                }
            }
        }
        let base_hash = hashes[0];
        let side_hash = self.hasher.side_to_move_hash;
        let side_to_move_is_player2 = if self.hash == base_hash {
            false
        } else if self.hash == (base_hash ^ side_hash) {
            true
        } else {
            let mut count1 = 0usize;
            let mut count2 = 0usize;
            for &cell in &self.board {
                if cell == 1 {
                    count1 += 1;
                } else if cell == 2 {
                    count2 += 1;
                }
            }
            count1 > count2
        };
        if side_to_move_is_player2 {
            for hash in &mut hashes {
                *hash ^= side_hash;
            }
        }
        hashes.iter().copied().min().unwrap_or(0)
    }

    #[must_use]
    pub const fn get_hash(&self) -> u64 {
        self.hash
    }

    #[must_use]
    pub fn check_win(&self, player: u8) -> bool {
        self.threat_index
            .get_pattern_windows(player, self.win_len, 0)
            .next()
            .is_some()
    }
}

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
            cells.extend(window.empty_cells.iter());
        }
        cells
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
        self.apply_proximity_delta(mov, player, 1.0);
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
        self.apply_proximity_delta(mov, player, -1.0);
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
}

use std::sync::Arc;

use super::{Bitboard, BitboardWorkspace, GomokuGameState, ThreatIndex, ZobristHasher};
use crate::utils::board_index;
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
    pub(crate) const fn board_index(&self, r: usize, c: usize) -> usize {
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

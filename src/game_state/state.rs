use std::sync::Arc;

use super::{
    Bitboard, BitboardWorkspace, GomokuEvaluator, GomokuGameState, GomokuMoveCache, GomokuPosition,
    GomokuRules, ThreatIndex, ZobristHasher,
};
use crate::{config::EvaluationConfig, utils::board_index};

impl GomokuGameState {
    #[must_use]
    pub fn new(
        initial_board: Vec<u8>,
        board_size: usize,
        hasher: Arc<ZobristHasher>,
        current_player: u8,
        win_len: usize,
        evaluation: EvaluationConfig,
    ) -> Self {
        let mut position =
            GomokuPosition::new(initial_board, board_size, hasher, current_player, win_len);
        position.threat_index.initialize_from_board(&position.board);
        let evaluator = GomokuEvaluator::new(board_size, evaluation);
        let mut move_cache = GomokuMoveCache::new(&position.bitboard);
        let mut workspace = BitboardWorkspace::new(position.bitboard.num_words());
        GomokuRules::rebuild_candidate_moves(&position, &mut move_cache, &mut workspace);
        Self {
            position,
            evaluator,
            move_cache,
        }
    }
}

impl GomokuPosition {
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
        let mut position = Self {
            board,
            bitboard,
            board_size,
            win_len,
            hasher,
            hash: 0u64,
            threat_index: ThreatIndex::new(board_size, win_len),
        };
        position.rebuild_hashes(current_player);
        position
    }

    #[inline]
    pub(crate) const fn board_index(&self, r: usize, c: usize) -> usize {
        board_index(self.board_size, r, c)
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
}
impl GomokuMoveCache {
    pub(crate) fn new(bitboard: &Bitboard) -> Self {
        Self {
            candidate_moves: bitboard.empty_mask(),
            candidate_move_history: Vec::new(),
        }
    }
}

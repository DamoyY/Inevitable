use std::{collections::HashSet, sync::Arc};

mod bitboard;
mod evaluation;
mod logic;
mod threat_index;
mod zobrist;

pub use bitboard::Bitboard;
pub use threat_index::ThreatIndex;
pub use zobrist::ZobristHasher;

pub type Coord = (usize, usize);
pub type MoveHistory = Vec<(Coord, HashSet<Coord>)>;
pub type ForcingMoves = (Vec<Coord>, Vec<Coord>);

pub struct GomokuGameState {
    pub board: Vec<Vec<u8>>,
    pub bitboard: Bitboard,
    pub board_size: usize,
    pub win_len: usize,
    pub hasher: Arc<ZobristHasher>,
    pub hash: u64,
    pub threat_index: ThreatIndex,
    pub candidate_moves: HashSet<Coord>,
    pub(crate) candidate_move_history: MoveHistory,
    pub(crate) proximity_kernel: Vec<Vec<f32>>,
    pub(crate) proximity_scale: f32,
    pub(crate) positional_bonus: Vec<Vec<f32>>,
}

impl GomokuGameState {
    #[must_use]
    pub fn new(
        initial_board: Vec<Vec<u8>>,
        hasher: Arc<ZobristHasher>,
        current_player: u8,
        win_len: usize,
    ) -> Self {
        let board = initial_board;
        let board_size = board.len();
        let bitboard = Bitboard::from_board(&board);
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
            candidate_moves: HashSet::new(),
            candidate_move_history: Vec::new(),
            proximity_kernel,
            proximity_scale,
            positional_bonus,
        };
        state.rebuild_hashes(current_player);
        state.threat_index.initialize_from_board(&state.board);
        state.rebuild_candidate_moves();
        state
    }

    pub(crate) fn rebuild_candidate_moves(&mut self) {
        self.candidate_moves.clear();
        let occupied = self.bitboard.occupied();
        if Bitboard::is_all_zeros(&occupied) {
            let center = self.board_size / 2;
            self.candidate_moves.insert((center, center));
            return;
        }
        let neighbors = self.bitboard.neighbors(&occupied);
        for coord in self.bitboard.iter_bits(&neighbors) {
            self.candidate_moves.insert(coord);
        }
    }

    pub(crate) fn rebuild_hashes(&mut self, player: u8) {
        self.hash = 0;
        for r in 0..self.board_size {
            for c in 0..self.board_size {
                let piece = self.board[r][c];
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
                let piece = self.board[r][c];
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
            for row in &self.board {
                for &cell in row {
                    if cell == 1 {
                        count1 += 1;
                    } else if cell == 2 {
                        count2 += 1;
                    }
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
        !self
            .threat_index
            .get_pattern_windows(player, self.win_len, 0)
            .is_empty()
    }
}

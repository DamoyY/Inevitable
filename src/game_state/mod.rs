use std::collections::HashSet;

use rand::Rng;

use crate::threat_index::ThreatIndex;

pub type Coord = (usize, usize);
pub type MoveHistory = Vec<(Coord, HashSet<Coord>)>;
pub type ForcingMoves = (Vec<Coord>, Vec<Coord>);

pub struct ZobristHasher {
    pub(crate) board_size: usize,
    pub(crate) zobrist_table: Vec<Vec<[u64; 3]>>,
    pub side_to_move_hash: u64,
}

impl ZobristHasher {
    pub fn new(board_size: usize) -> Self {
        let mut rng = rand::thread_rng();
        let mut zobrist_table = vec![vec![[0u64; 3]; board_size]; board_size];

        for row in zobrist_table.iter_mut() {
            for cell in row.iter_mut() {
                for piece in cell.iter_mut() {
                    *piece = rng.r#gen::<u64>() & ((1u64 << 63) - 1);
                }
            }
        }

        let side_to_move_hash = rng.r#gen::<u64>() & ((1u64 << 63) - 1);

        Self {
            board_size,
            zobrist_table,
            side_to_move_hash,
        }
    }

    pub fn get_hash(&self, r: usize, c: usize, piece: usize) -> u64 {
        self.zobrist_table[r][c][piece]
    }

    pub fn get_symmetric_coords(&self, r: usize, c: usize) -> [(usize, usize); 8] {
        let n = self.board_size - 1;
        [
            (r, c),
            (c, n - r),
            (n - r, n - c),
            (n - c, r),
            (r, n - c),
            (c, r),
            (n - r, c),
            (n - c, n - r),
        ]
    }
}

pub struct GomokuGameState {
    pub board: Vec<Vec<u8>>,
    pub board_size: usize,
    pub win_len: usize,
    pub hasher: ZobristHasher,
    pub hashes: [u64; 8],
    pub threat_index: ThreatIndex,
    pub candidate_moves: HashSet<Coord>,
    pub(crate) candidate_move_history: MoveHistory,
    pub(crate) proximity_kernel: Vec<Vec<f32>>,
    pub(crate) proximity_scale: f32,
    pub(crate) positional_bonus: Vec<Vec<f32>>,
}

mod logic;

impl GomokuGameState {
    pub fn new(
        initial_board: Vec<Vec<u8>>,
        hasher: ZobristHasher,
        current_player: u8,
        win_len: usize,
    ) -> Self {
        let board_size = initial_board.len();

        let k_size = 7;
        let k_center = k_size / 2;
        let mut proximity_kernel = vec![vec![0.0f32; k_size]; k_size];
        for (r, row) in proximity_kernel.iter_mut().enumerate() {
            for (c, cell) in row.iter_mut().enumerate() {
                let dist = (r as i32 - k_center as i32).abs() + (c as i32 - k_center as i32).abs();
                *cell = 1.0 / (dist as f32 + 1.0);
            }
        }

        let center = board_size / 2;
        let mut positional_bonus = vec![vec![0.0f32; board_size]; board_size];
        for (r, row) in positional_bonus.iter_mut().enumerate() {
            for (c, cell) in row.iter_mut().enumerate() {
                let bonus = (center as i32 - (r as i32 - center as i32).abs())
                    + (center as i32 - (c as i32 - center as i32).abs());
                *cell = bonus as f32 * 0.1;
            }
        }

        let mut state = Self {
            board: initial_board,
            board_size,
            win_len,
            hasher,
            hashes: [0u64; 8],
            threat_index: ThreatIndex::new(board_size, win_len),
            candidate_moves: HashSet::new(),
            candidate_move_history: Vec::new(),
            proximity_kernel,
            proximity_scale: 60.0,
            positional_bonus,
        };

        state.rebuild_hashes(current_player);
        state.threat_index.initialize_from_board(&state.board);
        state.rebuild_candidate_moves();

        state
    }

    pub(crate) fn rebuild_candidate_moves(&mut self) {
        self.candidate_moves.clear();
        let has_stones = self
            .board
            .iter()
            .any(|row| row.iter().any(|&cell| cell != 0));

        if !has_stones {
            let center = self.board_size / 2;
            self.candidate_moves.insert((center, center));
            return;
        }

        for r in 0..self.board_size {
            for c in 0..self.board_size {
                if self.board[r][c] != 0 {
                    for dr in -1i32..=1 {
                        for dc in -1i32..=1 {
                            if dr == 0 && dc == 0 {
                                continue;
                            }
                            let nr = r as i32 + dr;
                            let nc = c as i32 + dc;
                            if nr >= 0
                                && nr < self.board_size as i32
                                && nc >= 0
                                && nc < self.board_size as i32
                            {
                                let nr = nr as usize;
                                let nc = nc as usize;
                                if self.board[nr][nc] == 0 {
                                    self.candidate_moves.insert((nr, nc));
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    pub(crate) fn rebuild_hashes(&mut self, player: u8) {
        self.hashes = [0u64; 8];
        for r in 0..self.board_size {
            for c in 0..self.board_size {
                let piece = self.board[r][c];
                if piece != 0 {
                    let symmetric_coords = self.hasher.get_symmetric_coords(r, c);
                    for (i, (sr, sc)) in symmetric_coords.iter().enumerate() {
                        self.hashes[i] ^= self.hasher.get_hash(*sr, *sc, piece as usize);
                    }
                }
            }
        }
        if player == 2 {
            for hash in &mut self.hashes {
                *hash ^= self.hasher.side_to_move_hash;
            }
        }
    }

    pub fn get_canonical_hash(&self) -> u64 {
        *self.hashes.iter().min().unwrap()
    }

    pub fn check_win(&self, player: u8) -> bool {
        !self
            .threat_index
            .get_pattern_windows(player, self.win_len, 0)
            .is_empty()
    }
}

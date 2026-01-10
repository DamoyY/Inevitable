use std::{collections::HashSet, sync::Arc};
mod bitboard;
mod evaluation;
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
    fn neighbor_coords(&self) -> Vec<Coord> {
        let occupied = self.bitboard.occupied();
        let neighbors = self.bitboard.neighbors(&occupied);
        self.bitboard.iter_bits(&neighbors).collect()
    }

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
        for coord in self.neighbor_coords() {
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
        self.threat_index
            .get_pattern_windows(player, self.win_len, 0)
            .next()
            .is_some()
    }
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

    fn score_and_sort_moves(&self, player: u8, moves: &[Coord]) -> Vec<Coord> {
        let mut scored_moves = self.score_moves(player, moves);
        scored_moves.sort_by(|a, b| b.1.total_cmp(&a.1));
        scored_moves.into_iter().map(|(coord, _)| coord).collect()
    }

    pub fn make_move(&mut self, mov: Coord, player: u8) {
        let (r, c) = mov;
        self.board[r][c] = player;
        self.bitboard.set(r, c, player);
        self.threat_index.update_on_move(mov, player);
        let mut newly_added_candidates = HashSet::new();
        self.candidate_moves.remove(&mov);
        for coord in self.neighbor_coords() {
            if self.candidate_moves.insert(coord) {
                newly_added_candidates.insert(coord);
            }
        }
        self.candidate_move_history
            .push((mov, newly_added_candidates));
        self.hash ^= self.hasher.get_hash(r, c, player as usize);
        self.hash ^= self.hasher.side_to_move_hash;
    }

    pub fn undo_move(&mut self, mov: Coord) {
        let Some((undone_move, added_by_this_move)) = self.candidate_move_history.pop() else {
            return;
        };
        let (r, c) = mov;
        let player = self.board[r][c];
        self.threat_index.update_on_undo(mov, player);
        self.board[r][c] = 0;
        self.bitboard.clear(r, c);
        debug_assert_eq!(undone_move, mov, "Undo mismatch");
        self.candidate_moves.insert(undone_move);
        for m in added_by_this_move {
            self.candidate_moves.remove(&m);
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
    pub fn get_legal_moves(&self, player: u8) -> Vec<Coord> {
        let (win_moves, threat_moves) = self.find_forcing_moves(player);

        if !win_moves.is_empty() {
            return win_moves;
        }
        if !threat_moves.is_empty() {
            return self.score_and_sort_moves(player, &threat_moves);
        }
        let empty_bits = self.bitboard.empty();
        if Bitboard::is_all_zeros(&empty_bits) {
            return Vec::new();
        }
        let empties: Vec<Coord> = self.bitboard.iter_bits(&empty_bits).collect();
        self.score_and_sort_moves(player, &empties)
    }
}

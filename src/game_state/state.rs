use super::{
    Bitboard, BitboardWorkspace, GameState, GomokuEvaluator, GomokuMoveCache, GomokuPosition,
    GomokuRules, ThreatIndex,
};
use crate::{checked, config::EvaluationWeights, utils::board_index};
use alloc::sync::Arc;
use rand::rngs::StdRng;
const ZOBRIST_HASH_MASK: u64 = 0x7FFF_FFFF_FFFF_FFFF;
pub struct ZobristHasher {
    pub(crate) board_size: usize,
    pub(crate) zobrist_table: Vec<Vec<[u64; 3]>>,
    pub(crate) side_to_move_hash: u64,
}
impl ZobristHasher {
    #[inline]
    #[must_use]
    pub fn new(board_size: usize) -> Self {
        Self::with_seed(board_size, 0x005F_15E5_D0FE_DF9A)
    }
    #[inline]
    #[must_use]
    pub fn with_seed(board_size: usize, seed: u64) -> Self {
        let mut rng = <StdRng as rand::SeedableRng>::seed_from_u64(seed);
        let mut zobrist_table = vec![vec![[0_u64; 3]; board_size]; board_size];
        for row in &mut zobrist_table {
            for cell in row.iter_mut() {
                for piece in cell.iter_mut() {
                    *piece = <StdRng as rand::RngExt>::random::<u64>(&mut rng) & ZOBRIST_HASH_MASK;
                }
            }
        }
        let side_to_move_hash =
            <StdRng as rand::RngExt>::random::<u64>(&mut rng) & ZOBRIST_HASH_MASK;
        Self {
            board_size,
            zobrist_table,
            side_to_move_hash,
        }
    }
    fn row(&self, row_index: usize) -> &Vec<[u64; 3]> {
        let Some(row) = self.zobrist_table.get(row_index) else {
            eprintln!("ZobristHasher::row 行索引越界: {row_index}");
            panic!("ZobristHasher::row 行索引越界");
        };
        row
    }
    fn row_cell(&self, row_index: usize, column_index: usize) -> &[u64; 3] {
        let row = self.row(row_index);
        let Some(cell) = row.get(column_index) else {
            eprintln!("ZobristHasher::row_cell 列索引越界: ({row_index}, {column_index})");
            panic!("ZobristHasher::row_cell 列索引越界");
        };
        cell
    }
    #[inline]
    #[must_use]
    pub(crate) fn get_hash(&self, row_index: usize, column_index: usize, piece: usize) -> u64 {
        let cell = self.row_cell(row_index, column_index);
        let Some(&hash) = cell.get(piece) else {
            eprintln!(
                "ZobristHasher::get_hash 棋子索引越界: ({row_index}, {column_index}, {piece})"
            );
            panic!("ZobristHasher::get_hash 棋子索引越界");
        };
        hash
    }
    #[inline]
    #[must_use]
    pub(crate) fn get_symmetric_coords(
        &self,
        row_index: usize,
        column_index: usize,
    ) -> [(usize, usize); 8] {
        let last_index = checked::sub_usize(
            self.board_size,
            1_usize,
            "ZobristHasher::get_symmetric_coords::last_index",
        );
        let rotated_row = checked::sub_usize(
            last_index,
            row_index,
            "ZobristHasher::get_symmetric_coords::rotated_row",
        );
        let rotated_column = checked::sub_usize(
            last_index,
            column_index,
            "ZobristHasher::get_symmetric_coords::rotated_column",
        );
        [
            (row_index, column_index),
            (column_index, rotated_row),
            (rotated_row, rotated_column),
            (rotated_column, row_index),
            (row_index, rotated_column),
            (column_index, row_index),
            (rotated_row, column_index),
            (rotated_column, rotated_row),
        ]
    }
}
impl GameState {
    #[inline]
    #[must_use]
    pub fn new(
        initial_board: Vec<u8>,
        board_size: usize,
        hasher: Arc<ZobristHasher>,
        current_player: u8,
        win_len: usize,
        evaluation: EvaluationWeights,
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
    #[inline]
    #[must_use]
    pub(crate) fn new(
        initial_board: Vec<u8>,
        board_size: usize,
        hasher: Arc<ZobristHasher>,
        current_player: u8,
        win_len: usize,
    ) -> Self {
        let board = initial_board;
        let expected_len =
            checked::mul_usize(board_size, board_size, "GomokuPosition::new::expected_len");
        if board.len() != expected_len {
            eprintln!(
                "GomokuPosition::new 棋盘长度不匹配: 实际 {}, 期望 {}",
                board.len(),
                expected_len
            );
            panic!("GomokuPosition::new 棋盘长度不匹配");
        }
        let bitboard = Bitboard::from_board(&board, board_size);
        let mut position = Self {
            board,
            bitboard,
            board_size,
            win_len,
            hasher,
            hash: 0_u64,
            threat_index: ThreatIndex::new(board_size, win_len),
        };
        position.rebuild_hashes(current_player);
        position
    }
    #[inline]
    pub(crate) fn board_index(&self, row_index: usize, column_index: usize) -> usize {
        board_index(self.board_size, row_index, column_index)
    }
    pub(crate) fn cell(&self, row_index: usize, column_index: usize) -> u8 {
        let board_index = self.board_index(row_index, column_index);
        let Some(&cell) = self.board.get(board_index) else {
            eprintln!("GomokuPosition::cell 棋盘索引越界: ({row_index}, {column_index})");
            panic!("GomokuPosition::cell 棋盘索引越界");
        };
        cell
    }
    pub(crate) fn set_cell(&mut self, row_index: usize, column_index: usize, player: u8) {
        let board_index = self.board_index(row_index, column_index);
        let Some(cell) = self.board.get_mut(board_index) else {
            eprintln!("GomokuPosition::set_cell 棋盘索引越界: ({row_index}, {column_index})");
            panic!("GomokuPosition::set_cell 棋盘索引越界");
        };
        *cell = player;
    }
    pub(crate) fn rebuild_hashes(&mut self, player: u8) {
        self.hash = 0;
        for row_index in 0..self.board_size {
            for column_index in 0..self.board_size {
                let piece = self.cell(row_index, column_index);
                if piece != 0 {
                    self.hash ^= self
                        .hasher
                        .get_hash(row_index, column_index, usize::from(piece));
                }
            }
        }
        if player == 2 {
            self.hash ^= self.hasher.side_to_move_hash;
        }
    }
    #[inline]
    #[must_use]
    pub(crate) fn get_canonical_hash(&self) -> u64 {
        let mut hashes = [0_u64; 8];
        for row_index in 0..self.board_size {
            for column_index in 0..self.board_size {
                let piece = self.cell(row_index, column_index);
                if piece != 0 {
                    let symmetric_coords =
                        self.hasher.get_symmetric_coords(row_index, column_index);
                    for (hash_index, symmetric_coord) in symmetric_coords.into_iter().enumerate() {
                        let (symmetric_row, symmetric_column) = symmetric_coord;
                        let Some(hash) = hashes.get_mut(hash_index) else {
                            eprintln!(
                                "GomokuPosition::get_canonical_hash 哈希数组索引越界: {hash_index}"
                            );
                            panic!("GomokuPosition::get_canonical_hash 哈希数组索引越界");
                        };
                        *hash ^= self.hasher.get_hash(
                            symmetric_row,
                            symmetric_column,
                            usize::from(piece),
                        );
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
            let mut count1 = 0_usize;
            let mut count2 = 0_usize;
            for &cell in &self.board {
                if cell == 1 {
                    count1 = checked::add_usize(
                        count1,
                        1_usize,
                        "GomokuPosition::get_canonical_hash::count1",
                    );
                } else if cell == 2 {
                    count2 = checked::add_usize(
                        count2,
                        1_usize,
                        "GomokuPosition::get_canonical_hash::count2",
                    );
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
    #[inline]
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

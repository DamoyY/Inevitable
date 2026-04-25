use super::{Bitboard, WORD_BITS, bit_mask, word_mut, words_for_bits};
use crate::checked;
use smallvec::{SmallVec, smallvec};
impl Bitboard {
    #[inline]
    #[must_use]
    pub fn new(board_size: usize) -> Self {
        let total_bits = checked::mul_usize(board_size, board_size, "Bitboard::new::total_bits");
        let num_words = words_for_bits(total_bits);
        Self {
            black: smallvec ! [0 ; num_words],
            white: smallvec ! [0 ; num_words],
            size: board_size,
            num_words,
        }
    }
    #[inline]
    #[must_use]
    pub const fn num_words(&self) -> usize {
        self.num_words
    }
    #[inline]
    fn coord_to_index(&self, row_index: usize, column_index: usize) -> (usize, usize) {
        let row_offset =
            checked::mul_usize(row_index, self.size, "Bitboard::coord_to_index::row_offset");
        let bit_pos = checked::add_usize(
            row_offset,
            column_index,
            "Bitboard::coord_to_index::bit_pos",
        );
        (
            checked::div_usize(bit_pos, WORD_BITS, "Bitboard::coord_to_index::word"),
            checked::rem_usize(bit_pos, WORD_BITS, "Bitboard::coord_to_index::bit"),
        )
    }
    #[inline]
    #[must_use]
    pub fn coord_to_bit(&self, row_index: usize, column_index: usize) -> (usize, u64) {
        let (word_index, bit_index) = self.coord_to_index(row_index, column_index);
        (word_index, bit_mask(bit_index, "Bitboard::coord_to_bit"))
    }
    #[inline]
    #[must_use]
    pub fn empty_mask(&self) -> SmallVec<[u64; 8]> {
        smallvec ! [0_u64 ; self . num_words]
    }
    #[inline]
    pub(in crate::game_state) fn set_in(
        &self,
        bits: &mut [u64],
        row_index: usize,
        column_index: usize,
    ) -> bool {
        let (word_index, mask) = self.coord_to_bit(row_index, column_index);
        let word = word_mut(bits, word_index, "Bitboard::set_in");
        let was_set = *word & mask != 0;
        *word |= mask;
        !was_set
    }
    #[inline]
    pub(in crate::game_state) fn clear_in(
        &self,
        bits: &mut [u64],
        row_index: usize,
        column_index: usize,
    ) -> bool {
        let (word_index, mask) = self.coord_to_bit(row_index, column_index);
        let word = word_mut(bits, word_index, "Bitboard::clear_in");
        let was_set = *word & mask != 0;
        *word &= !mask;
        was_set
    }
    #[inline]
    pub(in crate::game_state) fn set(&mut self, row_index: usize, column_index: usize, player: u8) {
        let (word_index, bit) = self.coord_to_bit(row_index, column_index);
        match player {
            1 => *word_mut(&mut self.black, word_index, "Bitboard::set::black") |= bit,
            2 => *word_mut(&mut self.white, word_index, "Bitboard::set::white") |= bit,
            _ => {
                eprintln!("Bitboard::set 收到非法玩家编号: {player}");
                panic!("Bitboard::set 收到非法玩家编号");
            }
        }
    }
    #[inline]
    pub(in crate::game_state) fn clear(&mut self, row_index: usize, column_index: usize) {
        let (word_index, bit) = self.coord_to_bit(row_index, column_index);
        *word_mut(&mut self.black, word_index, "Bitboard::clear::black") &= !bit;
        *word_mut(&mut self.white, word_index, "Bitboard::clear::white") &= !bit;
    }
    #[inline]
    pub(in crate::game_state) fn occupied_into(&self, target: &mut Vec<u64>) {
        self.resize_target(target);
        for ((target_word, black_word), white_word) in target
            .iter_mut()
            .zip(self.black.iter())
            .zip(self.white.iter())
        {
            *target_word = *black_word | *white_word;
        }
    }
    #[inline]
    pub(in crate::game_state) fn empty_into(&self, target: &mut Vec<u64>) {
        self.resize_target(target);
        for ((target_word, black_word), white_word) in target
            .iter_mut()
            .zip(self.black.iter())
            .zip(self.white.iter())
        {
            *target_word = !(*black_word | *white_word);
        }
        self.apply_mask(target);
    }
    #[inline]
    #[must_use]
    pub fn is_all_zeros(bits: &[u64]) -> bool {
        bits.iter().all(|&word| word == 0)
    }
    #[inline]
    #[must_use]
    pub(in crate::game_state) fn from_board(board: &[u8], board_size: usize) -> Self {
        let expected_len =
            checked::mul_usize(board_size, board_size, "Bitboard::from_board::expected_len");
        if board.len() != expected_len {
            eprintln!(
                "Bitboard::from_board 棋盘长度不匹配: 实际 {}, 期望 {}",
                board.len(),
                expected_len
            );
            panic!("Bitboard::from_board 棋盘长度不匹配");
        }
        let mut bitboard = Self::new(board_size);
        for (flat_index, &cell) in board.iter().enumerate() {
            let row_index = checked::div_usize(flat_index, board_size, "Bitboard::from_board::row");
            let column_index =
                checked::rem_usize(flat_index, board_size, "Bitboard::from_board::column");
            match cell {
                0 => {}
                1 => bitboard.set(row_index, column_index, 1),
                2 => bitboard.set(row_index, column_index, 2),
                _ => {
                    eprintln!("Bitboard::from_board 收到非法棋子编号: {cell}");
                    panic!("Bitboard::from_board 收到非法棋子编号");
                }
            }
        }
        bitboard
    }
    fn last_word_mask(&self) -> u64 {
        let total_bits = checked::mul_usize(self.size, self.size, "Bitboard::last_word_mask");
        let bits_in_last = checked::rem_usize(total_bits, WORD_BITS, "Bitboard::last_word_mask");
        if bits_in_last == 0 {
            u64::MAX
        } else {
            checked::sub_u64(
                checked::shl_u64(1_u64, bits_in_last, "Bitboard::last_word_mask"),
                1_u64,
                "Bitboard::last_word_mask",
            )
        }
    }
    pub(super) fn validate_bits_len(&self, bits: &[u64], context: &str) {
        if bits.len() != self.num_words {
            eprintln!(
                "{context} 位棋盘字数不匹配: 实际 {}, 期望 {}",
                bits.len(),
                self.num_words
            );
            panic!("{context} 位棋盘字数不匹配");
        }
    }
    pub(super) fn resize_target(&self, target: &mut Vec<u64>) {
        if target.len() != self.num_words {
            target.resize(self.num_words, 0);
        }
    }
    pub(super) fn apply_mask(&self, bits: &mut [u64]) {
        if let Some(last) = bits.last_mut() {
            *last &= self.last_word_mask();
        }
    }
}

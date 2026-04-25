use super::Coord;
use crate::checked;
use smallvec::{SmallVec, smallvec};
mod ops;
const WORD_BITS: usize = 64;
const WORD_BITS_OFFSET: usize = 63;
#[derive(Clone, Debug, Default)]
pub struct Bitboard {
    black: SmallVec<[u64; 8]>,
    white: SmallVec<[u64; 8]>,
    size: usize,
    num_words: usize,
}
pub struct BitboardWorkspace {
    scratch_pad: [Vec<u64>; 5],
}
type ScratchPads<'workspace> = [&'workspace mut Vec<u64>; 5];
fn bit_mask(bit_index: usize, context: &str) -> u64 {
    checked::shl_u64(1_u64, bit_index, context)
}
fn words_for_bits(total_bits: usize) -> usize {
    if total_bits == 0 {
        return 0;
    }
    let adjusted = checked::add_usize(total_bits, WORD_BITS_OFFSET, "Bitboard::words_for_bits");
    checked::div_usize(adjusted, WORD_BITS, "Bitboard::words_for_bits")
}
pub(super) fn word_at(bits: &[u64], word_index: usize, context: &str) -> u64 {
    let Some(word) = bits.get(word_index) else {
        eprintln!("{context} 位棋盘字索引越界: {word_index}");
        panic!("{context} 位棋盘字索引越界");
    };
    *word
}
pub(super) fn word_mut<'bits>(
    bits: &'bits mut [u64],
    word_index: usize,
    context: &str,
) -> &'bits mut u64 {
    let Some(word) = bits.get_mut(word_index) else {
        eprintln!("{context} 位棋盘字索引越界: {word_index}");
        panic!("{context} 位棋盘字索引越界");
    };
    word
}
impl BitboardWorkspace {
    #[inline]
    #[must_use]
    pub fn new(num_words: usize) -> Self {
        let scratch_pad = core::array::from_fn(|_| vec![0; num_words]);
        Self { scratch_pad }
    }
    #[inline]
    pub(super) const fn pads_mut(&mut self) -> ScratchPads<'_> {
        self.scratch_pad.each_mut()
    }
}
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
    pub(super) fn set_in(&self, bits: &mut [u64], row_index: usize, column_index: usize) -> bool {
        let (word_index, mask) = self.coord_to_bit(row_index, column_index);
        let word = word_mut(bits, word_index, "Bitboard::set_in");
        let was_set = *word & mask != 0;
        *word |= mask;
        !was_set
    }
    #[inline]
    pub(super) fn clear_in(&self, bits: &mut [u64], row_index: usize, column_index: usize) -> bool {
        let (word_index, mask) = self.coord_to_bit(row_index, column_index);
        let word = word_mut(bits, word_index, "Bitboard::clear_in");
        let was_set = *word & mask != 0;
        *word &= !mask;
        was_set
    }
    #[inline]
    pub(super) fn set(&mut self, row_index: usize, column_index: usize, player: u8) {
        let (word_index, bit) = self.coord_to_bit(row_index, column_index);
        match player {
            1 => {
                *word_mut(&mut self.black, word_index, "Bitboard::set::black") |= bit;
            }
            2 => {
                *word_mut(&mut self.white, word_index, "Bitboard::set::white") |= bit;
            }
            _ => {
                eprintln!("Bitboard::set 收到非法玩家编号: {player}");
                panic!("Bitboard::set 收到非法玩家编号");
            }
        }
    }
    #[inline]
    pub(super) fn clear(&mut self, row_index: usize, column_index: usize) {
        let (word_index, bit) = self.coord_to_bit(row_index, column_index);
        *word_mut(&mut self.black, word_index, "Bitboard::clear::black") &= !bit;
        *word_mut(&mut self.white, word_index, "Bitboard::clear::white") &= !bit;
    }
    #[inline]
    pub(super) fn occupied_into(&self, target: &mut Vec<u64>) {
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
    pub(super) fn empty_into(&self, target: &mut Vec<u64>) {
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
    pub fn iter_bits<'bits>(&self, bit_words: &'bits [u64]) -> impl Iterator<Item = Coord> + 'bits {
        let board_size = self.size;
        bit_words
            .iter()
            .copied()
            .enumerate()
            .flat_map(move |(word_index, word)| {
                let base_bit =
                    checked::mul_usize(word_index, WORD_BITS, "Bitboard::iter_bits::base_bit");
                (0_usize..WORD_BITS).filter_map(move |bit_index| {
                    let mask = bit_mask(bit_index, "Bitboard::iter_bits::mask");
                    if word & mask == 0 {
                        return None;
                    }
                    let global_bit =
                        checked::add_usize(base_bit, bit_index, "Bitboard::iter_bits::global_bit");
                    let row_index =
                        checked::div_usize(global_bit, board_size, "Bitboard::iter_bits::row");
                    if row_index >= board_size {
                        return None;
                    }
                    let column_index =
                        checked::rem_usize(global_bit, board_size, "Bitboard::iter_bits::column");
                    Some((row_index, column_index))
                })
            })
    }
    #[inline]
    #[must_use]
    pub(super) fn from_board(board: &[u8], board_size: usize) -> Self {
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
}

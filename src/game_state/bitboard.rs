use smallvec::{SmallVec, smallvec};

use super::Coord;
mod iterator;
mod ops;
pub use iterator::BitIterator;
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
type ScratchPads<'a> = (
    &'a mut Vec<u64>,
    &'a mut Vec<u64>,
    &'a mut Vec<u64>,
    &'a mut Vec<u64>,
    &'a mut Vec<u64>,
);
impl BitboardWorkspace {
    #[must_use]
    pub fn new(num_words: usize) -> Self {
        let scratch_pad = std::array::from_fn(|_| vec![0; num_words]);
        Self { scratch_pad }
    }

    pub const fn pads_mut(&mut self) -> ScratchPads<'_> {
        let [pad0, pad1, pad2, pad3, pad4] = &mut self.scratch_pad;
        (pad0, pad1, pad2, pad3, pad4)
    }
}
impl Bitboard {
    #[must_use]
    pub fn new(board_size: usize) -> Self {
        let total_bits = board_size * board_size;
        let num_words = total_bits.div_ceil(64);
        Self {
            black: smallvec![0; num_words],
            white: smallvec![0; num_words],
            size: board_size,
            num_words,
        }
    }

    #[must_use]
    pub const fn num_words(&self) -> usize {
        self.num_words
    }

    #[inline]
    const fn coord_to_index(&self, r: usize, c: usize) -> (usize, usize) {
        let bit_pos = r * self.size + c;
        (bit_pos / 64, bit_pos % 64)
    }

    #[inline]
    #[must_use]
    pub const fn coord_to_bit(&self, r: usize, c: usize) -> (usize, u64) {
        let (word_idx, bit_idx) = self.coord_to_index(r, c);
        (word_idx, 1u64 << bit_idx)
    }

    #[inline]
    #[must_use]
    pub fn empty_mask(&self) -> SmallVec<[u64; 8]> {
        smallvec![0u64; self.num_words]
    }

    #[inline]
    pub fn set_in(&self, bits: &mut [u64], r: usize, c: usize) -> bool {
        let (word_idx, mask) = self.coord_to_bit(r, c);
        let was_set = bits[word_idx] & mask != 0;
        bits[word_idx] |= mask;
        !was_set
    }

    #[inline]
    pub fn clear_in(&self, bits: &mut [u64], r: usize, c: usize) -> bool {
        let (word_idx, mask) = self.coord_to_bit(r, c);
        let was_set = bits[word_idx] & mask != 0;
        bits[word_idx] &= !mask;
        was_set
    }

    #[inline]
    pub fn set(&mut self, r: usize, c: usize, player: u8) {
        let (word_idx, bit_idx) = self.coord_to_index(r, c);
        let bit = 1u64 << bit_idx;
        if player == 1 {
            self.black[word_idx] |= bit;
        } else if player == 2 {
            self.white[word_idx] |= bit;
        }
    }

    #[inline]
    pub fn clear(&mut self, r: usize, c: usize) {
        let (word_idx, bit_idx) = self.coord_to_index(r, c);
        let bit = 1u64 << bit_idx;
        self.black[word_idx] &= !bit;
        self.white[word_idx] &= !bit;
    }

    #[inline]
    pub fn occupied_into(&self, target: &mut Vec<u64>) {
        self.resize_target(target);
        for ((word, b), w) in target
            .iter_mut()
            .zip(self.black.iter())
            .zip(self.white.iter())
        {
            *word = b | w;
        }
    }

    #[inline]
    pub fn empty_into(&self, target: &mut Vec<u64>) {
        self.resize_target(target);
        for ((word, b), w) in target
            .iter_mut()
            .zip(self.black.iter())
            .zip(self.white.iter())
        {
            *word = !(b | w);
        }
        self.apply_mask(target);
    }

    #[must_use]
    pub fn is_all_zeros(bits: &[u64]) -> bool {
        bits.iter().all(|&w| w == 0)
    }

    #[must_use]
    pub const fn iter_bits<'a>(&self, bb: &'a [u64]) -> BitIterator<'a> {
        BitIterator {
            bits: bb,
            size: self.size,
            word_idx: 0,
            base_bit: 0,
            current_word: 0,
        }
    }

    #[must_use]
    pub fn from_board(board: &[u8], board_size: usize) -> Self {
        let mut bb = Self::new(board_size);
        for r in 0..board_size {
            for c in 0..board_size {
                let cell = board[r * board_size + c];
                if cell == 1 {
                    bb.set(r, c, 1);
                } else if cell == 2 {
                    bb.set(r, c, 2);
                }
            }
        }
        bb
    }
}

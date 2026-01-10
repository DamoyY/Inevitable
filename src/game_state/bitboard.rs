use super::Coord;
use smallvec::{smallvec, SmallVec};
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
    const fn last_word_mask(&self) -> u64 {
        let total_bits = self.size * self.size;
        let bits_in_last = total_bits % 64;
        if bits_in_last == 0 {
            u64::MAX
        } else {
            (1u64 << bits_in_last) - 1
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

    fn shift_into(&self, bits: &[u64], target: &mut Vec<u64>, n: usize, left: bool) {
        self.resize_target(target);
        if n == 0 {
            target.copy_from_slice(bits);
            return;
        }
        target.fill(0);
        let word_shift = n / 64;
        let bit_shift = n % 64;
        if left {
            for (i, word) in target.iter_mut().enumerate().skip(word_shift) {
                let src = i - word_shift;
                let mut value = bits[src] << bit_shift;
                if bit_shift > 0 && src > 0 {
                    value |= bits[src - 1] >> (64 - bit_shift);
                }
                *word = value;
            }
        } else {
            let count = self.num_words.saturating_sub(word_shift);
            for (i, word) in target.iter_mut().enumerate().take(count) {
                let src = i + word_shift;
                let mut value = bits[src] >> bit_shift;
                if bit_shift > 0 && src + 1 < self.num_words {
                    value |= bits[src + 1] << (64 - bit_shift);
                }
                *word = value;
            }
        }
    }

    fn shift_left_into(&self, bits: &[u64], target: &mut Vec<u64>, n: usize) {
        self.shift_into(bits, target, n, true);
    }

    fn shift_right_into(&self, bits: &[u64], target: &mut Vec<u64>, n: usize) {
        self.shift_into(bits, target, n, false);
    }

    fn copy_and_clear_col(&self, source: &[u64], target: &mut Vec<u64>, col: usize) {
        self.resize_target(target);
        target.copy_from_slice(source);
        for row in 0..self.size {
            let (word_idx, mask) = self.coord_to_bit(row, col);
            target[word_idx] &= !mask;
        }
    }

    fn or_inplace(target: &mut [u64], src: &[u64]) {
        for (word, add) in target.iter_mut().zip(src.iter()) {
            *word |= add;
        }
    }

    fn resize_target(&self, target: &mut Vec<u64>) {
        if target.len() != self.num_words {
            target.resize(self.num_words, 0);
        }
    }

    const fn apply_mask(&self, bits: &mut [u64]) {
        if let Some(last) = bits.last_mut() {
            *last &= self.last_word_mask();
        }
    }

    pub fn dilate_into(
        &self,
        bb: &[u64],
        target: &mut Vec<u64>,
        masked_not_left: &mut Vec<u64>,
        masked_not_right: &mut Vec<u64>,
        temp: &mut Vec<u64>,
    ) {
        let size = self.size;
        self.copy_and_clear_col(bb, masked_not_left, 0);
        self.copy_and_clear_col(bb, masked_not_right, size - 1);
        self.resize_target(target);
        target.copy_from_slice(bb);
        let sources: [&[u64]; 3] = [bb, masked_not_left, masked_not_right];
        let ops = [
            (1usize, false, 1usize),
            (2usize, true, 1usize),
            (0usize, false, size),
            (0usize, true, size),
            (1usize, false, size + 1),
            (2usize, false, size - 1),
            (1usize, true, size - 1),
            (2usize, true, size + 1),
        ];
        for (src_idx, shift_left, n) in ops {
            if shift_left {
                self.shift_left_into(sources[src_idx], temp, n);
            } else {
                self.shift_right_into(sources[src_idx], temp, n);
            }
            Self::or_inplace(target, temp);
        }
        self.apply_mask(target);
    }

    pub fn neighbors_into(
        &self,
        bb: &[u64],
        target: &mut Vec<u64>,
        masked_not_left: &mut Vec<u64>,
        masked_not_right: &mut Vec<u64>,
        temp: &mut Vec<u64>,
    ) {
        self.dilate_into(bb, target, masked_not_left, masked_not_right, temp);
        for (word, src) in target.iter_mut().zip(bb.iter()) {
            *word &= !src;
        }
        self.apply_mask(target);
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
pub struct BitIterator<'a> {
    bits: &'a [u64],
    size: usize,
    word_idx: usize,
    base_bit: usize,
    current_word: u64,
}
impl Iterator for BitIterator<'_> {
    type Item = Coord;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if self.current_word == 0 {
                if self.word_idx >= self.bits.len() {
                    return None;
                }
                self.current_word = self.bits[self.word_idx];
                if self.current_word == 0 {
                    self.word_idx += 1;
                    self.base_bit += 64;
                    continue;
                }
            }
            let bit_in_word = self.current_word.trailing_zeros() as usize;
            self.current_word &= self.current_word - 1;
            let global_bit = self.base_bit + bit_in_word;
            if self.current_word == 0 {
                self.word_idx += 1;
                self.base_bit += 64;
            }
            return Some((global_bit / self.size, global_bit % self.size));
        }
    }
}

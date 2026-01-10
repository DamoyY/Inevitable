use super::Coord;
#[derive(Clone, Debug, Default)]
pub struct Bitboard {
    black: Vec<u64>,
    white: Vec<u64>,
    size: usize,
    num_words: usize,
}
pub struct BitboardWorkspace {
    scratch_pad: Vec<Vec<u64>>,
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
        let mut scratch_pad = Vec::with_capacity(5);
        for _ in 0..5 {
            scratch_pad.push(vec![0; num_words]);
        }
        Self { scratch_pad }
    }

    pub fn pads_mut(&mut self) -> ScratchPads<'_> {
        let (pad0, rest) = self.scratch_pad.split_at_mut(1);
        let (pad1, rest) = rest.split_at_mut(1);
        let (pad2, rest) = rest.split_at_mut(1);
        let (pad3, rest) = rest.split_at_mut(1);
        let pad4 = &mut rest[0];
        (&mut pad0[0], &mut pad1[0], &mut pad2[0], &mut pad3[0], pad4)
    }
}
impl Bitboard {
    #[must_use]
    pub fn new(board_size: usize) -> Self {
        let total_bits = board_size * board_size;
        let num_words = total_bits.div_ceil(64);
        Self {
            black: vec![0; num_words],
            white: vec![0; num_words],
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
            let bit_pos = row * self.size + col;
            let word_idx = bit_pos / 64;
            let bit_idx = bit_pos % 64;
            target[word_idx] &= !(1u64 << bit_idx);
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
        self.shift_right_into(masked_not_left, temp, 1);
        Self::or_inplace(target, temp);
        self.shift_left_into(masked_not_right, temp, 1);
        Self::or_inplace(target, temp);
        self.shift_right_into(bb, temp, size);
        Self::or_inplace(target, temp);
        self.shift_left_into(bb, temp, size);
        Self::or_inplace(target, temp);
        self.shift_right_into(masked_not_left, temp, size + 1);
        Self::or_inplace(target, temp);
        self.shift_right_into(masked_not_right, temp, size - 1);
        Self::or_inplace(target, temp);
        self.shift_left_into(masked_not_left, temp, size - 1);
        Self::or_inplace(target, temp);
        self.shift_left_into(masked_not_right, temp, size + 1);
        Self::or_inplace(target, temp);
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
    pub fn iter_bits(&self, bb: &[u64]) -> BitIterator {
        BitIterator {
            bits: bb.to_vec(),
            size: self.size,
            word_idx: 0,
            base_bit: 0,
        }
    }

    #[must_use]
    pub fn from_board(board: &[Vec<u8>]) -> Self {
        let size = board.len();
        let mut bb = Self::new(size);
        for (r, row) in board.iter().enumerate() {
            for (c, &cell) in row.iter().enumerate() {
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
pub struct BitIterator {
    bits: Vec<u64>,
    size: usize,
    word_idx: usize,
    base_bit: usize,
}
impl Iterator for BitIterator {
    type Item = Coord;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        while self.word_idx < self.bits.len() {
            let word = self.bits[self.word_idx];
            if word != 0 {
                let bit_in_word = word.trailing_zeros() as usize;
                self.bits[self.word_idx] &= self.bits[self.word_idx] - 1;
                let global_bit = self.base_bit + bit_in_word;
                return Some((global_bit / self.size, global_bit % self.size));
            }
            self.word_idx += 1;
            self.base_bit += 64;
        }
        None
    }
}

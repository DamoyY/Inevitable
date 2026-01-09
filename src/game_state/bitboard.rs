use super::Coord;
#[derive(Clone, Debug, Default)]
pub struct Bitboard {
    black: Vec<u64>,
    white: Vec<u64>,
    size: usize,
    num_words: usize,
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
    #[must_use]
    pub fn occupied(&self) -> Vec<u64> {
        self.black
            .iter()
            .zip(self.white.iter())
            .map(|(b, w)| b | w)
            .collect()
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
    #[must_use]
    pub fn empty(&self) -> Vec<u64> {
        let occupied = self.occupied();
        let mut result: Vec<u64> = occupied.iter().map(|o| !o).collect();
        if let Some(last) = result.last_mut() {
            *last &= self.last_word_mask();
        }
        result
    }

    #[must_use]
    pub fn is_all_zeros(bits: &[u64]) -> bool {
        bits.iter().all(|&w| w == 0)
    }

    fn shift(&self, bits: &[u64], n: usize, left: bool) -> Vec<u64> {
        if n == 0 {
            return bits.to_vec();
        }
        let word_shift = n / 64;
        let bit_shift = n % 64;
        let mut result = vec![0u64; self.num_words];
        if left {
            for (i, result_word) in result.iter_mut().enumerate().skip(word_shift) {
                let src = i - word_shift;
                *result_word = bits[src] << bit_shift;
                if bit_shift > 0 && src > 0 {
                    *result_word |= bits[src - 1] >> (64 - bit_shift);
                }
            }
        } else {
            let count = self.num_words.saturating_sub(word_shift);
            for (i, result_word) in result.iter_mut().enumerate().take(count) {
                let src = i + word_shift;
                *result_word = bits[src] >> bit_shift;
                if bit_shift > 0 && src + 1 < self.num_words {
                    *result_word |= bits[src + 1] << (64 - bit_shift);
                }
            }
        }
        result
    }

    fn shift_left(&self, bits: &[u64], n: usize) -> Vec<u64> {
        self.shift(bits, n, true)
    }

    fn shift_right(&self, bits: &[u64], n: usize) -> Vec<u64> {
        self.shift(bits, n, false)
    }

    fn bitwise_or(a: &[u64], b: &[u64]) -> Vec<u64> {
        a.iter().zip(b.iter()).map(|(x, y)| x | y).collect()
    }

    fn bitwise_and_not(a: &[u64], b: &[u64]) -> Vec<u64> {
        a.iter().zip(b.iter()).map(|(x, y)| x & !y).collect()
    }

    const fn apply_mask(&self, bits: &mut [u64]) {
        if let Some(last) = bits.last_mut() {
            *last &= self.last_word_mask();
        }
    }

    #[must_use]
    pub fn dilate(&self, bb: &[u64]) -> Vec<u64> {
        let size = self.size;
        let left_mask = self.col_mask(0);
        let right_mask = self.col_mask(size - 1);
        let masked_not_left = Self::bitwise_and_not(bb, &left_mask);
        let masked_not_right = Self::bitwise_and_not(bb, &right_mask);
        let shifted_left = self.shift_right(&masked_not_left, 1);
        let shifted_right = self.shift_left(&masked_not_right, 1);
        let shifted_up = self.shift_right(bb, size);
        let shifted_down = self.shift_left(bb, size);
        let shifted_up_left = self.shift_right(&masked_not_left, size + 1);
        let shifted_up_right = self.shift_right(&masked_not_right, size - 1);
        let shifted_down_left = self.shift_left(&masked_not_left, size - 1);
        let shifted_down_right = self.shift_left(&masked_not_right, size + 1);
        let mut result = bb.to_vec();
        result = Self::bitwise_or(&result, &shifted_left);
        result = Self::bitwise_or(&result, &shifted_right);
        result = Self::bitwise_or(&result, &shifted_up);
        result = Self::bitwise_or(&result, &shifted_down);
        result = Self::bitwise_or(&result, &shifted_up_left);
        result = Self::bitwise_or(&result, &shifted_up_right);
        result = Self::bitwise_or(&result, &shifted_down_left);
        result = Self::bitwise_or(&result, &shifted_down_right);
        self.apply_mask(&mut result);
        result
    }

    #[must_use]
    pub fn neighbors(&self, bb: &[u64]) -> Vec<u64> {
        let dilated = self.dilate(bb);
        Self::bitwise_and_not(&dilated, bb)
    }

    fn col_mask(&self, col: usize) -> Vec<u64> {
        let mut result = vec![0u64; self.num_words];
        for row in 0..self.size {
            let bit_pos = row * self.size + col;
            let word_idx = bit_pos / 64;
            let bit_idx = bit_pos % 64;
            result[word_idx] |= 1u64 << bit_idx;
        }
        result
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

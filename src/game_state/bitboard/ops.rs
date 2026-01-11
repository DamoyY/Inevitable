use super::Bitboard;
impl Bitboard {
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

    pub(super) fn resize_target(&self, target: &mut Vec<u64>) {
        if target.len() != self.num_words {
            target.resize(self.num_words, 0);
        }
    }

    pub(super) const fn apply_mask(&self, bits: &mut [u64]) {
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
}

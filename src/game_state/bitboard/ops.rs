use super::{Bitboard, WORD_BITS, word_at, word_mut};
use crate::checked;
impl Bitboard {
    #[inline]
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
    fn validate_bits_len(&self, bits: &[u64], context: &str) {
        if bits.len() != self.num_words {
            eprintln!(
                "{context} 位棋盘字数不匹配: 实际 {}, 期望 {}",
                bits.len(),
                self.num_words
            );
            panic!("{context} 位棋盘字数不匹配");
        }
    }
    fn shift_into(&self, bits: &[u64], target: &mut Vec<u64>, shift_distance: usize, left: bool) {
        self.validate_bits_len(bits, "Bitboard::shift_into");
        self.resize_target(target);
        if shift_distance == 0 {
            target.copy_from_slice(bits);
            return;
        }
        target.fill(0);
        let word_shift = checked::div_usize(
            shift_distance,
            WORD_BITS,
            "Bitboard::shift_into::word_shift",
        );
        let bit_shift =
            checked::rem_usize(shift_distance, WORD_BITS, "Bitboard::shift_into::bit_shift");
        if left {
            for (target_index, target_word) in target.iter_mut().enumerate().skip(word_shift) {
                let source_index = checked::sub_usize(
                    target_index,
                    word_shift,
                    "Bitboard::shift_into::left_source",
                );
                let mut value = checked::shl_u64(
                    word_at(bits, source_index, "Bitboard::shift_into::left_word"),
                    bit_shift,
                    "Bitboard::shift_into::left_shift",
                );
                if bit_shift > 0 && source_index > 0 {
                    let previous_source_index = checked::sub_usize(
                        source_index,
                        1_usize,
                        "Bitboard::shift_into::previous_source",
                    );
                    let reverse_shift = checked::sub_usize(
                        WORD_BITS,
                        bit_shift,
                        "Bitboard::shift_into::left_reverse_shift",
                    );
                    value |= checked::shr_u64(
                        word_at(
                            bits,
                            previous_source_index,
                            "Bitboard::shift_into::previous_left_word",
                        ),
                        reverse_shift,
                        "Bitboard::shift_into::left_carry",
                    );
                }
                *target_word = value;
            }
        } else {
            let count = if word_shift >= self.num_words {
                0
            } else {
                checked::sub_usize(
                    self.num_words,
                    word_shift,
                    "Bitboard::shift_into::right_count",
                )
            };
            for (target_index, target_word) in target.iter_mut().enumerate().take(count) {
                let source_index = checked::add_usize(
                    target_index,
                    word_shift,
                    "Bitboard::shift_into::right_source",
                );
                let mut value = checked::shr_u64(
                    word_at(bits, source_index, "Bitboard::shift_into::right_word"),
                    bit_shift,
                    "Bitboard::shift_into::right_shift",
                );
                if bit_shift > 0 {
                    let next_source_index = checked::add_usize(
                        source_index,
                        1_usize,
                        "Bitboard::shift_into::next_source",
                    );
                    if next_source_index < self.num_words {
                        let reverse_shift = checked::sub_usize(
                            WORD_BITS,
                            bit_shift,
                            "Bitboard::shift_into::right_reverse_shift",
                        );
                        value |= checked::shl_u64(
                            word_at(
                                bits,
                                next_source_index,
                                "Bitboard::shift_into::next_right_word",
                            ),
                            reverse_shift,
                            "Bitboard::shift_into::right_carry",
                        );
                    }
                }
                *target_word = value;
            }
        }
    }
    fn shift_left_into(&self, bits: &[u64], target: &mut Vec<u64>, shift_distance: usize) {
        self.shift_into(bits, target, shift_distance, true);
    }
    fn shift_right_into(&self, bits: &[u64], target: &mut Vec<u64>, shift_distance: usize) {
        self.shift_into(bits, target, shift_distance, false);
    }
    fn copy_and_clear_col(&self, source: &[u64], target: &mut Vec<u64>, column_index: usize) {
        self.validate_bits_len(source, "Bitboard::copy_and_clear_col");
        self.resize_target(target);
        target.copy_from_slice(source);
        for row_index in 0..self.size {
            let (word_index, mask) = self.coord_to_bit(row_index, column_index);
            *word_mut(target, word_index, "Bitboard::copy_and_clear_col") &= !mask;
        }
    }
    fn or_inplace(target: &mut [u64], source: &[u64]) {
        for (target_word, source_word) in target.iter_mut().zip(source.iter()) {
            *target_word |= *source_word;
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
    #[inline]
    pub(super) fn dilate_into(
        &self,
        bb: &[u64],
        target: &mut Vec<u64>,
        masked_not_left: &mut Vec<u64>,
        masked_not_right: &mut Vec<u64>,
        temp: &mut Vec<u64>,
    ) {
        let size = self.size;
        let last_column = checked::sub_usize(size, 1_usize, "Bitboard::dilate_into::last_column");
        let size_minus_one = last_column;
        let size_plus_one =
            checked::add_usize(size, 1_usize, "Bitboard::dilate_into::size_plus_one");
        self.copy_and_clear_col(bb, masked_not_left, 0);
        self.copy_and_clear_col(bb, masked_not_right, last_column);
        self.resize_target(target);
        target.copy_from_slice(bb);
        let sources: [&[u64]; 3] = [bb, masked_not_left, masked_not_right];
        let ops = [
            (1_usize, false, 1_usize),
            (2_usize, true, 1_usize),
            (0_usize, false, size),
            (0_usize, true, size),
            (1_usize, false, size_plus_one),
            (2_usize, false, size_minus_one),
            (1_usize, true, size_minus_one),
            (2_usize, true, size_plus_one),
        ];
        for (source_index, shift_left, shift_distance) in ops {
            let Some(source_bits) = sources.get(source_index) else {
                eprintln!("Bitboard::dilate_into 来源索引越界: {source_index}");
                panic!("Bitboard::dilate_into 来源索引越界");
            };
            if shift_left {
                self.shift_left_into(source_bits, temp, shift_distance);
            } else {
                self.shift_right_into(source_bits, temp, shift_distance);
            }
            Self::or_inplace(target, temp);
        }
        self.apply_mask(target);
    }
    #[inline]
    pub(in crate::game_state) fn neighbors_into(
        &self,
        bb: &[u64],
        target: &mut Vec<u64>,
        masked_not_left: &mut Vec<u64>,
        masked_not_right: &mut Vec<u64>,
        temp: &mut Vec<u64>,
    ) {
        self.dilate_into(bb, target, masked_not_left, masked_not_right, temp);
        for (target_word, source_word) in target.iter_mut().zip(bb.iter()) {
            *target_word &= !*source_word;
        }
        self.apply_mask(target);
    }
}

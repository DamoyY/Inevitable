use super::{Bitboard, WORD_BITS, word_at, word_mut};
use crate::checked;
impl Bitboard {
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
            return;
        }
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
                let next_source_index =
                    checked::add_usize(source_index, 1_usize, "Bitboard::shift_into::next_source");
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
    pub(super) fn shift_left_into(
        &self,
        bits: &[u64],
        target: &mut Vec<u64>,
        shift_distance: usize,
    ) {
        self.shift_into(bits, target, shift_distance, true);
    }
    pub(super) fn shift_right_into(
        &self,
        bits: &[u64],
        target: &mut Vec<u64>,
        shift_distance: usize,
    ) {
        self.shift_into(bits, target, shift_distance, false);
    }
    pub(super) fn copy_and_clear_col(
        &self,
        source: &[u64],
        target: &mut Vec<u64>,
        column_index: usize,
    ) {
        self.validate_bits_len(source, "Bitboard::copy_and_clear_col");
        self.resize_target(target);
        target.copy_from_slice(source);
        for row_index in 0..self.size {
            let (word_index, mask) = self.coord_to_bit(row_index, column_index);
            *word_mut(target, word_index, "Bitboard::copy_and_clear_col") &= !mask;
        }
    }
    pub(super) fn or_inplace(target: &mut [u64], source: &[u64]) {
        for (target_word, source_word) in target.iter_mut().zip(source.iter()) {
            *target_word |= *source_word;
        }
    }
}

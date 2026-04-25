use super::{Bitboard, WORD_BITS};
use crate::{checked, game_state::Coord};
impl Bitboard {
    #[inline]
    pub fn iter_bits<'bits>(&self, bit_words: &'bits [u64]) -> impl Iterator<Item = Coord> + 'bits {
        let board_size = self.size;
        bit_words
            .iter()
            .copied()
            .enumerate()
            .flat_map(move |(word_index, mut word)| {
                let base_bit =
                    checked::mul_usize(word_index, WORD_BITS, "Bitboard::iter_bits::base_bit");
                core::iter::from_fn(move || {
                    if word == 0 {
                        return None;
                    }
                    let bit_index = match usize::try_from(word.trailing_zeros()) {
                        Ok(converted) => converted,
                        Err(err) => {
                            eprintln!("Bitboard::iter_bits 位索引转换失败: {err}");
                            panic!("Bitboard::iter_bits 位索引转换失败");
                        }
                    };
                    word &= checked::sub_u64(word, 1_u64, "Bitboard::iter_bits::clear_low_bit");
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
}

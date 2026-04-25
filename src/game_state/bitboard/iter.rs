use super::{Bitboard, WORD_BITS, bit_mask};
use crate::{checked, game_state::Coord};
impl Bitboard {
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
}

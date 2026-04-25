use crate::checked;
use smallvec::SmallVec;
mod core;
mod iter;
mod neighborhood;
mod shift;
mod workspace;
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

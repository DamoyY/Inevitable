use super::Coord;
pub struct BitIterator<'a> {
    pub(super) bits: &'a [u64],
    pub(super) size: usize,
    pub(super) word_idx: usize,
    pub(super) base_bit: usize,
    pub(super) current_word: u64,
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

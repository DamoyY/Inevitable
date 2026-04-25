use super::Bitboard;
use crate::checked;
impl Bitboard {
    #[inline]
    fn dilate_into(
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

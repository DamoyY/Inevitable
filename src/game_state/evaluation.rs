use super::{Coord, GomokuEvaluator, GomokuPosition};
use crate::{checked, config::EvaluationWeights};
fn board_area(board_size: usize, context: &str) -> usize {
    checked::mul_usize(board_size, board_size, context)
}
fn score_index(board_size: usize, row_index: usize, column_index: usize, context: &str) -> usize {
    let row_offset = checked::mul_usize(row_index, board_size, context);
    checked::add_usize(row_offset, column_index, context)
}
fn score_slot(score_buffer: &[f32], board_index: usize, context: &str) -> f32 {
    let Some(&score) = score_buffer.get(board_index) else {
        eprintln!("{context} 评分缓冲区索引越界: {board_index}");
        panic!("{context} 评分缓冲区索引越界");
    };
    score
}
fn score_slot_mut<'buffer>(
    score_buffer: &'buffer mut [f32],
    board_index: usize,
    context: &str,
) -> &'buffer mut f32 {
    let Some(score) = score_buffer.get_mut(board_index) else {
        eprintln!("{context} 评分缓冲区索引越界: {board_index}");
        panic!("{context} 评分缓冲区索引越界");
    };
    score
}
impl GomokuEvaluator {
    #[inline]
    #[must_use]
    pub fn new(board_size: usize, config: EvaluationWeights) -> Self {
        let proximity_kernel = Self::init_proximity_kernel(config);
        let positional_bonus = Self::init_positional_bonus(board_size, config);
        Self {
            config,
            proximity_kernel,
            positional_bonus,
        }
    }
    fn init_proximity_kernel(config: EvaluationWeights) -> Vec<Vec<f32>> {
        let kernel_size = config.proximity_kernel_size;
        if kernel_size == 0 {
            eprintln!("GomokuEvaluator::init_proximity_kernel 核大小不能为 0");
            panic!("GomokuEvaluator::init_proximity_kernel 核大小不能为 0");
        }
        let kernel_center =
            checked::div_usize(kernel_size, 2_usize, "GomokuEvaluator::kernel_center");
        let mut proximity_kernel = vec![vec![0.0_f32; kernel_size]; kernel_size];
        for (row_index, kernel_row) in proximity_kernel.iter_mut().enumerate() {
            for (column_index, kernel_value) in kernel_row.iter_mut().enumerate() {
                let distance = checked::add_usize(
                    row_index.abs_diff(kernel_center),
                    column_index.abs_diff(kernel_center),
                    "GomokuEvaluator::init_proximity_kernel::distance",
                );
                let distance_u16 = checked::usize_to_u16(
                    distance,
                    "GomokuEvaluator::init_proximity_kernel::distance_u16",
                );
                *kernel_value = 1.0_f32 / (f32::from(distance_u16) + 1.0_f32);
            }
        }
        proximity_kernel
    }
    fn init_positional_bonus(board_size: usize, config: EvaluationWeights) -> Vec<f32> {
        let center = checked::div_usize(
            board_size,
            2_usize,
            "GomokuEvaluator::init_positional_bonus",
        );
        let mut positional_bonus =
            vec![0.0_f32; board_area(board_size, "GomokuEvaluator::init_positional_bonus")];
        for row_index in 0..board_size {
            for column_index in 0..board_size {
                let row_bonus = checked::sub_usize(
                    center,
                    center.abs_diff(row_index),
                    "GomokuEvaluator::init_positional_bonus::row_bonus",
                );
                let column_bonus = checked::sub_usize(
                    center,
                    center.abs_diff(column_index),
                    "GomokuEvaluator::init_positional_bonus::column_bonus",
                );
                let bonus = checked::add_usize(
                    row_bonus,
                    column_bonus,
                    "GomokuEvaluator::init_positional_bonus::bonus",
                );
                let bonus_u16 = checked::usize_to_u16(
                    bonus,
                    "GomokuEvaluator::init_positional_bonus::bonus_u16",
                );
                let slot_index = score_index(
                    board_size,
                    row_index,
                    column_index,
                    "GomokuEvaluator::init_positional_bonus::slot_index",
                );
                *score_slot_mut(
                    &mut positional_bonus,
                    slot_index,
                    "GomokuEvaluator::init_positional_bonus",
                ) = f32::from(bonus_u16) * config.positional_bonus_scale;
            }
        }
        positional_bonus
    }
    fn add_proximity_scores(
        &self,
        position: &GomokuPosition,
        player: u8,
        score_buffer: &mut [f32],
    ) {
        let board_size = position.board_size;
        let scale = self.config.proximity_scale;
        for row_index in 0..board_size {
            for column_index in 0..board_size {
                if position.cell(row_index, column_index) == player {
                    self.apply_proximity_kernel_scaled(
                        board_size,
                        (row_index, column_index),
                        scale,
                        score_buffer,
                    );
                }
            }
        }
    }
    pub(crate) fn rebuild_proximity_scores(
        &self,
        position: &GomokuPosition,
        player: u8,
        target: &mut [f32],
    ) {
        let required_len = board_area(
            position.board_size,
            "GomokuEvaluator::rebuild_proximity_scores::required_len",
        );
        if target.len() != required_len {
            eprintln!(
                "GomokuEvaluator::rebuild_proximity_scores 缓冲区长度不匹配: 实际 {}, 期望 {}",
                target.len(),
                required_len
            );
            panic!("GomokuEvaluator::rebuild_proximity_scores 缓冲区长度不匹配");
        }
        target.fill(0.0_f32);
        self.add_proximity_scores(position, player, target);
    }
    pub(crate) fn apply_proximity_delta(
        &self,
        position: &GomokuPosition,
        mov: Coord,
        delta: f32,
        target: &mut [f32],
    ) {
        let required_len = board_area(
            position.board_size,
            "GomokuEvaluator::apply_proximity_delta::required_len",
        );
        if target.len() != required_len {
            eprintln!(
                "GomokuEvaluator::apply_proximity_delta 缓冲区长度不匹配: 实际 {}, 期望 {}",
                target.len(),
                required_len
            );
            panic!("GomokuEvaluator::apply_proximity_delta 缓冲区长度不匹配");
        }
        let scale = self.config.proximity_scale * delta;
        self.apply_proximity_kernel_scaled(position.board_size, mov, scale, target);
    }
    fn apply_proximity_kernel_scaled(
        &self,
        board_size: usize,
        mov: Coord,
        scale: f32,
        target: &mut [f32],
    ) {
        let required_len = board_area(board_size, "GomokuEvaluator::apply_proximity_kernel_scaled");
        if target.len() != required_len {
            eprintln!(
                "GomokuEvaluator::apply_proximity_kernel_scaled 缓冲区长度不匹配: 实际 {}, 期望 {}",
                target.len(),
                required_len
            );
            panic!("GomokuEvaluator::apply_proximity_kernel_scaled 缓冲区长度不匹配");
        }
        let (row_index, column_index) = mov;
        let kernel_height = self.proximity_kernel.len();
        let Some(first_kernel_row) = self.proximity_kernel.first() else {
            eprintln!("GomokuEvaluator::apply_proximity_kernel_scaled 核数据为空");
            panic!("GomokuEvaluator::apply_proximity_kernel_scaled 核数据为空");
        };
        let kernel_width = first_kernel_row.len();
        let row_padding = checked::div_usize(
            kernel_height,
            2_usize,
            "GomokuEvaluator::apply_proximity_kernel_scaled::row_padding",
        );
        let column_padding = checked::div_usize(
            kernel_width,
            2_usize,
            "GomokuEvaluator::apply_proximity_kernel_scaled::column_padding",
        );
        let base_row = checked::add_isize(
            checked::usize_to_isize(
                row_index,
                "GomokuEvaluator::apply_proximity_kernel_scaled::row_index",
            ),
            checked::usize_to_isize(
                row_padding,
                "GomokuEvaluator::apply_proximity_kernel_scaled::row_padding_isize",
            ),
            "GomokuEvaluator::apply_proximity_kernel_scaled::base_row",
        );
        let base_column = checked::add_isize(
            checked::usize_to_isize(
                column_index,
                "GomokuEvaluator::apply_proximity_kernel_scaled::column_index",
            ),
            checked::usize_to_isize(
                column_padding,
                "GomokuEvaluator::apply_proximity_kernel_scaled::column_padding_isize",
            ),
            "GomokuEvaluator::apply_proximity_kernel_scaled::base_column",
        );
        let board_limit = checked::usize_to_isize(
            board_size,
            "GomokuEvaluator::apply_proximity_kernel_scaled::board_limit",
        );
        for (kernel_row_index, kernel_row) in self.proximity_kernel.iter().enumerate() {
            let row_offset = checked::usize_to_isize(
                kernel_row_index,
                "GomokuEvaluator::apply_proximity_kernel_scaled::row_offset",
            );
            let target_row = checked::sub_isize(
                base_row,
                row_offset,
                "GomokuEvaluator::apply_proximity_kernel_scaled::target_row",
            );
            if target_row < 0 || target_row >= board_limit {
                continue;
            }
            let target_row_index = checked::isize_to_usize(
                target_row,
                "GomokuEvaluator::apply_proximity_kernel_scaled::target_row_index",
            );
            let row_start = checked::mul_usize(
                target_row_index,
                board_size,
                "GomokuEvaluator::apply_proximity_kernel_scaled::row_start",
            );
            for (kernel_column_index, &kernel_value) in kernel_row.iter().enumerate() {
                let column_offset = checked::usize_to_isize(
                    kernel_column_index,
                    "GomokuEvaluator::apply_proximity_kernel_scaled::column_offset",
                );
                let target_column = checked::sub_isize(
                    base_column,
                    column_offset,
                    "GomokuEvaluator::apply_proximity_kernel_scaled::target_column",
                );
                if target_column < 0 || target_column >= board_limit {
                    continue;
                }
                let target_column_index = checked::isize_to_usize(
                    target_column,
                    "GomokuEvaluator::apply_proximity_kernel_scaled::target_column_index",
                );
                let target_index = checked::add_usize(
                    row_start,
                    target_column_index,
                    "GomokuEvaluator::apply_proximity_kernel_scaled::target_index",
                );
                let score = score_slot_mut(
                    target,
                    target_index,
                    "GomokuEvaluator::apply_proximity_kernel_scaled",
                );
                *score = kernel_value.mul_add(scale, *score);
            }
        }
    }
    fn patterns_to_score(
        position: &GomokuPosition,
        evaluation: EvaluationWeights,
    ) -> [(usize, usize, f32); 9] {
        let win_minus_one = checked::sub_usize(
            position.win_len,
            1_usize,
            "GomokuEvaluator::patterns_to_score::win_minus_one",
        );
        let win_minus_two = checked::sub_usize(
            position.win_len,
            2_usize,
            "GomokuEvaluator::patterns_to_score::win_minus_two",
        );
        let win_minus_three = checked::sub_usize(
            position.win_len,
            3_usize,
            "GomokuEvaluator::patterns_to_score::win_minus_three",
        );
        let win_minus_four = checked::sub_usize(
            position.win_len,
            4_usize,
            "GomokuEvaluator::patterns_to_score::win_minus_four",
        );
        [
            (win_minus_one, 0, evaluation.score_win),
            (win_minus_two, 0, evaluation.score_live_four),
            (win_minus_three, 0, evaluation.score_live_three),
            (win_minus_four, 0, evaluation.score_live_two),
            (win_minus_two, 1, evaluation.score_blocked_four),
            (0, win_minus_one, evaluation.score_block_win),
            (0, win_minus_two, evaluation.score_block_live_four),
            (0, win_minus_three, evaluation.score_block_live_three),
            (1, win_minus_two, evaluation.score_block_blocked_four),
        ]
    }
    pub(crate) fn score_moves_into(
        &self,
        position: &GomokuPosition,
        player: u8,
        moves_to_score: &[Coord],
        score_buffer: &mut Vec<f32>,
        scored_moves: &mut Vec<(Coord, f32)>,
    ) {
        self.score_moves_into_with_proximity(
            position,
            player,
            moves_to_score,
            &[],
            score_buffer,
            scored_moves,
        );
    }
    pub(crate) fn score_moves_into_with_proximity(
        &self,
        position: &GomokuPosition,
        player: u8,
        moves_to_score: &[Coord],
        proximity_scores: &[f32],
        score_buffer: &mut Vec<f32>,
        scored_moves: &mut Vec<(Coord, f32)>,
    ) {
        let evaluation = self.config;
        scored_moves.clear();
        if moves_to_score.is_empty() {
            return;
        }
        let board_size = position.board_size;
        let required_len = board_area(
            board_size,
            "GomokuEvaluator::score_moves_into_with_proximity::required_len",
        );
        if score_buffer.len() != required_len {
            score_buffer.resize(required_len, 0.0_f32);
        }
        score_buffer.copy_from_slice(&self.positional_bonus);
        if proximity_scores.len() == required_len {
            for (score, proximity_score) in score_buffer.iter_mut().zip(proximity_scores.iter()) {
                *score += *proximity_score;
            }
        } else {
            self.add_proximity_scores(position, player, score_buffer);
        }
        for (player_count, opponent_count, pattern_score) in
            Self::patterns_to_score(position, evaluation)
        {
            let windows =
                position
                    .threat_index
                    .get_pattern_windows(player, player_count, opponent_count);
            for window_index in windows {
                let window = position.threat_index.window(window_index);
                for &(row_index, column_index) in &window.coords {
                    if position.cell(row_index, column_index) == 0 {
                        let score_index = position.board_index(row_index, column_index);
                        *score_slot_mut(
                            score_buffer,
                            score_index,
                            "GomokuEvaluator::score_moves_into_with_proximity::pattern_score",
                        ) += pattern_score;
                    }
                }
            }
        }
        for &(row_index, column_index) in moves_to_score {
            let score_index = position.board_index(row_index, column_index);
            scored_moves.push((
                (row_index, column_index),
                score_slot(
                    score_buffer,
                    score_index,
                    "GomokuEvaluator::score_moves_into_with_proximity::scored_moves",
                ),
            ));
        }
    }
}

use super::{Coord, GomokuEvaluator, GomokuPosition};
use crate::{checked, config::EvaluationWeights};
fn board_area(board_size: usize, context: &str) -> usize {
    checked::mul_usize(board_size, board_size, context)
}
fn score_index(board_size: usize, row_index: usize, column_index: usize, context: &str) -> usize {
    let row_offset = checked::mul_usize(row_index, board_size, context);
    checked::add_usize(row_offset, column_index, context)
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
    fn init_proximity_kernel(config: EvaluationWeights) -> Vec<(usize, usize, f32)> {
        let kernel_size = config.proximity_kernel_size;
        if kernel_size == 0 {
            eprintln!("GomokuEvaluator::init_proximity_kernel 核大小不能为 0");
            panic!("GomokuEvaluator::init_proximity_kernel 核大小不能为 0");
        }
        let kernel_center = checked::div_usize(
            kernel_size,
            2_usize,
            "GomokuEvaluator::init_proximity_kernel",
        );
        let mut proximity_kernel = Vec::with_capacity(checked::mul_usize(
            kernel_size,
            kernel_size,
            "GomokuEvaluator::init_proximity_kernel::capacity",
        ));
        for row_index in 0..kernel_size {
            let row_distance = row_index.abs_diff(kernel_center);
            for column_index in 0..kernel_size {
                let distance = checked::add_usize(
                    row_distance,
                    column_index.abs_diff(kernel_center),
                    "GomokuEvaluator::init_proximity_kernel::distance",
                );
                let distance_u16 = checked::usize_to_u16(
                    distance,
                    "GomokuEvaluator::init_proximity_kernel::distance_u16",
                );
                proximity_kernel.push((
                    row_index,
                    column_index,
                    1.0_f32 / (f32::from(distance_u16) + 1.0_f32),
                ));
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
        let kernel_center = checked::div_usize(
            self.config.proximity_kernel_size,
            2_usize,
            "GomokuEvaluator::apply_proximity_kernel_scaled::kernel_center",
        );
        for &(kernel_row_index, kernel_column_index, kernel_value) in &self.proximity_kernel {
            let target_row = if kernel_row_index <= kernel_center {
                let offset = checked::sub_usize(
                    kernel_center,
                    kernel_row_index,
                    "GomokuEvaluator::apply_proximity_kernel_scaled::target_row_forward_offset",
                );
                let target_row = checked::add_usize(
                    row_index,
                    offset,
                    "GomokuEvaluator::apply_proximity_kernel_scaled::target_row_forward",
                );
                if target_row >= board_size {
                    continue;
                }
                target_row
            } else {
                let offset = checked::sub_usize(
                    kernel_row_index,
                    kernel_center,
                    "GomokuEvaluator::apply_proximity_kernel_scaled::target_row_backward_offset",
                );
                if offset > row_index {
                    continue;
                }
                checked::sub_usize(
                    row_index,
                    offset,
                    "GomokuEvaluator::apply_proximity_kernel_scaled::target_row_backward",
                )
            };
            let target_column = if kernel_column_index <= kernel_center {
                let offset = checked::sub_usize(
                    kernel_center,
                    kernel_column_index,
                    "GomokuEvaluator::apply_proximity_kernel_scaled::target_column_forward_offset",
                );
                let target_column = checked::add_usize(
                    column_index,
                    offset,
                    "GomokuEvaluator::apply_proximity_kernel_scaled::target_column_forward",
                );
                if target_column >= board_size {
                    continue;
                }
                target_column
            } else {
                let offset = checked::sub_usize(
                    kernel_column_index,
                    kernel_center,
                    "GomokuEvaluator::apply_proximity_kernel_scaled::target_column_backward_offset",
                );
                if offset > column_index {
                    continue;
                }
                checked::sub_usize(
                    column_index,
                    offset,
                    "GomokuEvaluator::apply_proximity_kernel_scaled::target_column_backward",
                )
            };
            let target_index = score_index(
                board_size,
                target_row,
                target_column,
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
    fn positional_score(&self, board_index: usize) -> f32 {
        let Some(&score) = self.positional_bonus.get(board_index) else {
            eprintln!("GomokuEvaluator::positional_score 位置评分索引越界: {board_index}");
            panic!("GomokuEvaluator::positional_score 位置评分索引越界");
        };
        score
    }
    fn proximity_score_for_point(
        &self,
        position: &GomokuPosition,
        player: u8,
        board_index: usize,
        proximity_scores: &[f32],
    ) -> f32 {
        let required_len = board_area(
            position.board_size,
            "GomokuEvaluator::proximity_score_for_point::required_len",
        );
        if proximity_scores.len() == required_len {
            let Some(&score) = proximity_scores.get(board_index) else {
                eprintln!(
                    "GomokuEvaluator::proximity_score_for_point 邻近度评分索引越界: {board_index}"
                );
                panic!("GomokuEvaluator::proximity_score_for_point 邻近度评分索引越界");
            };
            return score;
        }
        if !proximity_scores.is_empty() {
            eprintln!(
                "GomokuEvaluator::proximity_score_for_point 邻近度评分长度不匹配: 实际 {}, 期望 {}",
                proximity_scores.len(),
                required_len
            );
            panic!("GomokuEvaluator::proximity_score_for_point 邻近度评分长度不匹配");
        }
        let mut score = 0.0_f32;
        let row_index = checked::div_usize(
            board_index,
            position.board_size,
            "GomokuEvaluator::proximity_score_for_point::row_index",
        );
        let column_index = checked::rem_usize(
            board_index,
            position.board_size,
            "GomokuEvaluator::proximity_score_for_point::column_index",
        );
        let kernel_center = checked::div_usize(
            self.config.proximity_kernel_size,
            2_usize,
            "GomokuEvaluator::proximity_score_for_point::kernel_center",
        );
        for &(kernel_row_index, kernel_column_index, kernel_value) in &self.proximity_kernel {
            let source_row = if kernel_row_index <= kernel_center {
                let offset = checked::sub_usize(
                    kernel_center,
                    kernel_row_index,
                    "GomokuEvaluator::proximity_score_for_point::source_row_backward_offset",
                );
                if offset > row_index {
                    continue;
                }
                checked::sub_usize(
                    row_index,
                    offset,
                    "GomokuEvaluator::proximity_score_for_point::source_row_backward",
                )
            } else {
                let offset = checked::sub_usize(
                    kernel_row_index,
                    kernel_center,
                    "GomokuEvaluator::proximity_score_for_point::source_row_forward_offset",
                );
                let source_row = checked::add_usize(
                    row_index,
                    offset,
                    "GomokuEvaluator::proximity_score_for_point::source_row_forward",
                );
                if source_row >= position.board_size {
                    continue;
                }
                source_row
            };
            let source_column = if kernel_column_index <= kernel_center {
                let offset = checked::sub_usize(
                    kernel_center,
                    kernel_column_index,
                    "GomokuEvaluator::proximity_score_for_point::source_column_backward_offset",
                );
                if offset > column_index {
                    continue;
                }
                checked::sub_usize(
                    column_index,
                    offset,
                    "GomokuEvaluator::proximity_score_for_point::source_column_backward",
                )
            } else {
                let offset = checked::sub_usize(
                    kernel_column_index,
                    kernel_center,
                    "GomokuEvaluator::proximity_score_for_point::source_column_forward_offset",
                );
                let source_column = checked::add_usize(
                    column_index,
                    offset,
                    "GomokuEvaluator::proximity_score_for_point::source_column_forward",
                );
                if source_column >= position.board_size {
                    continue;
                }
                source_column
            };
            if position.cell(source_row, source_column) == player {
                score = kernel_value.mul_add(self.config.proximity_scale, score);
            }
        }
        score
    }
    fn pattern_score_for_point(
        position: &GomokuPosition,
        player: u8,
        row_index: usize,
        column_index: usize,
        patterns: &[(usize, usize, f32); 9],
    ) -> f32 {
        let mut score = 0.0_f32;
        for &window_index_u16 in position
            .threat_index
            .window_indices_for_point(row_index, column_index)
        {
            let window = position.threat_index.window(usize::from(window_index_u16));
            let (player_count, opponent_count) = match player {
                1 => (window.p1_count, window.p2_count),
                2 => (window.p2_count, window.p1_count),
                _ => {
                    eprintln!(
                        "GomokuEvaluator::pattern_score_for_point 收到非法玩家编号: {player}"
                    );
                    panic!("GomokuEvaluator::pattern_score_for_point 收到非法玩家编号");
                }
            };
            for &(pattern_player_count, pattern_opponent_count, pattern_score) in patterns {
                if player_count == pattern_player_count && opponent_count == pattern_opponent_count
                {
                    score += pattern_score;
                }
            }
        }
        score
    }
    pub(crate) fn score_moves_into(
        &self,
        position: &GomokuPosition,
        player: u8,
        moves_to_score: &[Coord],
        scored_moves: &mut Vec<(Coord, f32)>,
    ) {
        self.score_moves_into_with_proximity(position, player, moves_to_score, &[], scored_moves);
    }
    pub(crate) fn score_moves_into_with_proximity(
        &self,
        position: &GomokuPosition,
        player: u8,
        moves_to_score: &[Coord],
        proximity_scores: &[f32],
        scored_moves: &mut Vec<(Coord, f32)>,
    ) {
        let evaluation = self.config;
        scored_moves.clear();
        if moves_to_score.is_empty() {
            return;
        }
        let patterns = Self::patterns_to_score(position, evaluation);
        for &(row_index, column_index) in moves_to_score {
            let board_index = position.board_index(row_index, column_index);
            let score = self.positional_score(board_index)
                + self.proximity_score_for_point(position, player, board_index, proximity_scores)
                + Self::pattern_score_for_point(
                    position,
                    player,
                    row_index,
                    column_index,
                    &patterns,
                );
            scored_moves.push(((row_index, column_index), score));
        }
    }
}

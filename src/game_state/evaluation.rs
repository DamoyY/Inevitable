use super::{Coord, GomokuEvaluator, GomokuPosition};
use crate::config::EvaluationConfig;

impl GomokuEvaluator {
    #[must_use]
    pub fn new(board_size: usize, config: EvaluationConfig) -> Self {
        let proximity_kernel = Self::init_proximity_kernel(config);
        let positional_bonus = Self::init_positional_bonus(board_size, config);
        Self {
            config,
            proximity_kernel,
            positional_bonus,
        }
    }

    fn init_proximity_kernel(config: EvaluationConfig) -> Vec<Vec<f32>> {
        let k_size = config.proximity_kernel_size;
        let k_center = k_size / 2;
        let mut proximity_kernel = vec![vec![0.0f32; k_size]; k_size];
        for (r, row) in proximity_kernel.iter_mut().enumerate() {
            for (c, cell) in row.iter_mut().enumerate() {
                let dist = r.abs_diff(k_center) + c.abs_diff(k_center);
                let dist_u16 = u16::try_from(dist).unwrap_or(u16::MAX);
                *cell = 1.0 / (f32::from(dist_u16) + 1.0);
            }
        }
        proximity_kernel
    }

    fn init_positional_bonus(board_size: usize, config: EvaluationConfig) -> Vec<f32> {
        let center = board_size / 2;
        let mut positional_bonus = vec![0.0f32; board_size.saturating_mul(board_size)];
        for r in 0..board_size {
            for c in 0..board_size {
                let row_bonus = center.saturating_sub(center.abs_diff(r));
                let col_bonus = center.saturating_sub(center.abs_diff(c));
                let bonus = row_bonus + col_bonus;
                let bonus_u16 = u16::try_from(bonus).unwrap_or(u16::MAX);
                let idx = r.saturating_mul(board_size).saturating_add(c);
                positional_bonus[idx] = f32::from(bonus_u16) * config.positional_bonus_scale;
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
        for r in 0..board_size {
            for c in 0..board_size {
                if position.board[position.board_index(r, c)] == player {
                    self.apply_proximity_kernel_scaled(board_size, (r, c), scale, score_buffer);
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
        let needed_len = position.board_size.saturating_mul(position.board_size);
        if target.len() != needed_len {
            return;
        }
        target.fill(0.0);
        self.add_proximity_scores(position, player, target);
    }

    pub(crate) fn apply_proximity_delta(
        &self,
        position: &GomokuPosition,
        mov: Coord,
        delta: f32,
        target: &mut [f32],
    ) {
        let needed_len = position.board_size.saturating_mul(position.board_size);
        if target.len() != needed_len {
            return;
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
        let (r, c) = mov;
        let kernel_h = self.proximity_kernel.len();
        let pad_h = kernel_h / 2;
        let pad_w = self.proximity_kernel[0].len() / 2;
        let r = isize::try_from(r).ok();
        let c = isize::try_from(c).ok();
        let pad_h = isize::try_from(pad_h).ok();
        let pad_w = isize::try_from(pad_w).ok();
        let board_limit = isize::try_from(board_size).ok();
        let (Some(r), Some(c), Some(pad_h), Some(pad_w), Some(board_limit)) =
            (r, c, pad_h, pad_w, board_limit)
        else {
            return;
        };
        let Some(base_r) = r.checked_add(pad_h) else {
            return;
        };
        let Some(base_c) = c.checked_add(pad_w) else {
            return;
        };
        for (ki, kernel_row) in self.proximity_kernel.iter().enumerate() {
            let Some(ki) = isize::try_from(ki).ok() else {
                return;
            };
            let out_r = base_r - ki;
            if out_r < 0 || out_r >= board_limit {
                continue;
            }
            let Ok(out_r) = usize::try_from(out_r) else {
                continue;
            };
            let row_start = out_r.saturating_mul(board_size);
            for (kj, &kernel_val) in kernel_row.iter().enumerate() {
                let Some(kj) = isize::try_from(kj).ok() else {
                    return;
                };
                let out_c = base_c - kj;
                if out_c < 0 || out_c >= board_limit {
                    continue;
                }
                let Ok(out_c) = usize::try_from(out_c) else {
                    continue;
                };
                let idx = row_start.saturating_add(out_c);
                target[idx] += kernel_val * scale;
            }
        }
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
        let needed_len = board_size.saturating_mul(board_size);
        if score_buffer.len() != needed_len {
            score_buffer.resize(needed_len, 0.0);
        }
        score_buffer.copy_from_slice(&self.positional_bonus);
        if proximity_scores.len() == needed_len {
            for (score, proximity) in score_buffer.iter_mut().zip(proximity_scores.iter()) {
                *score += *proximity;
            }
        } else {
            self.add_proximity_scores(position, player, score_buffer);
        }
        let patterns_to_score = [
            (position.win_len - 1, 0, evaluation.score_win),
            (position.win_len - 2, 0, evaluation.score_live_four),
            (position.win_len - 3, 0, evaluation.score_live_three),
            (position.win_len.saturating_sub(4), 0, evaluation.score_live_two),
            (position.win_len - 2, 1, evaluation.score_blocked_four),
            (0, position.win_len - 1, evaluation.score_block_win),
            (0, position.win_len - 2, evaluation.score_block_live_four),
            (0, position.win_len - 3, evaluation.score_block_live_three),
            (1, position.win_len - 2, evaluation.score_block_blocked_four),
        ];
        for &(p_req, o_req, score) in &patterns_to_score {
            let windows = position
                .threat_index
                .get_pattern_windows(player, p_req, o_req);
            for window_idx in windows {
                let window = &position.threat_index.all_windows[window_idx];
                for &(r, c) in &window.coords {
                    if position.board[position.board_index(r, c)] == 0 {
                        let idx = r.saturating_mul(board_size).saturating_add(c);
                        score_buffer[idx] += score;
                    }
                }
            }
        }
        scored_moves.extend(moves_to_score.iter().map(|&(r, c)| {
            let idx = r.saturating_mul(board_size).saturating_add(c);
            ((r, c), score_buffer[idx])
        }));
    }
}

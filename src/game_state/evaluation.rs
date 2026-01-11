use super::{Coord, GomokuGameState};
impl GomokuGameState {
    pub(crate) fn init_proximity_kernel(_board_size: usize) -> (Vec<Vec<f32>>, f32) {
        let k_size = 7;
        let k_center = k_size / 2;
        let mut proximity_kernel = vec![vec![0.0f32; k_size]; k_size];
        for (r, row) in proximity_kernel.iter_mut().enumerate() {
            for (c, cell) in row.iter_mut().enumerate() {
                let dist = r.abs_diff(k_center) + c.abs_diff(k_center);
                let dist_u16 = u16::try_from(dist).unwrap_or(u16::MAX);
                *cell = 1.0 / (f32::from(dist_u16) + 1.0);
            }
        }
        (proximity_kernel, 60.0)
    }

    pub(crate) fn init_positional_bonus(board_size: usize) -> Vec<f32> {
        let center = board_size / 2;
        let mut positional_bonus = vec![0.0f32; board_size.saturating_mul(board_size)];
        for r in 0..board_size {
            for c in 0..board_size {
                let row_bonus = center.saturating_sub(center.abs_diff(r));
                let col_bonus = center.saturating_sub(center.abs_diff(c));
                let bonus = row_bonus + col_bonus;
                let bonus_u16 = u16::try_from(bonus).unwrap_or(u16::MAX);
                let idx = r.saturating_mul(board_size).saturating_add(c);
                positional_bonus[idx] = f32::from(bonus_u16) * 0.1;
            }
        }
        positional_bonus
    }

    fn add_proximity_scores(&self, player: u8, score_buffer: &mut [f32]) {
        let board_size = self.board_size;
        let scale = self.proximity_scale;
        for r in 0..board_size {
            for c in 0..board_size {
                if self.board[self.board_index(r, c)] == player {
                    self.apply_proximity_kernel_scaled((r, c), scale, score_buffer);
                }
            }
        }
    }

    fn apply_proximity_kernel_scaled(&self, mov: Coord, scale: f32, target: &mut [f32]) {
        let (r, c) = mov;
        let kernel_h = self.proximity_kernel.len();
        let pad_h = kernel_h / 2;
        let pad_w = self.proximity_kernel[0].len() / 2;
        let r = isize::try_from(r).ok();
        let c = isize::try_from(c).ok();
        let pad_h = isize::try_from(pad_h).ok();
        let pad_w = isize::try_from(pad_w).ok();
        let board_limit = isize::try_from(self.board_size).ok();
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
        let board_size = self.board_size;
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

    pub(crate) fn score_moves(
        &self,
        player: u8,
        moves_to_score: &[Coord],
        score_buffer: &mut Vec<f32>,
    ) -> Vec<(Coord, f32)> {
        let mut scored_moves = Vec::with_capacity(moves_to_score.len());
        self.score_moves_into(player, moves_to_score, score_buffer, &mut scored_moves);
        scored_moves
    }

    pub(crate) fn score_moves_into(
        &self,
        player: u8,
        moves_to_score: &[Coord],
        score_buffer: &mut Vec<f32>,
        scored_moves: &mut Vec<(Coord, f32)>,
    ) {
        const SCORE_WIN: f32 = 10_000_000.0;
        const SCORE_LIVE_FOUR: f32 = 500_000.0;
        const SCORE_BLOCKED_FOUR: f32 = 15_000.0;
        const SCORE_LIVE_THREE: f32 = 10_000.0;
        const SCORE_LIVE_TWO: f32 = 200.0;
        const SCORE_BLOCK_WIN: f32 = 8_000_000.0;
        const SCORE_BLOCK_LIVE_FOUR: f32 = 400_000.0;
        const SCORE_BLOCK_BLOCKED_FOUR: f32 = 12_000.0;
        const SCORE_BLOCK_LIVE_THREE: f32 = 8_000.0;
        scored_moves.clear();
        if moves_to_score.is_empty() {
            return;
        }
        let board_size = self.board_size;
        let needed_len = board_size.saturating_mul(board_size);
        if score_buffer.len() != needed_len {
            score_buffer.resize(needed_len, 0.0);
        }
        score_buffer.copy_from_slice(&self.positional_bonus);
        self.add_proximity_scores(player, score_buffer);
        let patterns_to_score = [
            (self.win_len - 1, 0, SCORE_WIN),
            (self.win_len - 2, 0, SCORE_LIVE_FOUR),
            (self.win_len - 3, 0, SCORE_LIVE_THREE),
            (self.win_len.saturating_sub(4), 0, SCORE_LIVE_TWO),
            (self.win_len - 2, 1, SCORE_BLOCKED_FOUR),
            (0, self.win_len - 1, SCORE_BLOCK_WIN),
            (0, self.win_len - 2, SCORE_BLOCK_LIVE_FOUR),
            (0, self.win_len - 3, SCORE_BLOCK_LIVE_THREE),
            (1, self.win_len - 2, SCORE_BLOCK_BLOCKED_FOUR),
        ];
        for &(p_req, o_req, score) in &patterns_to_score {
            let windows = self.threat_index.get_pattern_windows(player, p_req, o_req);
            for window_idx in windows {
                let window = &self.threat_index.all_windows[window_idx];
                for &(r, c) in &window.coords {
                    if self.board[self.board_index(r, c)] == 0 {
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

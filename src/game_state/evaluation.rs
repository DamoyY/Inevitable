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

    pub(crate) fn init_positional_bonus(board_size: usize) -> Vec<Vec<f32>> {
        let center = board_size / 2;
        let mut positional_bonus = vec![vec![0.0f32; board_size]; board_size];
        for (r, row) in positional_bonus.iter_mut().enumerate() {
            for (c, cell) in row.iter_mut().enumerate() {
                let row_bonus = center.saturating_sub(center.abs_diff(r));
                let col_bonus = center.saturating_sub(center.abs_diff(c));
                let bonus = row_bonus + col_bonus;
                let bonus_u16 = u16::try_from(bonus).unwrap_or(u16::MAX);
                *cell = f32::from(bonus_u16) * 0.1;
            }
        }
        positional_bonus
    }

    pub(crate) fn conv2d(input: &[Vec<f32>], kernel: &[Vec<f32>]) -> Vec<Vec<f32>> {
        let input_h = input.len();
        let input_w = input[0].len();
        let kernel_h = kernel.len();
        let kernel_w = kernel[0].len();
        let pad_h = kernel_h / 2;
        let pad_w = kernel_w / 2;
        let mut output = vec![vec![0.0f32; input_w]; input_h];
        for (i, output_row) in output.iter_mut().enumerate() {
            for (j, output_cell) in output_row.iter_mut().enumerate() {
                let mut sum = 0.0f32;
                for (ki, kernel_row) in kernel.iter().enumerate() {
                    for (kj, &kernel_val) in kernel_row.iter().enumerate() {
                        let ii = i + ki;
                        let jj = j + kj;
                        if ii >= pad_h && jj >= pad_w {
                            let ii_idx = ii - pad_h;
                            let jj_idx = jj - pad_w;
                            if ii_idx < input_h && jj_idx < input_w {
                                sum += input[ii_idx][jj_idx] * kernel_val;
                            }
                        }
                    }
                }
                *output_cell = sum;
            }
        }
        output
    }

    pub(crate) fn score_moves(&self, player: u8, moves_to_score: &[Coord]) -> Vec<(Coord, f32)> {
        const SCORE_WIN: f32 = 10_000_000.0;
        const SCORE_LIVE_FOUR: f32 = 500_000.0;
        const SCORE_BLOCKED_FOUR: f32 = 15_000.0;
        const SCORE_LIVE_THREE: f32 = 10_000.0;
        const SCORE_LIVE_TWO: f32 = 200.0;
        const SCORE_BLOCK_WIN: f32 = 8_000_000.0;
        const SCORE_BLOCK_LIVE_FOUR: f32 = 400_000.0;
        const SCORE_BLOCK_BLOCKED_FOUR: f32 = 12_000.0;
        const SCORE_BLOCK_LIVE_THREE: f32 = 8_000.0;

        if moves_to_score.is_empty() {
            return Vec::new();
        }
        let mut p_board = vec![vec![0.0f32; self.board_size]; self.board_size];
        for (board_row, p_board_row) in self.board.iter().zip(p_board.iter_mut()) {
            for (&board_val, p_board_val) in board_row.iter().zip(p_board_row.iter_mut()) {
                if board_val == player {
                    *p_board_val = 1.0;
                }
            }
        }
        let mut total_scores = self.positional_bonus.clone();
        let proximity_conv = Self::conv2d(&p_board, &self.proximity_kernel);
        for (total_row, conv_row) in total_scores.iter_mut().zip(proximity_conv.iter()) {
            for (total_cell, &conv_val) in total_row.iter_mut().zip(conv_row.iter()) {
                *total_cell += conv_val * self.proximity_scale;
            }
        }
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
            for &window_idx in windows {
                let window = &self.threat_index.all_windows[window_idx];
                for &(r, c) in &window.empty_cells {
                    total_scores[r][c] += score;
                }
            }
        }
        moves_to_score
            .iter()
            .map(|&(r, c)| ((r, c), total_scores[r][c]))
            .collect()
    }
}

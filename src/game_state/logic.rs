use std::collections::HashSet;

use super::{Coord, ForcingMoves, GomokuGameState};

impl GomokuGameState {
    pub fn make_move(&mut self, mov: Coord, player: u8) {
        let (r, c) = mov;
        self.board[r][c] = player;
        self.threat_index.update_on_move(mov, player);

        let mut newly_added_candidates = HashSet::new();
        self.candidate_moves.remove(&mov);

        for dr in -1i32..=1 {
            for dc in -1i32..=1 {
                if dr == 0 && dc == 0 {
                    continue;
                }
                let nr = r as i32 + dr;
                let nc = c as i32 + dc;
                if nr >= 0 && nr < self.board_size as i32 && nc >= 0 && nc < self.board_size as i32
                {
                    let nr = nr as usize;
                    let nc = nc as usize;
                    if self.board[nr][nc] == 0 && !self.candidate_moves.contains(&(nr, nc)) {
                        newly_added_candidates.insert((nr, nc));
                        self.candidate_moves.insert((nr, nc));
                    }
                }
            }
        }

        self.candidate_move_history
            .push((mov, newly_added_candidates));

        let symmetric_coords = self.hasher.get_symmetric_coords(r, c);
        for (i, (sr, sc)) in symmetric_coords.iter().enumerate() {
            self.hashes[i] ^= self.hasher.get_hash(*sr, *sc, player as usize);
        }
        for hash in &mut self.hashes {
            *hash ^= self.hasher.side_to_move_hash;
        }
    }

    pub fn undo_move(&mut self, mov: Coord) {
        let (r, c) = mov;
        let player = self.board[r][c];
        self.threat_index.update_on_undo(mov, player);
        self.board[r][c] = 0;

        let (undone_move, added_by_this_move) = self.candidate_move_history.pop().unwrap();
        assert_eq!(undone_move, mov, "Undo mismatch");

        self.candidate_moves.insert(undone_move);
        for m in added_by_this_move {
            self.candidate_moves.remove(&m);
        }

        for hash in &mut self.hashes {
            *hash ^= self.hasher.side_to_move_hash;
        }
        let symmetric_coords = self.hasher.get_symmetric_coords(r, c);
        for (i, (sr, sc)) in symmetric_coords.iter().enumerate() {
            self.hashes[i] ^= self.hasher.get_hash(*sr, *sc, player as usize);
        }
    }

    pub fn find_forcing_moves(&self, player: u8) -> ForcingMoves {
        let opponent = 3 - player;
        let mut win_in_one_moves = HashSet::new();
        let mut threat_moves = HashSet::new();

        let win_windows = self
            .threat_index
            .get_pattern_windows(player, self.win_len - 1, 0);
        for &window_idx in win_windows {
            let window = &self.threat_index.all_windows[window_idx];
            win_in_one_moves.extend(window.empty_cells.iter());
        }

        let threat_windows = self
            .threat_index
            .get_pattern_windows(opponent, self.win_len - 1, 0);
        for &window_idx in threat_windows {
            let window = &self.threat_index.all_windows[window_idx];
            threat_moves.extend(window.empty_cells.iter());
        }

        (
            win_in_one_moves.into_iter().collect(),
            threat_moves.into_iter().collect(),
        )
    }

    fn conv2d(&self, input: &[Vec<f32>], kernel: &[Vec<f32>]) -> Vec<Vec<f32>> {
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
                        let ii = i as i32 + ki as i32 - pad_h as i32;
                        let jj = j as i32 + kj as i32 - pad_w as i32;
                        if ii >= 0 && ii < input_h as i32 && jj >= 0 && jj < input_w as i32 {
                            sum += input[ii as usize][jj as usize] * kernel_val;
                        }
                    }
                }
                *output_cell = sum;
            }
        }

        output
    }

    fn score_moves(&self, player: u8, moves_to_score: &[Coord]) -> Vec<(Coord, f32)> {
        if moves_to_score.is_empty() {
            return Vec::new();
        }

        let mut p_board = vec![vec![0.0f32; self.board_size]; self.board_size];
        for (r, (board_row, p_board_row)) in self.board.iter().zip(p_board.iter_mut()).enumerate() {
            for (c, (&board_val, p_board_val)) in
                board_row.iter().zip(p_board_row.iter_mut()).enumerate()
            {
                if board_val == player {
                    *p_board_val = 1.0;
                }
                let _ = (r, c);
            }
        }

        let mut total_scores = self.positional_bonus.clone();
        let proximity_conv = self.conv2d(&p_board, &self.proximity_kernel);

        for (total_row, conv_row) in total_scores.iter_mut().zip(proximity_conv.iter()) {
            for (total_cell, &conv_val) in total_row.iter_mut().zip(conv_row.iter()) {
                *total_cell += conv_val * self.proximity_scale;
            }
        }

        const SCORE_WIN: f32 = 10_000_000.0;
        const SCORE_LIVE_FOUR: f32 = 500_000.0;
        const SCORE_BLOCKED_FOUR: f32 = 15_000.0;
        const SCORE_LIVE_THREE: f32 = 10_000.0;
        const SCORE_LIVE_TWO: f32 = 200.0;
        const SCORE_BLOCK_WIN: f32 = 8_000_000.0;
        const SCORE_BLOCK_LIVE_FOUR: f32 = 400_000.0;
        const SCORE_BLOCK_BLOCKED_FOUR: f32 = 12_000.0;
        const SCORE_BLOCK_LIVE_THREE: f32 = 8_000.0;

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

    pub fn get_legal_moves(&self, player: u8) -> Vec<Coord> {
        let (win_moves, threat_moves) = self.find_forcing_moves(player);

        if !win_moves.is_empty() {
            return win_moves;
        }

        if !threat_moves.is_empty() {
            let scores = self.score_moves(player, &threat_moves);
            let mut scored_moves: Vec<(Coord, f32)> = scores;
            scored_moves.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
            return scored_moves.into_iter().map(|(coord, _)| coord).collect();
        }

        let mut empties = Vec::new();
        for r in 0..self.board_size {
            for c in 0..self.board_size {
                if self.board[r][c] == 0 {
                    empties.push((r, c));
                }
            }
        }

        if empties.is_empty() {
            return Vec::new();
        }

        let scores = self.score_moves(player, &empties);
        let mut scored_moves: Vec<(Coord, f32)> = scores;
        scored_moves.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
        scored_moves.into_iter().map(|(coord, _)| coord).collect()
    }
}

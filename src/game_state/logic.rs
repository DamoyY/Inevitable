use std::collections::HashSet;

use super::{Bitboard, Coord, ForcingMoves, GomokuGameState};
impl GomokuGameState {
    pub fn make_move(&mut self, mov: Coord, player: u8) {
        let (r, c) = mov;
        self.board[r][c] = player;
        self.bitboard.set(r, c, player);
        self.threat_index.update_on_move(mov, player);
        let mut newly_added_candidates = HashSet::new();
        self.candidate_moves.remove(&mov);
        let occupied = self.bitboard.occupied();
        let all_neighbors = self.bitboard.neighbors(&occupied);
        for coord in self.bitboard.iter_bits(&all_neighbors) {
            if !self.candidate_moves.contains(&coord) {
                newly_added_candidates.insert(coord);
                self.candidate_moves.insert(coord);
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
        self.bitboard.clear(r, c);
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
        let empty_bits = self.bitboard.empty();
        if Bitboard::is_all_zeros(&empty_bits) {
            return Vec::new();
        }
        let empties: Vec<Coord> = self.bitboard.iter_bits(&empty_bits).collect();
        let scores = self.score_moves(player, &empties);
        let mut scored_moves: Vec<(Coord, f32)> = scores;
        scored_moves.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
        scored_moves.into_iter().map(|(coord, _)| coord).collect()
    }
}

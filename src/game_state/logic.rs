use std::collections::HashSet;

use super::{Bitboard, Coord, ForcingMoves, GomokuGameState};
impl GomokuGameState {
    fn collect_empty_cells<'a, I>(&self, window_indices: I) -> HashSet<Coord>
    where
        I: IntoIterator<Item = &'a usize>,
    {
        let mut cells = HashSet::new();
        for &window_idx in window_indices {
            let window = &self.threat_index.all_windows[window_idx];
            cells.extend(window.empty_cells.iter());
        }
        cells
    }

    fn score_and_sort_moves(&self, player: u8, moves: &[Coord]) -> Vec<Coord> {
        let mut scored_moves = self.score_moves(player, moves);
        scored_moves.sort_by(|a, b| b.1.total_cmp(&a.1));
        scored_moves.into_iter().map(|(coord, _)| coord).collect()
    }

    pub fn make_move(&mut self, mov: Coord, player: u8) {
        let (r, c) = mov;
        self.board[r][c] = player;
        self.bitboard.set(r, c, player);
        self.threat_index.update_on_move(mov, player);
        let mut newly_added_candidates = HashSet::new();
        self.candidate_moves.remove(&mov);
        for coord in self.neighbor_coords() {
            if self.candidate_moves.insert(coord) {
                newly_added_candidates.insert(coord);
            }
        }
        self.candidate_move_history
            .push((mov, newly_added_candidates));
        self.hash ^= self.hasher.get_hash(r, c, player as usize);
        self.hash ^= self.hasher.side_to_move_hash;
    }

    pub fn undo_move(&mut self, mov: Coord) {
        let Some((undone_move, added_by_this_move)) = self.candidate_move_history.pop() else {
            return;
        };
        let (r, c) = mov;
        let player = self.board[r][c];
        self.threat_index.update_on_undo(mov, player);
        self.board[r][c] = 0;
        self.bitboard.clear(r, c);
        debug_assert_eq!(undone_move, mov, "Undo mismatch");
        self.candidate_moves.insert(undone_move);
        for m in added_by_this_move {
            self.candidate_moves.remove(&m);
        }
        self.hash ^= self.hasher.side_to_move_hash;
        self.hash ^= self.hasher.get_hash(r, c, player as usize);
    }

    #[must_use]
    pub fn find_forcing_moves(&self, player: u8) -> ForcingMoves {
        let opponent = 3 - player;
        let win_windows = self
            .threat_index
            .get_pattern_windows(player, self.win_len - 1, 0);
        let win_in_one_moves = self.collect_empty_cells(win_windows);
        let threat_windows = self
            .threat_index
            .get_pattern_windows(opponent, self.win_len - 1, 0);
        let threat_moves = self.collect_empty_cells(threat_windows);
        (
            win_in_one_moves.into_iter().collect(),
            threat_moves.into_iter().collect(),
        )
    }

    #[must_use]
    pub fn get_legal_moves(&self, player: u8) -> Vec<Coord> {
        let (win_moves, threat_moves) = self.find_forcing_moves(player);

        if !win_moves.is_empty() {
            return win_moves;
        }
        if !threat_moves.is_empty() {
            return self.score_and_sort_moves(player, &threat_moves);
        }
        let empty_bits = self.bitboard.empty();
        if Bitboard::is_all_zeros(&empty_bits) {
            return Vec::new();
        }
        let empties: Vec<Coord> = self.bitboard.iter_bits(&empty_bits).collect();
        self.score_and_sort_moves(player, &empties)
    }
}

use std::collections::HashMap;

use crate::utils::board_index;

mod buckets;
mod window;

use buckets::PatternBuckets;
pub use window::Window;

#[derive(Clone)]
pub struct ThreatIndex {
    pub board_size: usize,
    pub win_len: usize,
    pub point_to_windows_map: HashMap<(usize, usize), Vec<usize>>,
    pub all_windows: Vec<Window>,
    pattern_buckets: PatternBuckets,
}

impl ThreatIndex {
    #[must_use]
    pub fn new(board_size: usize, win_len: usize) -> Self {
        let mut threat_index = Self {
            board_size,
            win_len,
            point_to_windows_map: HashMap::new(),
            all_windows: Vec::new(),
            pattern_buckets: PatternBuckets::empty(),
        };
        threat_index.enumerate_windows();
        threat_index.pattern_buckets = PatternBuckets::new(win_len, threat_index.all_windows.len());
        threat_index
    }

    fn enumerate_windows(&mut self) {
        self.add_direction_windows(
            0..self.board_size,
            0..=self.board_size - self.win_len,
            &|r, c, i| (r, c + i),
        );
        self.add_direction_windows(
            0..=self.board_size - self.win_len,
            0..self.board_size,
            &|r, c, i| (r + i, c),
        );
        self.add_direction_windows(
            0..=self.board_size - self.win_len,
            0..=self.board_size - self.win_len,
            &|r, c, i| (r + i, c + i),
        );
        self.add_direction_windows(
            0..=self.board_size - self.win_len,
            (self.win_len - 1)..self.board_size,
            &|r, c, i| (r + i, c - i),
        );
    }

    fn add_direction_windows<RI, CI, F>(&mut self, rows: RI, cols: CI, coord_fn: &F)
    where
        RI: Iterator<Item = usize>,
        CI: Iterator<Item = usize>,
        F: Fn(usize, usize, usize) -> (usize, usize),
    {
        let cols: Vec<usize> = cols.collect();
        for r in rows {
            for &c in &cols {
                let coords: Vec<(usize, usize)> =
                    (0..self.win_len).map(|i| coord_fn(r, c, i)).collect();
                self.add_window(coords);
            }
        }
    }

    fn add_window(&mut self, coords: Vec<(usize, usize)>) {
        let window_idx = self.all_windows.len();
        let window = Window::new(coords.clone());
        self.all_windows.push(window);

        for point in coords {
            self.point_to_windows_map
                .entry(point)
                .or_default()
                .push(window_idx);
        }
    }

    pub fn initialize_from_board(&mut self, board: &[u8]) {
        let win_len = self.win_len;
        for window_idx in 0..self.all_windows.len() {
            let window = &mut self.all_windows[window_idx];
            window.p1_count = 0;
            window.p2_count = 0;
            window.empty_cells.clear();

            for &(r, c) in &window.coords {
                let player = board[board_index(self.board_size, r, c)];
                if player == 1 {
                    window.p1_count += 1;
                } else if player == 2 {
                    window.p2_count += 1;
                }
            }
            window.empty_count = win_len - window.p1_count - window.p2_count;

            for &(r, c) in &window.coords {
                if board[board_index(self.board_size, r, c)] == 0 {
                    window.empty_cells.insert((r, c));
                }
            }
        }

        self.pattern_buckets.reset();
        for window_idx in 0..self.all_windows.len() {
            self.update_bucket_add(window_idx);
        }
    }

    const fn window_bucket_keys(window: &Window) -> [(u8, usize, usize); 2] {
        [
            (1, window.p1_count, window.p2_count),
            (2, window.p2_count, window.p1_count),
        ]
    }

    fn update_bucket_add(&mut self, window_idx: usize) {
        let window = &self.all_windows[window_idx];
        let keys = Self::window_bucket_keys(window);
        self.pattern_buckets
            .insert(keys[0].0, window_idx, keys[0].1, keys[0].2);
        self.pattern_buckets
            .insert(keys[1].0, window_idx, keys[1].1, keys[1].2);
    }

    fn update_bucket_remove(&mut self, window_idx: usize) {
        let window = &self.all_windows[window_idx];
        let keys = Self::window_bucket_keys(window);
        self.pattern_buckets.remove(keys[0].0, window_idx);
        self.pattern_buckets.remove(keys[1].0, window_idx);
    }

    fn apply_window_update(&mut self, mov: (usize, usize), player: u8, is_move: bool) {
        let window_indices: Vec<usize> = self
            .point_to_windows_map
            .get(&mov)
            .cloned()
            .unwrap_or_default();
        for window_idx in window_indices {
            self.update_bucket_remove(window_idx);

            let window = &mut self.all_windows[window_idx];
            if is_move {
                window.empty_count -= 1;
                window.empty_cells.remove(&mov);
            } else {
                window.empty_count += 1;
                window.empty_cells.insert(mov);
            }
            if player == 1 {
                if is_move {
                    window.p1_count += 1;
                } else {
                    window.p1_count -= 1;
                }
            } else if is_move {
                window.p2_count += 1;
            } else {
                window.p2_count -= 1;
            }
            self.update_bucket_add(window_idx);
        }
    }

    pub fn update_on_move(&mut self, mov: (usize, usize), player: u8) {
        self.apply_window_update(mov, player, true);
    }

    pub fn update_on_undo(&mut self, mov: (usize, usize), player: u8) {
        self.apply_window_update(mov, player, false);
    }

    pub fn get_pattern_windows(
        &self,
        player: u8,
        p_count: usize,
        o_count: usize,
    ) -> impl Iterator<Item = usize> + '_ {
        self.pattern_buckets.iter(player, p_count, o_count)
    }
}

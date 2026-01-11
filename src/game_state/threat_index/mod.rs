use buckets::PatternBuckets;
use smallvec::SmallVec;

use crate::utils::board_index;
mod buckets;
#[derive(Clone)]
pub struct Window {
    pub coords: Vec<(usize, usize)>,
    pub p1_count: usize,
    pub p2_count: usize,
    pub empty_count: usize,
}
impl Window {
    pub const fn new(coords: Vec<(usize, usize)>) -> Self {
        let empty_count = coords.len();
        Self {
            coords,
            p1_count: 0,
            p2_count: 0,
            empty_count,
        }
    }
}
#[derive(Clone)]
pub struct ThreatIndex {
    pub board_size: usize,
    pub win_len: usize,
    pub point_to_windows_map: Vec<SmallVec<[u16; 4]>>,
    pub all_windows: Vec<Window>,
    pattern_buckets: PatternBuckets,
}
impl ThreatIndex {
    #[must_use]
    pub fn new(board_size: usize, win_len: usize) -> Self {
        let total_windows = Self::window_count(board_size, win_len);
        let point_count = board_size.saturating_mul(board_size);
        let mut threat_index = Self {
            board_size,
            win_len,
            point_to_windows_map: vec![SmallVec::new(); point_count],
            all_windows: Vec::with_capacity(total_windows),
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
        let Ok(window_idx_u16) = u16::try_from(window_idx) else {
            return;
        };
        let window = Window::new(coords.clone());
        self.all_windows.push(window);
        for point in coords {
            let point_idx = board_index(self.board_size, point.0, point.1);
            self.point_to_windows_map[point_idx].push(window_idx_u16);
        }
    }

    pub fn initialize_from_board(&mut self, board: &[u8]) {
        let win_len = self.win_len;
        for window_idx in 0..self.all_windows.len() {
            let window = &mut self.all_windows[window_idx];
            window.p1_count = 0;
            window.p2_count = 0;
            for &(r, c) in &window.coords {
                let player = board[board_index(self.board_size, r, c)];
                if player == 1 {
                    window.p1_count += 1;
                } else if player == 2 {
                    window.p2_count += 1;
                }
            }
            window.empty_count = win_len - window.p1_count - window.p2_count;
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
        if window.p1_count > 0 && window.p2_count > 0 {
            return;
        }
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
        let point_idx = board_index(self.board_size, mov.0, mov.1);
        let window_indices = self.point_to_windows_map[point_idx].clone();
        for window_idx in window_indices {
            let window_idx = usize::from(window_idx);
            self.update_bucket_remove(window_idx);
            let window = &mut self.all_windows[window_idx];
            if is_move {
                window.empty_count -= 1;
            } else {
                window.empty_count += 1;
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

    const fn window_count(board_size: usize, win_len: usize) -> usize {
        if board_size < win_len {
            return 0;
        }
        let span = board_size - win_len + 1;
        let lines = board_size.saturating_mul(span);
        let diags = span.saturating_mul(span);
        2usize
            .saturating_mul(lines)
            .saturating_add(2usize.saturating_mul(diags))
    }
}

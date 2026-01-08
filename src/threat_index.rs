use std::{
    collections::{HashMap, HashSet},
    sync::LazyLock,
};

#[derive(Clone)]
pub struct Window {
    pub coords: Vec<(usize, usize)>,
    pub p1_count: usize,
    pub p2_count: usize,
    pub empty_count: usize,
    pub empty_cells: HashSet<(usize, usize)>,
}

impl Window {
    pub fn new(coords: Vec<(usize, usize)>) -> Self {
        let empty_cells: HashSet<(usize, usize)> = coords.iter().copied().collect();
        let empty_count = coords.len();
        Self {
            coords,
            p1_count: 0,
            p2_count: 0,
            empty_count,
            empty_cells,
        }
    }
}

static EMPTY_SET: LazyLock<HashSet<usize>> = LazyLock::new(HashSet::new);

pub struct ThreatIndex {
    pub board_size: usize,
    pub win_len: usize,
    pub point_to_windows_map: HashMap<(usize, usize), Vec<usize>>,
    pub all_windows: Vec<Window>,
    pub pattern_buckets: HashMap<(u8, usize, usize), HashSet<usize>>,
}

impl ThreatIndex {
    pub fn new(board_size: usize, win_len: usize) -> Self {
        let mut threat_index = Self {
            board_size,
            win_len,
            point_to_windows_map: HashMap::new(),
            all_windows: Vec::new(),
            pattern_buckets: HashMap::new(),
        };
        threat_index.enumerate_windows();
        threat_index
    }

    fn enumerate_windows(&mut self) {
        for r in 0..self.board_size {
            for c in 0..=self.board_size - self.win_len {
                let coords: Vec<(usize, usize)> = (0..self.win_len).map(|i| (r, c + i)).collect();
                self.add_window(coords);
            }
        }
        for r in 0..=self.board_size - self.win_len {
            for c in 0..self.board_size {
                let coords: Vec<(usize, usize)> = (0..self.win_len).map(|i| (r + i, c)).collect();
                self.add_window(coords);
            }
        }
        for r in 0..=self.board_size - self.win_len {
            for c in 0..=self.board_size - self.win_len {
                let coords: Vec<(usize, usize)> =
                    (0..self.win_len).map(|i| (r + i, c + i)).collect();
                self.add_window(coords);
            }
        }
        for r in 0..=self.board_size - self.win_len {
            for c in (self.win_len - 1)..self.board_size {
                let coords: Vec<(usize, usize)> =
                    (0..self.win_len).map(|i| (r + i, c - i)).collect();
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

    pub fn initialize_from_board(&mut self, board: &[Vec<u8>]) {
        let win_len = self.win_len;
        for window_idx in 0..self.all_windows.len() {
            let window = &mut self.all_windows[window_idx];
            window.p1_count = 0;
            window.p2_count = 0;
            window.empty_cells.clear();

            for &(r, c) in &window.coords {
                let player = board[r][c];
                if player == 1 {
                    window.p1_count += 1;
                } else if player == 2 {
                    window.p2_count += 1;
                }
            }
            window.empty_count = win_len - window.p1_count - window.p2_count;

            for &(r, c) in &window.coords {
                if board[r][c] == 0 {
                    window.empty_cells.insert((r, c));
                }
            }
        }

        self.pattern_buckets.clear();
        for window_idx in 0..self.all_windows.len() {
            self.update_bucket_add(window_idx);
        }
    }

    fn update_bucket_add(&mut self, window_idx: usize) {
        let window = &self.all_windows[window_idx];
        let key1 = (1, window.p1_count, window.p2_count);
        let key2 = (2, window.p2_count, window.p1_count);

        self.pattern_buckets
            .entry(key1)
            .or_default()
            .insert(window_idx);
        self.pattern_buckets
            .entry(key2)
            .or_default()
            .insert(window_idx);
    }

    fn update_bucket_remove(&mut self, window_idx: usize) {
        let window = &self.all_windows[window_idx];
        let key1 = (1, window.p1_count, window.p2_count);
        let key2 = (2, window.p2_count, window.p1_count);

        if let Some(set) = self.pattern_buckets.get_mut(&key1) {
            set.remove(&window_idx);
        }
        if let Some(set) = self.pattern_buckets.get_mut(&key2) {
            set.remove(&window_idx);
        }
    }

    pub fn update_on_move(&mut self, mov: (usize, usize), player: u8) {
        let window_indices: Vec<usize> = self
            .point_to_windows_map
            .get(&mov)
            .cloned()
            .unwrap_or_default();

        for window_idx in window_indices {
            self.update_bucket_remove(window_idx);

            let window = &mut self.all_windows[window_idx];
            window.empty_count -= 1;
            window.empty_cells.remove(&mov);
            if player == 1 {
                window.p1_count += 1;
            } else {
                window.p2_count += 1;
            }

            self.update_bucket_add(window_idx);
        }
    }

    pub fn update_on_undo(&mut self, mov: (usize, usize), player: u8) {
        let window_indices: Vec<usize> = self
            .point_to_windows_map
            .get(&mov)
            .cloned()
            .unwrap_or_default();

        for window_idx in window_indices {
            self.update_bucket_remove(window_idx);

            let window = &mut self.all_windows[window_idx];
            window.empty_count += 1;
            window.empty_cells.insert(mov);
            if player == 1 {
                window.p1_count -= 1;
            } else {
                window.p2_count -= 1;
            }

            self.update_bucket_add(window_idx);
        }
    }

    pub fn get_pattern_windows(
        &self,
        player: u8,
        p_count: usize,
        o_count: usize,
    ) -> &HashSet<usize> {
        self.pattern_buckets
            .get(&(player, p_count, o_count))
            .unwrap_or(&EMPTY_SET)
    }
}

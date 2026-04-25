use crate::{checked, utils::board_index};
use smallvec::SmallVec;
mod buckets;
use buckets::PatternBuckets;
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
    board_size: usize,
    win_len: usize,
    point_to_windows_map: Vec<SmallVec<[u16; 4]>>,
    all_windows: Vec<Window>,
    pattern_buckets: PatternBuckets,
}
impl ThreatIndex {
    #[inline]
    #[must_use]
    pub fn new(board_size: usize, win_len: usize) -> Self {
        let total_windows = Self::window_count(board_size, win_len);
        let point_count =
            checked::mul_usize(board_size, board_size, "ThreatIndex::new::point_count");
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
    pub(crate) fn window(&self, window_index: usize) -> &Window {
        let Some(window) = self.all_windows.get(window_index) else {
            eprintln!("ThreatIndex::window 窗口索引越界: {window_index}");
            panic!("ThreatIndex::window 窗口索引越界");
        };
        window
    }
    fn window_mut(&mut self, window_index: usize) -> &mut Window {
        let Some(window) = self.all_windows.get_mut(window_index) else {
            eprintln!("ThreatIndex::window_mut 窗口索引越界: {window_index}");
            panic!("ThreatIndex::window_mut 窗口索引越界");
        };
        window
    }
    fn window_indices_for_point(
        &self,
        row_index: usize,
        column_index: usize,
    ) -> &SmallVec<[u16; 4]> {
        let point_index = board_index(self.board_size, row_index, column_index);
        let Some(window_indices) = self.point_to_windows_map.get(point_index) else {
            eprintln!(
                "ThreatIndex::window_indices_for_point 点索引越界: ({row_index}, {column_index})"
            );
            panic!("ThreatIndex::window_indices_for_point 点索引越界");
        };
        window_indices
    }
    fn enumerate_windows(&mut self) {
        if self.board_size < self.win_len {
            return;
        }
        let start_limit = checked::sub_usize(
            self.board_size,
            self.win_len,
            "ThreatIndex::enumerate_windows::start_limit",
        );
        let descending_column_start = checked::sub_usize(
            self.win_len,
            1_usize,
            "ThreatIndex::enumerate_windows::descending_column_start",
        );
        self.add_direction_windows(
            0..self.board_size,
            0..=start_limit,
            &|row_index, column_index, offset| {
                (
                    row_index,
                    checked::add_usize(
                        column_index,
                        offset,
                        "ThreatIndex::enumerate_windows::horizontal_column",
                    ),
                )
            },
        );
        self.add_direction_windows(
            0..=start_limit,
            0..self.board_size,
            &|row_index, column_index, offset| {
                (
                    checked::add_usize(
                        row_index,
                        offset,
                        "ThreatIndex::enumerate_windows::vertical_row",
                    ),
                    column_index,
                )
            },
        );
        self.add_direction_windows(
            0..=start_limit,
            0..=start_limit,
            &|row_index, column_index, offset| {
                (
                    checked::add_usize(
                        row_index,
                        offset,
                        "ThreatIndex::enumerate_windows::diag_down_row",
                    ),
                    checked::add_usize(
                        column_index,
                        offset,
                        "ThreatIndex::enumerate_windows::diag_down_column",
                    ),
                )
            },
        );
        self.add_direction_windows(
            0..=start_limit,
            descending_column_start..self.board_size,
            &|row_index, column_index, offset| {
                (
                    checked::add_usize(
                        row_index,
                        offset,
                        "ThreatIndex::enumerate_windows::diag_up_row",
                    ),
                    checked::sub_usize(
                        column_index,
                        offset,
                        "ThreatIndex::enumerate_windows::diag_up_column",
                    ),
                )
            },
        );
    }
    fn add_direction_windows<RI, CI, F>(&mut self, row_iter: RI, column_iter: CI, coord_fn: &F)
    where
        RI: Iterator<Item = usize>,
        CI: Iterator<Item = usize>,
        F: Fn(usize, usize, usize) -> (usize, usize),
    {
        let column_indices: Vec<usize> = column_iter.collect();
        for row_index in row_iter {
            for &column_index in &column_indices {
                let coords: Vec<(usize, usize)> = (0..self.win_len)
                    .map(|offset| coord_fn(row_index, column_index, offset))
                    .collect();
                self.add_window(coords);
            }
        }
    }
    fn add_window(&mut self, coords: Vec<(usize, usize)>) {
        let window_index = self.all_windows.len();
        let window_index_u16 =
            checked::usize_to_u16(window_index, "ThreatIndex::add_window::window_index");
        self.all_windows.push(Window::new(coords.clone()));
        for (row_index, column_index) in coords {
            let point_index = board_index(self.board_size, row_index, column_index);
            let Some(window_indices) = self.point_to_windows_map.get_mut(point_index) else {
                eprintln!("ThreatIndex::add_window 点索引越界: ({row_index}, {column_index})");
                panic!("ThreatIndex::add_window 点索引越界");
            };
            window_indices.push(window_index_u16);
        }
    }
    #[inline]
    pub fn initialize_from_board(&mut self, board: &[u8]) {
        let win_len = self.win_len;
        for window in &mut self.all_windows {
            window.p1_count = 0;
            window.p2_count = 0;
            for &(row_index, column_index) in &window.coords {
                let board_index = board_index(self.board_size, row_index, column_index);
                let Some(&player) = board.get(board_index) else {
                    eprintln!(
                        "ThreatIndex::initialize_from_board 棋盘索引越界: ({row_index}, {column_index})"
                    );
                    panic!("ThreatIndex::initialize_from_board 棋盘索引越界");
                };
                if player == 1 {
                    window.p1_count = checked::add_usize(
                        window.p1_count,
                        1_usize,
                        "ThreatIndex::initialize_from_board::p1_count",
                    );
                } else if player == 2 {
                    window.p2_count = checked::add_usize(
                        window.p2_count,
                        1_usize,
                        "ThreatIndex::initialize_from_board::p2_count",
                    );
                }
            }
            let occupied_count = checked::add_usize(
                window.p1_count,
                window.p2_count,
                "ThreatIndex::initialize_from_board::occupied_count",
            );
            window.empty_count = checked::sub_usize(
                win_len,
                occupied_count,
                "ThreatIndex::initialize_from_board::empty_count",
            );
        }
        self.pattern_buckets.reset();
        for window_index in 0..self.all_windows.len() {
            self.update_bucket_add(window_index);
        }
    }
    const fn window_bucket_keys(window: &Window) -> [(u8, usize, usize); 2] {
        [
            (1, window.p1_count, window.p2_count),
            (2, window.p2_count, window.p1_count),
        ]
    }
    fn update_bucket_add(&mut self, window_index: usize) {
        let window = self.window(window_index);
        if window.p1_count > 0 && window.p2_count > 0 {
            return;
        }
        let keys = Self::window_bucket_keys(window);
        self.pattern_buckets
            .insert(keys[0].0, window_index, keys[0].1, keys[0].2);
        self.pattern_buckets
            .insert(keys[1].0, window_index, keys[1].1, keys[1].2);
    }
    fn update_bucket_remove(&mut self, window_index: usize) {
        let window = self.window(window_index);
        let keys = Self::window_bucket_keys(window);
        self.pattern_buckets.remove(keys[0].0, window_index);
        self.pattern_buckets.remove(keys[1].0, window_index);
    }
    fn apply_window_update(&mut self, mov: (usize, usize), player: u8, is_move: bool) {
        let window_indices = self.window_indices_for_point(mov.0, mov.1).clone();
        for window_index_u16 in window_indices {
            let window_index = usize::from(window_index_u16);
            self.update_bucket_remove(window_index);
            let window = self.window_mut(window_index);
            if is_move {
                window.empty_count = checked::sub_usize(
                    window.empty_count,
                    1_usize,
                    "ThreatIndex::apply_window_update::empty_count_remove",
                );
            } else {
                window.empty_count = checked::add_usize(
                    window.empty_count,
                    1_usize,
                    "ThreatIndex::apply_window_update::empty_count_restore",
                );
            }
            match player {
                1 => {
                    if is_move {
                        window.p1_count = checked::add_usize(
                            window.p1_count,
                            1_usize,
                            "ThreatIndex::apply_window_update::p1_add",
                        );
                    } else {
                        window.p1_count = checked::sub_usize(
                            window.p1_count,
                            1_usize,
                            "ThreatIndex::apply_window_update::p1_sub",
                        );
                    }
                }
                2 => {
                    if is_move {
                        window.p2_count = checked::add_usize(
                            window.p2_count,
                            1_usize,
                            "ThreatIndex::apply_window_update::p2_add",
                        );
                    } else {
                        window.p2_count = checked::sub_usize(
                            window.p2_count,
                            1_usize,
                            "ThreatIndex::apply_window_update::p2_sub",
                        );
                    }
                }
                _ => {
                    eprintln!("ThreatIndex::apply_window_update 收到非法玩家编号: {player}");
                    panic!("ThreatIndex::apply_window_update 收到非法玩家编号");
                }
            }
            self.update_bucket_add(window_index);
        }
    }
    #[inline]
    pub fn update_on_move(&mut self, mov: (usize, usize), player: u8) {
        self.apply_window_update(mov, player, true);
    }
    #[inline]
    pub fn update_on_undo(&mut self, mov: (usize, usize), player: u8) {
        self.apply_window_update(mov, player, false);
    }
    #[inline]
    pub fn get_pattern_windows(
        &self,
        player: u8,
        player_count: usize,
        opponent_count: usize,
    ) -> impl Iterator<Item = usize> + '_ {
        self.pattern_buckets
            .iter(player, player_count, opponent_count)
    }
    fn window_count(board_size: usize, win_len: usize) -> usize {
        if board_size < win_len {
            return 0;
        }
        let span_base =
            checked::sub_usize(board_size, win_len, "ThreatIndex::window_count::span_base");
        let span = checked::add_usize(span_base, 1_usize, "ThreatIndex::window_count::span");
        let line_windows =
            checked::mul_usize(board_size, span, "ThreatIndex::window_count::line_windows");
        let diagonal_windows =
            checked::mul_usize(span, span, "ThreatIndex::window_count::diagonal_windows");
        let total_line_windows = checked::mul_usize(
            2_usize,
            line_windows,
            "ThreatIndex::window_count::total_line_windows",
        );
        let total_diagonal_windows = checked::mul_usize(
            2_usize,
            diagonal_windows,
            "ThreatIndex::window_count::total_diagonal_windows",
        );
        checked::add_usize(
            total_line_windows,
            total_diagonal_windows,
            "ThreatIndex::window_count::total_windows",
        )
    }
}

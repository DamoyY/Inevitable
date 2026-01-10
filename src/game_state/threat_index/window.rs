use std::collections::HashSet;

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

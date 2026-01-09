use rand::{Rng, SeedableRng, rngs::StdRng};

pub struct ZobristHasher {
    pub(crate) board_size: usize,
    pub(crate) zobrist_table: Vec<Vec<[u64; 3]>>,
    pub side_to_move_hash: u64,
}

impl ZobristHasher {
    #[must_use]
    pub fn new(board_size: usize) -> Self {
        Self::with_seed(board_size, 0x005F_15E5_D0FE_DF9A)
    }

    #[must_use]
    pub fn with_seed(board_size: usize, seed: u64) -> Self {
        let mut rng = StdRng::seed_from_u64(seed);
        let mut zobrist_table = vec![vec![[0u64; 3]; board_size]; board_size];
        for row in &mut zobrist_table {
            for cell in row.iter_mut() {
                for piece in cell.iter_mut() {
                    *piece = rng.random::<u64>() & ((1u64 << 63) - 1);
                }
            }
        }
        let side_to_move_hash = rng.random::<u64>() & ((1u64 << 63) - 1);
        Self {
            board_size,
            zobrist_table,
            side_to_move_hash,
        }
    }

    #[must_use]
    pub fn get_hash(&self, r: usize, c: usize, piece: usize) -> u64 {
        self.zobrist_table[r][c][piece]
    }

    #[must_use]
    pub const fn get_symmetric_coords(&self, r: usize, c: usize) -> [(usize, usize); 8] {
        let n = self.board_size - 1;
        [
            (r, c),
            (c, n - r),
            (n - r, n - c),
            (n - c, r),
            (r, n - c),
            (c, r),
            (n - r, c),
            (n - c, n - r),
        ]
    }
}

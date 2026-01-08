use rand::Rng;

pub struct ZobristHasher {
    pub(crate) board_size: usize,
    pub(crate) zobrist_table: Vec<Vec<[u64; 3]>>,
    pub side_to_move_hash: u64,
}

impl ZobristHasher {
    pub fn new(board_size: usize) -> Self {
        let mut rng = rand::rng();
        let mut zobrist_table = vec![vec![[0u64; 3]; board_size]; board_size];
        for row in zobrist_table.iter_mut() {
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

    pub fn get_hash(&self, r: usize, c: usize, piece: usize) -> u64 {
        self.zobrist_table[r][c][piece]
    }

    pub fn get_symmetric_coords(&self, r: usize, c: usize) -> [(usize, usize); 8] {
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

use std::sync::Arc;

use smallvec::SmallVec;
mod bitboard;
mod evaluation;
mod moves;
mod state;
mod threat_index;
mod zobrist;
pub use bitboard::{Bitboard, BitboardWorkspace};
pub use threat_index::ThreatIndex;
pub use zobrist::ZobristHasher;
pub type Coord = (usize, usize);
pub type MoveHistory = Vec<(Coord, SmallVec<[u64; 8]>)>;
pub type ForcingMoves = (Vec<Coord>, Vec<Coord>);
macro_rules! define_move_apply_timing {
    ( $( $field:ident => $stat_field:ident ),* $(,)? ) => {
        pub struct MoveApplyTiming {
            $(pub $field: u64,)*
        }
        impl MoveApplyTiming {
            #[must_use]
            pub const fn zero() -> Self {
                Self {
                    $($field: 0,)*
                }
            }
        }
    };
}
crate::for_each_move_apply_timing!(define_move_apply_timing);
#[derive(Clone, Copy, Default)]
pub struct MoveGenTiming {
    pub candidate_gen_ns: u64,
    pub scoring_ns: u64,
}
#[derive(Clone)]
pub struct GomokuGameState {
    pub board: Vec<u8>,
    pub bitboard: Bitboard,
    pub board_size: usize,
    pub win_len: usize,
    pub hasher: Arc<ZobristHasher>,
    pub hash: u64,
    pub threat_index: ThreatIndex,
    pub candidate_moves: SmallVec<[u64; 8]>,
    pub(crate) candidate_move_history: MoveHistory,
    pub(crate) proximity_kernel: Vec<Vec<f32>>,
    pub(crate) proximity_scale: f32,
    pub(crate) positional_bonus: Vec<f32>,
}

use crate::checked;
use crate::config::EvaluationWeights;
use crate::utils::duration_to_ns;
use alloc::sync::Arc;
use smallvec::SmallVec;
use std::time::Instant;
mod bitboard;
mod evaluation;
mod moves;
mod state;
mod threat_index;
pub type Bitboard = bitboard::Bitboard;
pub type BitboardWorkspace = bitboard::BitboardWorkspace;
pub type ZobristHasher = state::ZobristHasher;
pub type ThreatIndex = threat_index::ThreatIndex;
pub type Coord = (usize, usize);
pub type MoveHistory = Vec<(Coord, SmallVec<[Coord; 8]>)>;
pub type ForcingMoves = (Vec<Coord>, Vec<Coord>);
macro_rules ! define_move_apply_timing { ($ ($ field : ident => $ stat_field : ident) ,* $ (,) ?) => { pub struct MoveApplyTiming { $ (pub $ field : u64 ,) * } impl MoveApplyTiming { # [inline] # [must_use] pub const fn zero () -> Self { Self { $ ($ field : 0 ,) * } } } } ; }
crate::for_each_move_apply_timing!(define_move_apply_timing);
#[derive(Clone, Copy, Default)]
pub struct MoveGenTiming {
    pub candidate_gen_ns: u64,
    pub scoring_ns: u64,
}
pub struct MoveGenBuffers<'buffers> {
    pub forcing_bits: &'buffers mut Vec<u64>,
    pub scored_moves: &'buffers mut Vec<(Coord, f32)>,
    pub out_moves: &'buffers mut Vec<Coord>,
    pub candidate_moves: Option<&'buffers [u64]>,
    pub proximity_scores: Option<&'buffers [f32]>,
}
fn record_duration_ns<F: FnOnce()>(field: &mut u64, operation: F) {
    let start = Instant::now();
    operation();
    *field = duration_to_ns(start.elapsed());
}
fn record_duration_add_ns<F: FnOnce()>(field: &mut u64, operation: F) {
    let start = Instant::now();
    operation();
    *field = checked::add_u64(
        *field,
        duration_to_ns(start.elapsed()),
        "game_state::record_duration_add_ns",
    );
}
pub(crate) struct GomokuRules;
#[derive(Clone)]
pub struct GomokuPosition {
    pub board: Vec<u8>,
    pub bitboard: Bitboard,
    pub board_size: usize,
    pub win_len: usize,
    pub hasher: Arc<ZobristHasher>,
    pub hash: u64,
    pub threat_index: ThreatIndex,
}
#[derive(Clone)]
pub struct GomokuEvaluator {
    pub(crate) config: EvaluationWeights,
    pub(crate) proximity_kernel: Vec<(usize, usize, f32)>,
    pub(crate) positional_bonus: Vec<f32>,
}
#[derive(Clone)]
pub(crate) struct GomokuMoveCache {
    pub(crate) candidate_moves: SmallVec<[u64; 8]>,
    pub(crate) candidate_move_history: MoveHistory,
}
#[derive(Clone)]
pub struct GameState {
    pub(crate) position: GomokuPosition,
    pub(crate) evaluator: GomokuEvaluator,
    pub(crate) move_cache: GomokuMoveCache,
}

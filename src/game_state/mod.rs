use std::{sync::Arc, time::Instant};

use smallvec::SmallVec;

use crate::config::EvaluationConfig;
use crate::utils::duration_to_ns;
mod bitboard;
mod evaluation;
mod moves;
mod state;
mod threat_index;
pub use bitboard::{Bitboard, BitboardWorkspace};
pub use threat_index::ThreatIndex;
pub use state::ZobristHasher;
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
pub struct MoveGenBuffers<'a> {
    pub score_buffer: &'a mut Vec<f32>,
    pub forcing_bits: &'a mut Vec<u64>,
    pub scored_moves: &'a mut Vec<(Coord, f32)>,
    pub out_moves: &'a mut Vec<Coord>,
    pub proximity_scores: Option<&'a [f32]>,
}
fn record_duration_ns<F: FnOnce()>(field: &mut u64, f: F) {
    let start = Instant::now();
    f();
    *field = duration_to_ns(start.elapsed());
}

fn record_duration_add_ns<F: FnOnce()>(field: &mut u64, f: F) {
    let start = Instant::now();
    f();
    *field = field.saturating_add(duration_to_ns(start.elapsed()));
}
pub(crate) struct GomokuRules;
impl GomokuRules {
    fn sort_scored_moves(scored_moves: &mut [(Coord, f32)]) {
        scored_moves.sort_unstable_by(|a, b| b.1.total_cmp(&a.1));
    }

    fn fill_moves_from_scored(moves: &mut Vec<Coord>, scored_moves: &[(Coord, f32)]) {
        moves.clear();
        moves.extend(scored_moves.iter().map(|(coord, _)| *coord));
    }

    fn score_and_sort_moves_in_place(
        evaluator: &GomokuEvaluator,
        position: &GomokuPosition,
        player: u8,
        moves: &mut Vec<Coord>,
        score_buffer: &mut Vec<f32>,
        scored_moves: &mut Vec<(Coord, f32)>,
    ) {
        evaluator.score_moves_into(position, player, moves, score_buffer, scored_moves);
        Self::sort_scored_moves(scored_moves);
        Self::fill_moves_from_scored(moves, scored_moves);
    }

    fn score_and_sort_moves_in_place_with_proximity(
        evaluator: &GomokuEvaluator,
        position: &GomokuPosition,
        player: u8,
        moves: &mut Vec<Coord>,
        proximity_scores: &[f32],
        score_buffer: &mut Vec<f32>,
        scored_moves: &mut Vec<(Coord, f32)>,
    ) {
        evaluator.score_moves_into_with_proximity(
            position,
            player,
            moves,
            proximity_scores,
            score_buffer,
            scored_moves,
        );
        Self::sort_scored_moves(scored_moves);
        Self::fill_moves_from_scored(moves, scored_moves);
    }
}
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
    pub config: EvaluationConfig,
    pub(crate) proximity_kernel: Vec<Vec<f32>>,
    pub(crate) positional_bonus: Vec<f32>,
}
#[derive(Clone)]
pub(crate) struct GomokuMoveCache {
    pub candidate_moves: SmallVec<[u64; 8]>,
    pub(crate) candidate_move_history: MoveHistory,
}
#[derive(Clone)]
pub struct GomokuGameState {
    pub position: GomokuPosition,
    pub evaluator: GomokuEvaluator,
    pub(crate) move_cache: GomokuMoveCache,
}

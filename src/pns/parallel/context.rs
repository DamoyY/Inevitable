use super::node::NodeRef;
use crate::game_state::{BitboardWorkspace, GomokuGameState, MoveApplyTiming};
pub struct PathEntry {
    pub node: NodeRef,
    pub mov: (usize, usize),
    pub player: u8,
    pub virtual_pn_added: u64,
    pub virtual_dn_added: u64,
}
pub struct ThreadLocalContext {
    pub game_state: GomokuGameState,
    pub path_stack: Vec<PathEntry>,
    pub thread_id: usize,
    pub bitboard_workspace: BitboardWorkspace,
    pub score_buffer: Vec<f32>,
    pub legal_moves: Vec<(usize, usize)>,
    pub scored_moves: Vec<((usize, usize), f32)>,
    pub forcing_bits: Vec<u64>,
}
impl ThreadLocalContext {
    pub fn new(game_state: GomokuGameState, thread_id: usize) -> Self {
        let num_words = game_state.bitboard.num_words();
        let board_cells = game_state.board_size.saturating_mul(game_state.board_size);
        Self {
            game_state,
            path_stack: Vec::with_capacity(256),
            thread_id,
            bitboard_workspace: BitboardWorkspace::new(num_words),
            score_buffer: vec![0.0; board_cells],
            legal_moves: Vec::with_capacity(256),
            scored_moves: Vec::with_capacity(256),
            forcing_bits: vec![0u64; num_words],
        }
    }

    pub fn make_move(&mut self, mov: (usize, usize), player: u8) {
        self.game_state.make_move(mov, player);
    }

    pub fn make_move_with_timing(&mut self, mov: (usize, usize), player: u8) -> MoveApplyTiming {
        self.game_state.make_move_with_timing(mov, player)
    }

    pub fn undo_move(&mut self, mov: (usize, usize)) {
        self.game_state.undo_move(mov);
    }

    pub fn push_path(
        &mut self,
        node: NodeRef,
        mov: (usize, usize),
        player: u8,
        vpn: u64,
        vdn: u64,
    ) {
        self.path_stack.push(PathEntry {
            node,
            mov,
            player,
            virtual_pn_added: vpn,
            virtual_dn_added: vdn,
        });
    }

    pub fn pop_path(&mut self) -> Option<PathEntry> {
        self.path_stack.pop()
    }

    pub fn clear_path(&mut self) {
        self.path_stack.clear();
    }

    pub fn check_win(&self, player: u8) -> bool {
        self.game_state.check_win(player)
    }

    pub fn get_canonical_hash(&self) -> u64 {
        self.game_state.get_canonical_hash()
    }

    pub const fn get_hash(&self) -> u64 {
        self.game_state.get_hash()
    }

    pub fn refresh_legal_moves(&mut self, player: u8) {
        self.game_state.get_legal_moves_into(
            player,
            &mut self.bitboard_workspace,
            &mut self.score_buffer,
            &mut self.forcing_bits,
            &mut self.scored_moves,
            &mut self.legal_moves,
        );
    }
}

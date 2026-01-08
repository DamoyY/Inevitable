use super::node::NodeRef;
use crate::game_state::GomokuGameState;

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
}

impl ThreadLocalContext {
    pub fn new(game_state: GomokuGameState, thread_id: usize) -> Self {
        Self {
            game_state,
            path_stack: Vec::with_capacity(256),
            thread_id,
        }
    }

    pub fn make_move(&mut self, mov: (usize, usize), player: u8) {
        self.game_state.make_move(mov, player);
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

    pub fn get_hash(&self) -> u64 {
        self.game_state.get_hash()
    }

    pub fn get_legal_moves(&self, player: u8) -> Vec<(usize, usize)> {
        self.game_state.get_legal_moves(player)
    }
}

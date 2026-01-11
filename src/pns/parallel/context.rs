use std::collections::VecDeque;

use hashbrown::HashMap;

use super::node::NodeRef;
use crate::game_state::{
    BitboardWorkspace, GomokuGameState, MoveApplyTiming, MoveGenBuffers, MoveGenTiming,
};
const NODE_CACHE_CAPACITY: usize = 1024;
type NodeKey = (u64, usize);
struct LocalNodeCache {
    capacity: usize,
    entries: HashMap<NodeKey, NodeRef>,
    order: VecDeque<NodeKey>,
}
impl LocalNodeCache {
    fn new(capacity: usize) -> Self {
        Self {
            capacity,
            entries: HashMap::with_capacity(capacity),
            order: VecDeque::with_capacity(capacity),
        }
    }

    fn get(&mut self, key: &NodeKey) -> Option<NodeRef> {
        let node = self.entries.get(key).cloned()?;
        self.touch(key);
        Some(node)
    }

    fn insert(&mut self, key: NodeKey, node: NodeRef) {
        if self.capacity == 0 {
            return;
        }
        if self.entries.contains_key(&key) {
            self.entries.insert(key, node);
            self.touch(&key);
            return;
        }
        if self.entries.len() >= self.capacity
            && let Some(old_key) = self.order.pop_front()
        {
            self.entries.remove(&old_key);
        }
        self.order.push_back(key);
        self.entries.insert(key, node);
    }

    fn touch(&mut self, key: &NodeKey) {
        if let Some(pos) = self.order.iter().position(|item| item == key) {
            self.order.remove(pos);
        }
        self.order.push_back(*key);
    }
}
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
    pub current_proximity_scores: Vec<f32>,
    pub legal_moves: Vec<(usize, usize)>,
    pub scored_moves: Vec<((usize, usize), f32)>,
    pub forcing_bits: Vec<u64>,
    node_cache: LocalNodeCache,
}
impl ThreadLocalContext {
    pub fn new(game_state: GomokuGameState, thread_id: usize) -> Self {
        let num_words = game_state.bitboard.num_words();
        let board_cells = game_state.board_size.saturating_mul(game_state.board_size);
        let mut current_proximity_scores = vec![0.0f32; board_cells.saturating_mul(2)];
        let (player_one_scores, player_two_scores) =
            current_proximity_scores.split_at_mut(board_cells);
        game_state.rebuild_proximity_scores(1, player_one_scores);
        game_state.rebuild_proximity_scores(2, player_two_scores);
        Self {
            game_state,
            path_stack: Vec::with_capacity(256),
            thread_id,
            bitboard_workspace: BitboardWorkspace::new(num_words),
            score_buffer: vec![0.0; board_cells],
            current_proximity_scores,
            legal_moves: Vec::with_capacity(256),
            scored_moves: Vec::with_capacity(256),
            forcing_bits: vec![0u64; num_words],
            node_cache: LocalNodeCache::new(NODE_CACHE_CAPACITY),
        }
    }

    pub fn make_move(&mut self, mov: (usize, usize), player: u8) {
        self.game_state.make_move(mov, player);
        let board_cells = self
            .game_state
            .board_size
            .saturating_mul(self.game_state.board_size);
        let game_state = &self.game_state;
        if let Some(scores) =
            proximity_scores_for_player_mut(&mut self.current_proximity_scores, board_cells, player)
        {
            game_state.apply_proximity_delta(mov, 1.0, scores);
        }
    }

    pub fn make_move_with_timing(&mut self, mov: (usize, usize), player: u8) -> MoveApplyTiming {
        let timing = self.game_state.make_move_with_timing(mov, player);
        let board_cells = self
            .game_state
            .board_size
            .saturating_mul(self.game_state.board_size);
        let game_state = &self.game_state;
        if let Some(scores) =
            proximity_scores_for_player_mut(&mut self.current_proximity_scores, board_cells, player)
        {
            game_state.apply_proximity_delta(mov, 1.0, scores);
        }
        timing
    }

    pub fn undo_move(&mut self, mov: (usize, usize), player: u8) {
        let board_cells = self
            .game_state
            .board_size
            .saturating_mul(self.game_state.board_size);
        let game_state = &self.game_state;
        if let Some(scores) =
            proximity_scores_for_player_mut(&mut self.current_proximity_scores, board_cells, player)
        {
            game_state.apply_proximity_delta(mov, -1.0, scores);
        }
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

    pub fn refresh_legal_moves(&mut self, player: u8) -> MoveGenTiming {
        let board_cells = self
            .game_state
            .board_size
            .saturating_mul(self.game_state.board_size);
        let proximity_scores =
            proximity_scores_for_player(&self.current_proximity_scores, board_cells, player);
        let mut buffers = MoveGenBuffers {
            score_buffer: &mut self.score_buffer,
            forcing_bits: &mut self.forcing_bits,
            scored_moves: &mut self.scored_moves,
            out_moves: &mut self.legal_moves,
            proximity_scores: Some(proximity_scores),
        };
        self.game_state
            .get_legal_moves_into(player, &mut self.bitboard_workspace, &mut buffers)
    }

    pub fn get_cached_node(&mut self, key: &(u64, usize)) -> Option<NodeRef> {
        self.node_cache.get(key)
    }

    pub fn cache_node(&mut self, key: (u64, usize), node: NodeRef) {
        self.node_cache.insert(key, node);
    }
}
fn proximity_scores_for_player(scores: &[f32], board_cells: usize, player: u8) -> &[f32] {
    let total_cells = board_cells.saturating_mul(2);
    match player {
        1 => scores.get(0..board_cells).unwrap_or(&[]),
        2 => scores.get(board_cells..total_cells).unwrap_or(&[]),
        _ => &[],
    }
}
fn proximity_scores_for_player_mut(
    scores: &mut [f32],
    board_cells: usize,
    player: u8,
) -> Option<&mut [f32]> {
    let total_cells = board_cells.saturating_mul(2);
    match player {
        1 => scores.get_mut(0..board_cells),
        2 => scores.get_mut(board_cells..total_cells),
        _ => None,
    }
}

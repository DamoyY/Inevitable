use super::ParallelSolver;
pub(super) fn root_pn(solver: &ParallelSolver) -> u64 {
    solver.tree.root.get_pn()
}
pub(super) fn root_dn(solver: &ParallelSolver) -> u64 {
    solver.tree.root.get_dn()
}
pub(super) fn root_player(solver: &ParallelSolver) -> u8 {
    solver.tree.root.player
}
pub(super) fn root_win_len(solver: &ParallelSolver) -> u64 {
    solver.tree.root.get_win_len()
}
pub(super) const fn game_state(solver: &ParallelSolver) -> &crate::game_state::GameState {
    &solver.base_game_state
}
pub(super) const fn board_size(solver: &ParallelSolver) -> usize {
    solver.board_size
}
pub(super) const fn win_len(solver: &ParallelSolver) -> usize {
    solver.win_len
}

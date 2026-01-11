use std::time::Duration;
#[inline]
pub const fn board_index(board_size: usize, r: usize, c: usize) -> usize {
    r * board_size + c
}
#[inline]
pub fn duration_to_ns(duration: Duration) -> u64 {
    u64::try_from(duration.as_nanos()).unwrap_or(u64::MAX)
}

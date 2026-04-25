use super::{BitboardWorkspace, ScratchPads};
impl BitboardWorkspace {
    #[inline]
    #[must_use]
    pub fn new(num_words: usize) -> Self {
        let scratch_pad = core::array::from_fn(|_| vec![0; num_words]);
        Self { scratch_pad }
    }
    #[inline]
    pub(in crate::game_state) const fn pads_mut(&mut self) -> ScratchPads<'_> {
        self.scratch_pad.each_mut()
    }
}

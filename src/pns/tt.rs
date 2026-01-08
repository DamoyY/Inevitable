#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct TTEntry {
    pub pn: u64,
    pub dn: u64,
    pub win_len: u64,
}

pub struct PNSNode {
    pub player: u8,
    pub parent: Option<usize>,
    pub mov: Option<(usize, usize)>,
    pub children: Vec<usize>,
    pub pn: u64,
    pub dn: u64,
    pub is_expanded: bool,
    pub win_len: u64,
    pub depth: usize,
    pub is_depth_limited: bool,
    pub hash: u64,
}
impl PNSNode {
    pub fn new(
        player: u8,
        parent: Option<usize>,
        mov: Option<(usize, usize)>,
        depth: usize,
    ) -> Self {
        Self {
            player,
            parent,
            mov,
            children: Vec::new(),
            pn: 1,
            dn: 1,
            is_expanded: false,
            win_len: u64::MAX,
            depth,
            is_depth_limited: false,
            hash: 0,
        }
    }

    pub fn is_or_node(&self) -> bool {
        self.player == 1
    }
}

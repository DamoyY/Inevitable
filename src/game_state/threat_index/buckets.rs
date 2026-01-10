const NONE_INDEX: usize = usize::MAX;

#[derive(Clone, Copy)]
struct Bucket {
    head: usize,
}

#[derive(Clone, Copy)]
struct BucketNode {
    prev: usize,
    next: usize,
    bucket: usize,
}

#[derive(Clone)]
pub(super) struct PatternBuckets {
    win_len: usize,
    window_count: usize,
    buckets: Vec<Bucket>,
    nodes: Vec<BucketNode>,
}

impl PatternBuckets {
    pub(super) const fn empty() -> Self {
        Self {
            win_len: 0,
            window_count: 0,
            buckets: Vec::new(),
            nodes: Vec::new(),
        }
    }

    pub(super) fn new(win_len: usize, window_count: usize) -> Self {
        let bucket_count = 2 * (win_len + 1) * (win_len + 1);
        let buckets = vec![Bucket { head: NONE_INDEX }; bucket_count];
        let nodes = vec![
            BucketNode {
                prev: NONE_INDEX,
                next: NONE_INDEX,
                bucket: NONE_INDEX,
            };
            2 * window_count
        ];
        Self {
            win_len,
            window_count,
            buckets,
            nodes,
        }
    }

    pub(super) fn reset(&mut self) {
        for bucket in &mut self.buckets {
            bucket.head = NONE_INDEX;
        }
        for node in &mut self.nodes {
            node.prev = NONE_INDEX;
            node.next = NONE_INDEX;
            node.bucket = NONE_INDEX;
        }
    }

    const fn bucket_index(&self, player: u8, p_count: usize, o_count: usize) -> usize {
        let player_idx = (player - 1) as usize;
        (player_idx * (self.win_len + 1) + p_count) * (self.win_len + 1) + o_count
    }

    const fn node_index(&self, player: u8, window_idx: usize) -> usize {
        let player_idx = (player - 1) as usize;
        player_idx * self.window_count + window_idx
    }

    pub(super) fn insert(&mut self, player: u8, window_idx: usize, p_count: usize, o_count: usize) {
        let bucket_idx = self.bucket_index(player, p_count, o_count);
        let node_idx = self.node_index(player, window_idx);
        debug_assert_eq!(self.nodes[node_idx].bucket, NONE_INDEX);
        self.nodes[node_idx].bucket = bucket_idx;
        self.nodes[node_idx].prev = NONE_INDEX;
        self.nodes[node_idx].next = self.buckets[bucket_idx].head;
        if self.buckets[bucket_idx].head != NONE_INDEX {
            self.nodes[self.buckets[bucket_idx].head].prev = node_idx;
        }
        self.buckets[bucket_idx].head = node_idx;
    }

    pub(super) fn remove(&mut self, player: u8, window_idx: usize) {
        let node_idx = self.node_index(player, window_idx);
        let bucket_idx = self.nodes[node_idx].bucket;
        if bucket_idx == NONE_INDEX {
            return;
        }
        let prev = self.nodes[node_idx].prev;
        let next = self.nodes[node_idx].next;
        if prev == NONE_INDEX {
            self.buckets[bucket_idx].head = next;
        } else {
            self.nodes[prev].next = next;
        }
        if next != NONE_INDEX {
            self.nodes[next].prev = prev;
        }
        self.nodes[node_idx].prev = NONE_INDEX;
        self.nodes[node_idx].next = NONE_INDEX;
        self.nodes[node_idx].bucket = NONE_INDEX;
    }

    pub(super) fn iter(&self, player: u8, p_count: usize, o_count: usize) -> PatternBucketIter<'_> {
        let bucket_idx = self.bucket_index(player, p_count, o_count);
        PatternBucketIter {
            current: self.buckets[bucket_idx].head,
            nodes: &self.nodes,
            window_count: self.window_count,
        }
    }
}

pub(super) struct PatternBucketIter<'a> {
    current: usize,
    nodes: &'a [BucketNode],
    window_count: usize,
}

impl Iterator for PatternBucketIter<'_> {
    type Item = usize;

    fn next(&mut self) -> Option<Self::Item> {
        if self.current == NONE_INDEX {
            return None;
        }
        let node_idx = self.current;
        self.current = self.nodes[node_idx].next;
        Some(node_idx % self.window_count)
    }
}

use crate::checked;
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
        let win_len_plus_one =
            checked::add_usize(win_len, 1_usize, "PatternBuckets::new::win_len_plus_one");
        let per_player_bucket_count = checked::mul_usize(
            win_len_plus_one,
            win_len_plus_one,
            "PatternBuckets::new::per_player_bucket_count",
        );
        let bucket_count = checked::mul_usize(
            2_usize,
            per_player_bucket_count,
            "PatternBuckets::new::bucket_count",
        );
        let node_count =
            checked::mul_usize(2_usize, window_count, "PatternBuckets::new::node_count");
        let buckets = vec![Bucket { head: NONE_INDEX }; bucket_count];
        let nodes = vec![
            BucketNode {
                prev: NONE_INDEX,
                next: NONE_INDEX,
                bucket: NONE_INDEX,
            };
            node_count
        ];
        Self {
            win_len,
            window_count,
            buckets,
            nodes,
        }
    }
    fn bucket(&self, bucket_index: usize, context: &str) -> &Bucket {
        let Some(bucket) = self.buckets.get(bucket_index) else {
            eprintln!("{context} 桶索引越界: {bucket_index}");
            panic!("{context} 桶索引越界");
        };
        bucket
    }
    fn bucket_mut(&mut self, bucket_index: usize, context: &str) -> &mut Bucket {
        let Some(bucket) = self.buckets.get_mut(bucket_index) else {
            eprintln!("{context} 桶索引越界: {bucket_index}");
            panic!("{context} 桶索引越界");
        };
        bucket
    }
    fn node(&self, node_index: usize, context: &str) -> &BucketNode {
        let Some(node) = self.nodes.get(node_index) else {
            eprintln!("{context} 节点索引越界: {node_index}");
            panic!("{context} 节点索引越界");
        };
        node
    }
    fn node_mut(&mut self, node_index: usize, context: &str) -> &mut BucketNode {
        let Some(node) = self.nodes.get_mut(node_index) else {
            eprintln!("{context} 节点索引越界: {node_index}");
            panic!("{context} 节点索引越界");
        };
        node
    }
    fn player_index(player: u8, context: &str) -> usize {
        match player {
            1 => 0,
            2 => 1,
            _ => {
                eprintln!("{context} 收到非法玩家编号: {player}");
                panic!("{context} 收到非法玩家编号");
            }
        }
    }
    fn bucket_index(&self, player: u8, player_count: usize, opponent_count: usize) -> usize {
        let player_index = Self::player_index(player, "PatternBuckets::bucket_index");
        let win_len_plus_one = checked::add_usize(
            self.win_len,
            1_usize,
            "PatternBuckets::bucket_index::win_len_plus_one",
        );
        let player_offset = checked::mul_usize(
            player_index,
            win_len_plus_one,
            "PatternBuckets::bucket_index::player_offset",
        );
        let row_index = checked::add_usize(
            player_offset,
            player_count,
            "PatternBuckets::bucket_index::row_index",
        );
        let bucket_base = checked::mul_usize(
            row_index,
            win_len_plus_one,
            "PatternBuckets::bucket_index::bucket_base",
        );
        checked::add_usize(
            bucket_base,
            opponent_count,
            "PatternBuckets::bucket_index::bucket_index",
        )
    }
    fn node_index(&self, player: u8, window_index: usize) -> usize {
        let player_index = Self::player_index(player, "PatternBuckets::node_index");
        let player_offset = checked::mul_usize(
            player_index,
            self.window_count,
            "PatternBuckets::node_index::player_offset",
        );
        checked::add_usize(
            player_offset,
            window_index,
            "PatternBuckets::node_index::node_index",
        )
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
    pub(super) fn insert(
        &mut self,
        player: u8,
        window_index: usize,
        player_count: usize,
        opponent_count: usize,
    ) {
        let bucket_index = self.bucket_index(player, player_count, opponent_count);
        let node_index = self.node_index(player, window_index);
        debug_assert_eq!(
            self.node(node_index, "PatternBuckets::insert::existing_node")
                .bucket,
            NONE_INDEX,
            "PatternBuckets::insert 节点已存在于桶中"
        );
        let next_node_index = self
            .bucket(bucket_index, "PatternBuckets::insert::bucket")
            .head;
        {
            let node = self.node_mut(node_index, "PatternBuckets::insert::node");
            node.bucket = bucket_index;
            node.prev = NONE_INDEX;
            node.next = next_node_index;
        }
        if next_node_index != NONE_INDEX {
            self.node_mut(next_node_index, "PatternBuckets::insert::next_node")
                .prev = node_index;
        }
        self.bucket_mut(bucket_index, "PatternBuckets::insert::bucket_mut")
            .head = node_index;
    }
    pub(super) fn remove(&mut self, player: u8, window_index: usize) {
        let node_index = self.node_index(player, window_index);
        let node = self.node(node_index, "PatternBuckets::remove::node");
        let bucket_index = node.bucket;
        if bucket_index == NONE_INDEX {
            return;
        }
        let previous_node_index = node.prev;
        let next_node_index = node.next;
        if previous_node_index == NONE_INDEX {
            self.bucket_mut(bucket_index, "PatternBuckets::remove::bucket")
                .head = next_node_index;
        } else {
            self.node_mut(previous_node_index, "PatternBuckets::remove::previous_node")
                .next = next_node_index;
        }
        if next_node_index != NONE_INDEX {
            self.node_mut(next_node_index, "PatternBuckets::remove::next_node")
                .prev = previous_node_index;
        }
        let node_to_clear = self.node_mut(node_index, "PatternBuckets::remove::node_mut");
        node_to_clear.prev = NONE_INDEX;
        node_to_clear.next = NONE_INDEX;
        node_to_clear.bucket = NONE_INDEX;
    }
    pub(super) fn iter(
        &self,
        player: u8,
        player_count: usize,
        opponent_count: usize,
    ) -> impl Iterator<Item = usize> + '_ {
        let bucket_index = self.bucket_index(player, player_count, opponent_count);
        let mut current_node_index = self
            .bucket(bucket_index, "PatternBuckets::iter::bucket")
            .head;
        let nodes = &self.nodes;
        let window_count = self.window_count;
        core::iter::from_fn(move || {
            if current_node_index == NONE_INDEX {
                return None;
            }
            let Some(node) = nodes.get(current_node_index) else {
                eprintln!("PatternBuckets::iter 节点索引越界: {current_node_index}");
                panic!("PatternBuckets::iter 节点索引越界");
            };
            let node_index = current_node_index;
            current_node_index = node.next;
            let window_index = if node_index < window_count {
                node_index
            } else {
                checked::sub_usize(
                    node_index,
                    window_count,
                    "PatternBuckets::iter::window_index",
                )
            };
            Some(window_index)
        })
    }
}

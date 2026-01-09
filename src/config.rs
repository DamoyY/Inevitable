use std::{fs, thread};

use serde::Deserialize;
#[derive(Debug, Deserialize)]
pub struct Config {
    pub board_size: usize,
    pub win_len: usize,
    pub verbose: bool,
    pub num_threads: usize,
    pub log_interval_ms: u64,
}
impl Config {
    #[must_use] 
    pub fn load() -> Self {
        let config_str = fs::read_to_string("config.yaml").expect("无法读取 config.yaml");
        let mut config: Self = serde_yaml::from_str(&config_str).expect("解析 config.yaml 失败");
        if config.num_threads == 0 {
            config.num_threads = thread::available_parallelism()
                .map(std::num::NonZero::get)
                .unwrap_or(4);
        }
        if config.log_interval_ms == 0 {
            config.log_interval_ms = 1000;
        }
        config
    }
}

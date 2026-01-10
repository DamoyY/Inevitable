use std::{fs, process, thread};

use serde::Deserialize;
#[derive(Debug, Deserialize)]
pub struct Config {
    pub board_size: usize,
    pub win_len: usize,
    pub verbose: bool,
    pub num_threads: usize,
}
impl Config {
    pub fn load() -> Self {
        let config_str = fs::read_to_string("config.yaml").unwrap_or_else(|err| {
            eprintln!("无法读取 config.yaml: {err}");
            process::exit(1);
        });
        let mut config: Self = serde_yaml::from_str(&config_str).unwrap_or_else(|err| {
            eprintln!("解析 config.yaml 失败: {err}");
            process::exit(1);
        });
        if config.num_threads == 0 {
            config.num_threads = thread::available_parallelism()
                .map(std::num::NonZero::get)
                .unwrap_or(4);
        }
        config
    }
}

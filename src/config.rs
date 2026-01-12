use std::{fs, process, thread};

use serde::Deserialize;
#[derive(Debug, Deserialize, Clone, Copy)]
pub struct EvaluationConfig {
    pub proximity_kernel_size: usize,
    pub proximity_scale: f32,
    pub positional_bonus_scale: f32,
    pub score_win: f32,
    pub score_live_four: f32,
    pub score_blocked_four: f32,
    pub score_live_three: f32,
    pub score_live_two: f32,
    pub score_block_win: f32,
    pub score_block_live_four: f32,
    pub score_block_blocked_four: f32,
    pub score_block_live_three: f32,
}
#[derive(Debug, Deserialize)]
pub struct Config {
    pub board_size: usize,
    pub win_len: usize,
    pub verbose: bool,
    pub num_threads: usize,
    pub evaluation: EvaluationConfig,
    #[serde(default = "default_min_available_memory_mb")]
    pub min_available_memory_mb: u64,
    #[serde(default = "default_memory_check_interval_ms")]
    pub memory_check_interval_ms: u64,
}

const fn default_min_available_memory_mb() -> u64 {
    1024
}

const fn default_memory_check_interval_ms() -> u64 {
    500
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

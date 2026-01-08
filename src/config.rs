use std::fs;

use serde::Deserialize;
#[derive(Debug, Deserialize)]
pub struct Config {
    pub board_size: usize,
    pub win_len: usize,
    pub verbose: bool,
    #[serde(default)]
    pub num_threads: Option<usize>,
}
impl Config {
    pub fn load() -> Self {
        let config_str = fs::read_to_string("config.yaml").expect("无法读取 config.yaml");
        serde_yaml::from_str(&config_str).expect("解析 config.yaml 失败")
    }
}

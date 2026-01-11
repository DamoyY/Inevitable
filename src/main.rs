use inevitable::alloc_stats::TrackingAllocator;
#[global_allocator]
static GLOBAL: TrackingAllocator = TrackingAllocator::new();
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::Duration,
};

use inevitable::{config::Config, ui, utils::available_memory_bytes};

fn spawn_memory_watchdog(exit_flag: Arc<AtomicBool>, config: &Config) {
    let min_available_memory_mb = config.min_available_memory_mb;
    let min_available_memory_bytes = min_available_memory_mb.saturating_mul(1024 * 1024);
    let poll_interval = Duration::from_millis(config.memory_check_interval_ms.max(1));
    thread::spawn(move || {
        loop {
            if exit_flag.load(Ordering::SeqCst) {
                return;
            }
            if let Some(available) = available_memory_bytes()
                && available < min_available_memory_bytes
            {
                eprintln!("剩余内存不足 {min_available_memory_mb}MB，程序将退出。");
                exit_flag.store(true, Ordering::SeqCst);
                std::process::exit(1);
            }
            thread::sleep(poll_interval);
        }
    });
}

fn main() {
    let config = Config::load();
    let exit_flag = Arc::new(AtomicBool::new(false));
    let flag = exit_flag.clone();
    ctrlc::set_handler(move || {
        flag.store(true, Ordering::SeqCst);
        println!("\n收到 Ctrl+C，正在退出...");
    })
    .expect("无法设置 Ctrl+C 处理程序");
    spawn_memory_watchdog(exit_flag.clone(), &config);
    ui::play_game(&exit_flag, &config);
}

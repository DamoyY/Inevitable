use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use five_stone::ui;
fn main() {
    let exit_flag = Arc::new(AtomicBool::new(false));
    let flag = exit_flag.clone();
    ctrlc::set_handler(move || {
        flag.store(true, Ordering::SeqCst);
        println!("\n收到 Ctrl+C，正在退出...");
    })
    .expect("无法设置 Ctrl+C 处理程序");
    ui::play_game(&exit_flag);
}

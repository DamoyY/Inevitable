use std::{
    io::{self, Write},
    sync::{atomic::AtomicBool, mpsc},
    thread,
    time::Duration,
};

use crate::utils::board_index;
pub(super) fn read_player_move(
    board: &[u8],
    board_size: usize,
    exit_flag: &AtomicBool,
) -> Option<(usize, usize)> {
    loop {
        if exit_flag.load(std::sync::atomic::Ordering::SeqCst) {
            return None;
        }
        print!("请输入您的落子位置 (行 列)，例如 '3 4': ");
        io::stdout().flush().unwrap();
        let input = match read_line_with_exit(exit_flag) {
            Ok(line) => line,
            Err(InputError::Exit) => return None,
            Err(InputError::Io) => {
                println!("读取输入失败。");
                continue;
            }
        };
        let parts: Vec<&str> = input.split_whitespace().collect();
        if parts.len() != 2 {
            println!("输入格式错误，请输入两个数字。");
            continue;
        }
        let row: Result<usize, _> = parts[0].parse();
        let col: Result<usize, _> = parts[1].parse();
        match (row, col) {
            (Ok(r), Ok(c)) => {
                if r >= board_size || c >= board_size {
                    println!("坐标超出范围。");
                    continue;
                }
                if board[board_index(board_size, r, c)] != 0 {
                    println!("该位置已有棋子。");
                    continue;
                }
                return Some((r, c));
            }
            _ => {
                println!("输入无效。");
            }
        }
    }
}
enum InputError {
    Exit,
    Io,
}
fn read_line_with_exit(exit_flag: &AtomicBool) -> Result<String, InputError> {
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let mut input = String::new();
        let result = io::stdin().read_line(&mut input).map(|_| input);
        let _ = tx.send(result);
    });
    loop {
        if exit_flag.load(std::sync::atomic::Ordering::SeqCst) {
            return Err(InputError::Exit);
        }
        match rx.recv_timeout(Duration::from_millis(100)) {
            Ok(Ok(line)) => return Ok(line),
            Ok(Err(_)) | Err(mpsc::RecvTimeoutError::Disconnected) => {
                return Err(InputError::Io);
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
        }
    }
}

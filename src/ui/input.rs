use crate::utils::board_index;
use core::{
    sync::atomic::{AtomicBool, Ordering},
    time::Duration,
};
use std::{io, sync::mpsc, thread};
pub(super) fn read_player_move(
    board: &[u8],
    board_size: usize,
    exit_flag: &AtomicBool,
) -> Option<(usize, usize)> {
    loop {
        if exit_flag.load(Ordering::SeqCst) {
            return None;
        }
        print!("请输入您的落子位置 (行 列)，例如 '3 4': ");
        let mut stdout = io::stdout();
        if let Err(err) = io::Write::flush(&mut stdout) {
            eprintln!("刷新标准输出失败: {err}");
            return None;
        }
        let input = match read_line_with_exit(exit_flag) {
            Ok(line) => line,
            Err(InputError::Exit) => return None,
            Err(InputError::Io) => {
                println!("读取输入失败。");
                continue;
            }
        };
        let mut parts = input.split_whitespace();
        let Some(row_text) = parts.next() else {
            println!("输入格式错误，请输入两个数字。");
            continue;
        };
        let Some(column_text) = parts.next() else {
            println!("输入格式错误，请输入两个数字。");
            continue;
        };
        if parts.next().is_some() {
            println!("输入格式错误，请输入两个数字。");
            continue;
        }
        let row = row_text.parse::<usize>();
        let column = column_text.parse::<usize>();
        match (row, column) {
            (Ok(row_index), Ok(column_index)) => {
                if row_index >= board_size || column_index >= board_size {
                    println!("坐标超出范围。");
                    continue;
                }
                let board_position = board_index(board_size, row_index, column_index);
                let Some(cell) = board.get(board_position) else {
                    eprintln!("棋盘数据长度不足，无法读取位置 ({row_index}, {column_index})。");
                    return None;
                };
                if *cell != 0 {
                    println!("该位置已有棋子。");
                    continue;
                }
                return Some((row_index, column_index));
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
        if let Err(err) = tx.send(result) {
            eprintln!("发送输入结果失败: {err}");
        }
    });
    loop {
        if exit_flag.load(core::sync::atomic::Ordering::SeqCst) {
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

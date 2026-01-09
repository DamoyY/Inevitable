use std::{
    io::{self, Write},
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread,
    time::Duration,
};

use crate::{
    config::Config,
    pns::{ParallelSolver, TranspositionTable},
};

pub fn print_board(board: &[Vec<u8>]) {
    let board_size = board.len();
    print!("  ");
    for i in 0..board_size {
        print!("{i:2} ");
    }
    println!();
    for (i, row) in board.iter().enumerate() {
        print!("{i:2} ");
        for &cell in row {
            let c = match cell {
                1 => "X",
                2 => "O",
                _ => ".",
            };
            print!("{c}  ");
        }
        println!();
    }
}
pub fn play_game(exit_flag: &AtomicBool) {
    let config = Config::load();
    print_intro(&config);
    let board_size = config.board_size;
    let mut board = vec![vec![0u8; board_size]; board_size];
    let mut current_player = 1u8;
    let mut tt: Option<TranspositionTable> = None;
    loop {
        if exit_flag.load(Ordering::SeqCst) {
            return;
        }
        let has_stones = board.iter().any(|row| row.iter().any(|&cell| cell != 0));
        if has_stones {
            println!("\n当前棋盘:");
            print_board(&board);
        }
        if current_player == 1 {
            if ai_turn(&mut board, &config, !has_stones, &mut tt) {
                break;
            }
            if exit_flag.load(Ordering::SeqCst) {
                return;
            }
            current_player = 2;
        } else {
            if player_turn(&mut board, board_size, exit_flag) {
                return;
            }
            current_player = 1;
        }
    }
}

fn print_intro(config: &Config) {
    println!(
        "棋盘大小: {size}x{size}, 获胜条件: {win_len}子连珠",
        size = config.board_size,
        win_len = config.win_len
    );
    println!(
        "使用 {threads} 个线程进行并行搜索",
        threads = config.num_threads
    );
    println!("程序执黑 (X)先手，您执白 (O)后手。");
}

fn ai_turn(
    board: &mut [Vec<u8>],
    config: &Config,
    board_empty: bool,
    tt: &mut Option<TranspositionTable>,
) -> bool {
    let board_size = config.board_size;
    let win_len = config.win_len;
    let num_threads = config.num_threads;
    let log_interval_ms = config.log_interval_ms;
    let verbose = config.verbose;
    println!("\n轮到程序 (X) 落子。");
    let mov = if board_empty {
        (board_size / 2, board_size / 2)
    } else {
        println!("程序正在思考...");
        let (best_move, new_tt) = ParallelSolver::find_best_move_with_tt(
            board.to_vec(),
            board_size,
            win_len,
            num_threads,
            log_interval_ms,
            verbose,
            tt.take(),
        );
        *tt = Some(new_tt);
        best_move.unwrap()
    };
    println!("程序选择落子于: {mov:?}");
    board[mov.0][mov.1] = 1;
    if check_win(board, board_size, win_len, num_threads, log_interval_ms, 1) {
        println!("\n最终棋盘:");
        print_board(board);
        println!("程序获胜");
        return true;
    }
    false
}

fn player_turn(board: &mut [Vec<u8>], board_size: usize, exit_flag: &AtomicBool) -> bool {
    println!("\n轮到您 (O) 落子。");
    let Some(mov) = read_player_move(board, board_size, exit_flag) else {
        return true;
    };
    board[mov.0][mov.1] = 2;
    false
}

fn read_player_move(
    board: &[Vec<u8>],
    board_size: usize,
    exit_flag: &AtomicBool,
) -> Option<(usize, usize)> {
    loop {
        if exit_flag.load(Ordering::SeqCst) {
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
                if board[r][c] != 0 {
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
        if exit_flag.load(Ordering::SeqCst) {
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

fn check_win(
    board: &[Vec<u8>],
    board_size: usize,
    win_len: usize,
    num_threads: usize,
    log_interval_ms: u64,
    player: u8,
) -> bool {
    let solver = ParallelSolver::new(
        board.to_vec(),
        board_size,
        win_len,
        Some(1),
        num_threads,
        log_interval_ms,
    );
    solver.game_state().check_win(player)
}

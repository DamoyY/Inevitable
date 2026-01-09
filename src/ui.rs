use std::io::{self, Write};

use crate::{
    config::Config,
    pns::{ParallelSolver, TranspositionTable},
};

pub fn print_board(board: &[Vec<u8>]) {
    let board_size = board.len();
    print!("  ");
    for i in 0..board_size {
        print!("{:2} ", i);
    }
    println!();
    for (i, row) in board.iter().enumerate() {
        print!("{:2} ", i);
        for &cell in row {
            let c = match cell {
                1 => "X",
                2 => "O",
                _ => ".",
            };
            print!("{}  ", c);
        }
        println!();
    }
}

pub fn play_game() {
    let config = Config::load();
    let board_size = config.board_size;
    let win_len = config.win_len;
    let num_threads = config.num_threads;
    let log_interval_ms = config.log_interval_ms;

    println!(
        "棋盘大小: {}x{}, 获胜条件: {}子连珠",
        board_size, board_size, win_len
    );
    println!("使用 {} 个线程进行并行搜索", num_threads);
    println!("程序执黑 (X)先手，您执白 (O)后手。");

    let mut board = vec![vec![0u8; board_size]; board_size];
    let mut current_player = 1u8;
    let mut tt: Option<TranspositionTable> = None;

    loop {
        let has_stones = board.iter().any(|row| row.iter().any(|&cell| cell != 0));
        if has_stones {
            println!("\n当前棋盘:");
            print_board(&board);
        }

        if current_player == 1 {
            println!("\n轮到程序 (X) 落子。");
            let board_empty = !has_stones;
            let mov = if board_empty {
                (board_size / 2, board_size / 2)
            } else {
                println!("程序正在思考...");
                let (best_move, new_tt) = ParallelSolver::find_best_move_with_tt(
                    board.clone(),
                    board_size,
                    win_len,
                    num_threads,
                    log_interval_ms,
                    config.verbose,
                    tt,
                );
                tt = Some(new_tt);
                best_move.unwrap()
            };
            println!("程序选择落子于: {:?}", mov);
            board[mov.0][mov.1] = 1;

            let solver = ParallelSolver::new(
                board.clone(),
                board_size,
                win_len,
                Some(1),
                num_threads,
                log_interval_ms,
            );
            if solver.game_state().check_win(1) {
                println!("\n最终棋盘:");
                print_board(&board);
                println!("程序获胜");
                break;
            }

            current_player = 2;
        } else {
            println!("\n轮到您 (O) 落子。");
            loop {
                print!("请输入您的落子位置 (行 列)，例如 '3 4': ");
                io::stdout().flush().unwrap();
                let mut input = String::new();
                if io::stdin().read_line(&mut input).is_err() {
                    println!("读取输入失败。");
                    continue;
                }
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
                        board[r][c] = 2;

                        let solver = ParallelSolver::new(
                            board.clone(),
                            board_size,
                            win_len,
                            Some(1),
                            num_threads,
                            log_interval_ms,
                        );
                        if solver.game_state().check_win(2) {
                            println!("\n最终棋盘:");
                            print_board(&board);
                            println!("您获胜");
                            return;
                        }

                        current_player = 1;
                        break;
                    }
                    _ => {
                        println!("输入无效。");
                    }
                }
            }
        }
    }
}

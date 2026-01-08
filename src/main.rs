use std::{
    fs,
    io::{self, Write},
};

use five_stone::pns::PNSSolver;
use serde::Deserialize;
#[derive(Debug, Deserialize)]
struct Config {
    board_size: usize,
    win_len: usize,
    initial_depth_limit: usize,
    verbose: bool,
}
fn print_board(board: &[Vec<u8>]) {
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
fn play_game() {
    let config_str = fs::read_to_string("config.yaml").expect("无法读取 config.yaml");
    let config: Config = serde_yaml::from_str(&config_str).expect("解析 config.yaml 失败");
    let board_size = config.board_size;
    let win_len = config.win_len;
    println!(
        "棋盘大小: {}x{}, 获胜条件: {}子连珠",
        board_size, board_size, win_len
    );
    println!("程序执黑 (X)先手，您执白 (O)后手。");
    let initial_board = vec![vec![0u8; board_size]; board_size];
    let mut solver = PNSSolver::new(
        initial_board,
        board_size,
        win_len,
        Some(config.initial_depth_limit),
    );
    loop {
        let board = &solver.game_state.board;
        let has_stones = board.iter().any(|row| row.iter().any(|&cell| cell != 0));
        if has_stones {
            println!("\n当前棋盘:");
            print_board(board);
        }
        if solver.game_state.check_win(1) {
            println!("程序获胜");
            break;
        }
        if solver.root_player() == 1 {
            println!("\n轮到程序 (X) 落子。");
            let board_empty = !has_stones;
            let mov = if board_empty {
                (board_size / 2, board_size / 2)
            } else {
                println!("程序正在思考...");
                solver
                    .find_best_move_iterative_deepening(config.verbose)
                    .unwrap()
            };
            println!("程序选择落子于: {:?}", mov);
            solver.update_root_after_move(mov);
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
                        if solver.game_state.board[r][c] != 0 {
                            println!("该位置已有棋子。");
                            continue;
                        }
                        solver.update_root_after_move((r, c));
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
fn main() {
    play_game();
}

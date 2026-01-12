use std::{
    io::{self, Write},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread,
    time::Duration,
};

use crate::{
    config::Config,
    game_state::{GomokuGameState, ZobristHasher},
    pns::{NodeTable, ParallelSolver, SearchParams, TranspositionTable},
    utils::board_index,
};
const BENCHMARK_BOARD_7X7: [&str; 7] = [
    ".......", ".......", "..O....", "...X...", ".......", ".......", ".......",
];
pub fn print_board(board: &[u8], board_size: usize) {
    print!("  ");
    for i in 0..board_size {
        print!("{i:2} ");
    }
    println!();
    for r in 0..board_size {
        print!("{r:2} ");
        for c in 0..board_size {
            let cell = board[board_index(board_size, r, c)];
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
pub fn run_benchmark(exit_flag: &Arc<AtomicBool>, config: &Config) {
    const BENCHMARK_RUNS: usize = 3;
    if config.board_size != 7 || config.win_len != 5 {
        eprintln!(
            "基准测试固定残局仅支持 7x7 棋盘与 5 连珠规则，当前配置为 {}x{}，胜利长度 {}。",
            config.board_size, config.board_size, config.win_len
        );
        return;
    }
    let board = match benchmark_board(config.board_size) {
        Ok(board) => board,
        Err(err) => {
            eprintln!("{err}");
            return;
        }
    };
    if check_win(&board, config.board_size, config.win_len, 1)
        || check_win(&board, config.board_size, config.win_len, 2)
    {
        eprintln!("基准残局已出现胜负，无法用于基准测试。");
        return;
    }
    println!("开始基准测试：固定残局，计算下一步棋，循环 {BENCHMARK_RUNS} 次。");
    let params = SearchParams::new(config.board_size, config.win_len, config.num_threads);
    let Some(result) =
        ParallelSolver::benchmark_next_move(&board, params, BENCHMARK_RUNS, exit_flag)
    else {
        println!("基准测试已被中断。");
        return;
    };
    println!(
        "基准测试完成，平均耗时 {avg:.6}s，日志已写入 log.csv。",
        avg = result.elapsed_secs
    );
}

fn benchmark_board(board_size: usize) -> Result<Vec<u8>, String> {
    if board_size != BENCHMARK_BOARD_7X7.len() {
        return Err(format!(
            "基准残局仅支持 {}x{} 棋盘。",
            BENCHMARK_BOARD_7X7.len(),
            BENCHMARK_BOARD_7X7.len()
        ));
    }
    let mut board = Vec::with_capacity(board_size.saturating_mul(board_size));
    for (row_idx, row) in BENCHMARK_BOARD_7X7.iter().enumerate() {
        let bytes = row.as_bytes();
        if bytes.len() != board_size {
            return Err(format!("基准残局第 {row_idx} 行长度不匹配。"));
        }
        for &cell in bytes {
            let value = match cell {
                b'.' => 0,
                b'X' => 1,
                b'O' => 2,
                _ => {
                    return Err(format!("基准残局包含非法字符 '{}'。", cell as char));
                }
            };
            board.push(value);
        }
    }
    Ok(board)
}
pub fn play_game(exit_flag: &Arc<AtomicBool>, config: &Config) {
    print_intro(config);
    let board_size = config.board_size;
    let mut board = vec![0u8; board_size.saturating_mul(board_size)];
    let mut current_player = 1u8;
    let mut tt: Option<TranspositionTable> = None;
    let mut node_table: NodeTable = NodeTable::default();
    loop {
        if exit_flag.load(Ordering::SeqCst) {
            return;
        }
        let has_stones = board.iter().any(|&cell| cell != 0);
        if has_stones {
            println!("\n当前棋盘:");
            print_board(&board, board_size);
        }
        if current_player == 1 {
            if ai_turn(
                &mut board,
                config,
                !has_stones,
                &mut tt,
                &mut node_table,
                exit_flag,
            ) {
                break;
            }
            if exit_flag.load(Ordering::SeqCst) {
                return;
            }
            current_player = 2;
        } else {
            if player_turn(&mut board, board_size, exit_flag.as_ref()) {
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
        "使用 {threads} 个线程进行搜索",
        threads = config.num_threads
    );
    println!("程序执黑 [X] 先手，您执白 [O] 后手");
}
fn ai_turn(
    board: &mut [u8],
    config: &Config,
    board_empty: bool,
    tt: &mut Option<TranspositionTable>,
    node_table: &mut NodeTable,
    exit_flag: &Arc<AtomicBool>,
) -> bool {
    if exit_flag.load(Ordering::SeqCst) {
        return true;
    }
    node_table.clear();
    let board_size = config.board_size;
    let win_len = config.win_len;
    let num_threads = config.num_threads;
    let verbose = config.verbose;
    println!("\n轮到程序 (X) 落子。");
    let mov = if board_empty {
        (board_size / 2, board_size / 2)
    } else {
        println!("程序正在思考...");
        let params = SearchParams::new(board_size, win_len, num_threads);
        let (best_move, new_tt, new_node_table) = ParallelSolver::find_best_move_with_tt_and_stop(
            board.to_vec(),
            params,
            verbose,
            exit_flag,
            tt.take(),
            Some(Arc::clone(node_table)),
        );
        *tt = Some(new_tt);
        *node_table = new_node_table;
        if let Some(mov) = best_move {
            mov
        } else {
            println!("搜索已中断。");
            return true;
        }
    };
    if exit_flag.load(Ordering::SeqCst) {
        return true;
    }
    println!("程序选择落子于: {mov:?}");
    board[board_index(board_size, mov.0, mov.1)] = 1;
    if check_win(board, board_size, win_len, 1) {
        println!("\n最终棋盘:");
        print_board(board, board_size);
        println!("程序获胜");
        return true;
    }
    false
}
fn player_turn(board: &mut [u8], board_size: usize, exit_flag: &AtomicBool) -> bool {
    println!("\n轮到您 (O) 落子。");
    let Some(mov) = read_player_move(board, board_size, exit_flag) else {
        return true;
    };
    board[board_index(board_size, mov.0, mov.1)] = 2;
    false
}
fn read_player_move(
    board: &[u8],
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
fn check_win(board: &[u8], board_size: usize, win_len: usize, player: u8) -> bool {
    let hasher = Arc::new(ZobristHasher::new(board_size));
    let game_state = GomokuGameState::new(board.to_vec(), board_size, hasher, 1, win_len);
    game_state.check_win(player)
}

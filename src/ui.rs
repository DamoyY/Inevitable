use crate::{
    checked,
    config::Config,
    game_state::{Coord, GameState, GomokuRules, ZobristHasher},
    pns::{NodeTable, ParallelSolver, SearchParams, TranspositionTable},
    utils::board_index,
};
use alloc::sync::Arc;
use core::sync::atomic::{AtomicBool, Ordering};
mod input;
use input::{PlayerInput, read_player_input};
const PROGRAM_PLAYER: u8 = 1;
const HUMAN_PLAYER: u8 = 2;
const BENCHMARK_BOARD_7X7: [&str; 7] = [
    ".......", ".......", "..O....", "...X...", ".......", ".......", ".......",
];
#[derive(Clone, Copy)]
struct PlayedMove {
    coord: Coord,
    player: u8,
}
enum PlayerTurnResult {
    MoveApplied,
    TakeBack,
    Finished,
}
#[inline]
pub fn print_board(board: &[u8], board_size: usize) {
    print!("  ");
    for column_index in 0..board_size {
        print!("{column_index:2} ");
    }
    println!();
    for row_index in 0..board_size {
        print!("{row_index:2} ");
        for column_index in 0..board_size {
            let Some(cell) = board.get(board_index(board_size, row_index, column_index)) else {
                eprintln!("棋盘数据长度不足，无法打印位置 ({row_index}, {column_index})。");
                return;
            };
            let cell_text = match *cell {
                PROGRAM_PLAYER => "X",
                HUMAN_PLAYER => "O",
                _ => ".",
            };
            print!("{cell_text}  ");
        }
        println!();
    }
}
#[inline]
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
    if check_win(
        &board,
        config.board_size,
        config.win_len,
        config.evaluation,
        PROGRAM_PLAYER,
    ) || check_win(
        &board,
        config.board_size,
        config.win_len,
        config.evaluation,
        HUMAN_PLAYER,
    ) {
        eprintln!("基准残局已出现胜负，无法用于基准测试。");
        return;
    }
    println!("开始基准测试：固定残局，计算下一步棋，循环 {BENCHMARK_RUNS} 次。");
    let params = SearchParams::new(
        config.board_size,
        config.win_len,
        config.num_threads,
        config.evaluation,
    );
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
                    return Err(format!("基准残局包含非法字符 '{}'。", char::from(cell)));
                }
            };
            board.push(value);
        }
    }
    Ok(board)
}
#[inline]
pub fn play_game(exit_flag: &Arc<AtomicBool>, config: &Config) {
    print_intro(config);
    let board_size = config.board_size;
    let mut board = vec![0_u8; board_size.saturating_mul(board_size)];
    let mut current_player = PROGRAM_PLAYER;
    let mut move_history = Vec::new();
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
        if current_player == PROGRAM_PLAYER {
            if ai_turn(
                &mut board,
                config,
                !has_stones,
                &mut tt,
                &mut node_table,
                exit_flag,
                &mut move_history,
            ) {
                break;
            }
            if exit_flag.load(Ordering::SeqCst) {
                return;
            }
            current_player = HUMAN_PLAYER;
        } else {
            match player_turn(
                &mut board,
                board_size,
                exit_flag.as_ref(),
                &mut move_history,
            ) {
                PlayerTurnResult::MoveApplied => {
                    current_player = PROGRAM_PLAYER;
                }
                PlayerTurnResult::TakeBack => {
                    if take_back_last_player_move(&mut board, board_size, &mut move_history) {
                        tt = None;
                        node_table.clear();
                    }
                    current_player = HUMAN_PLAYER;
                }
                PlayerTurnResult::Finished => return,
            }
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
    move_history: &mut Vec<PlayedMove>,
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
    let selected_move = if board_empty {
        let Some(center) = board_size.checked_div(2) else {
            eprintln!("棋盘大小无法计算中心点。");
            return true;
        };
        (center, center)
    } else {
        println!("程序正在思考...");
        let params = SearchParams::new(board_size, win_len, num_threads, config.evaluation);
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
        if let Some(best_move_coord) = best_move {
            best_move_coord
        } else {
            println!("搜索已中断。");
            return true;
        }
    };
    if exit_flag.load(Ordering::SeqCst) {
        return true;
    }
    println!(
        "程序选择落子于: ({row}, {column})",
        row = selected_move.0,
        column = selected_move.1
    );
    let move_index = board_index(board_size, selected_move.0, selected_move.1);
    let Some(cell) = board.get_mut(move_index) else {
        eprintln!(
            "程序落子位置超出棋盘数据范围: ({row}, {column})。",
            row = selected_move.0,
            column = selected_move.1
        );
        return true;
    };
    *cell = PROGRAM_PLAYER;
    move_history.push(PlayedMove {
        coord: selected_move,
        player: PROGRAM_PLAYER,
    });
    if check_win(
        board,
        board_size,
        win_len,
        config.evaluation,
        PROGRAM_PLAYER,
    ) {
        println!("\n最终棋盘:");
        print_board(board, board_size);
        println!("程序获胜");
        return true;
    }
    false
}
fn player_turn(
    board: &mut [u8],
    board_size: usize,
    exit_flag: &AtomicBool,
    move_history: &mut Vec<PlayedMove>,
) -> PlayerTurnResult {
    println!("\n轮到您 (O) 落子。");
    let Some(player_input) = read_player_input(board, board_size, exit_flag) else {
        return PlayerTurnResult::Finished;
    };
    let PlayerInput::Move(player_move) = player_input else {
        return PlayerTurnResult::TakeBack;
    };
    let move_index = board_index(board_size, player_move.0, player_move.1);
    let Some(cell) = board.get_mut(move_index) else {
        eprintln!(
            "玩家落子位置超出棋盘数据范围: ({row}, {column})。",
            row = player_move.0,
            column = player_move.1
        );
        return PlayerTurnResult::Finished;
    };
    *cell = HUMAN_PLAYER;
    move_history.push(PlayedMove {
        coord: player_move,
        player: HUMAN_PLAYER,
    });
    PlayerTurnResult::MoveApplied
}
fn take_back_last_player_move(
    board: &mut [u8],
    board_size: usize,
    move_history: &mut Vec<PlayedMove>,
) -> bool {
    if move_history.is_empty() {
        println!("当前没有可悔棋步。");
        return false;
    }
    if move_history.len() < 2 {
        println!("您尚未落子，无法悔棋。");
        return false;
    }
    let ai_move_index = checked::sub_usize(
        move_history.len(),
        1_usize,
        "take_back_last_player_move::ai_move_index",
    );
    let player_move_index = checked::sub_usize(
        move_history.len(),
        2_usize,
        "take_back_last_player_move::player_move_index",
    );
    let Some(&ai_move) = move_history.get(ai_move_index) else {
        eprintln!("悔棋状态异常：找不到程序上一手落子。");
        return false;
    };
    let Some(&player_move) = move_history.get(player_move_index) else {
        eprintln!("悔棋状态异常：找不到玩家上一手落子。");
        return false;
    };
    if ai_move.player != PROGRAM_PLAYER {
        eprintln!("悔棋状态异常：上一手不是程序落子。");
        return false;
    }
    if player_move.player != HUMAN_PLAYER {
        eprintln!("悔棋状态异常：找不到上一手玩家落子。");
        return false;
    }
    if !recorded_move_matches(board, board_size, ai_move)
        || !recorded_move_matches(board, board_size, player_move)
    {
        return false;
    }
    clear_recorded_move(board, board_size, ai_move);
    clear_recorded_move(board, board_size, player_move);
    move_history.truncate(player_move_index);
    println!("已悔棋，回到您上一手落子前。");
    true
}
fn recorded_move_matches(board: &[u8], board_size: usize, played_move: PlayedMove) -> bool {
    let (row, column) = played_move.coord;
    let move_index = board_index(board_size, row, column);
    let Some(&cell) = board.get(move_index) else {
        eprintln!("悔棋位置超出棋盘数据范围: ({row}, {column})。");
        return false;
    };
    if cell != played_move.player {
        eprintln!("悔棋状态异常：位置 ({row}, {column}) 的棋子与历史记录不一致。");
        return false;
    }
    true
}
fn clear_recorded_move(board: &mut [u8], board_size: usize, played_move: PlayedMove) {
    let (row, column) = played_move.coord;
    let move_index = board_index(board_size, row, column);
    let Some(cell) = board.get_mut(move_index) else {
        eprintln!("悔棋位置超出棋盘数据范围: ({row}, {column})。");
        panic!("悔棋位置超出棋盘数据范围");
    };
    *cell = 0;
}
fn check_win(
    board: &[u8],
    board_size: usize,
    win_len: usize,
    evaluation: crate::config::EvaluationWeights,
    player: u8,
) -> bool {
    let hasher = Arc::new(ZobristHasher::new(board_size));
    let game_state = GameState::new(
        board.to_vec(),
        board_size,
        hasher,
        PROGRAM_PLAYER,
        win_len,
        evaluation,
    );
    GomokuRules::check_win(&game_state.position, player)
}

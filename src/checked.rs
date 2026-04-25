#[must_use]
pub(crate) fn add_u64(left: u64, right: u64, context: &str) -> u64 {
    let Some(value) = left.checked_add(right) else {
        eprintln!("{context} 发生 u64 加法溢出: {left} + {right}");
        panic!("{context} 发生 u64 加法溢出");
    };
    value
}
#[must_use]
pub(crate) fn sub_u64(left: u64, right: u64, context: &str) -> u64 {
    let Some(value) = left.checked_sub(right) else {
        eprintln!("{context} 发生 u64 减法下溢: {left} - {right}");
        panic!("{context} 发生 u64 减法下溢");
    };
    value
}
#[must_use]
pub(crate) fn div_u64(left: u64, right: u64, context: &str) -> u64 {
    let Some(value) = left.checked_div(right) else {
        eprintln!("{context} 发生 u64 除法错误: {left} / {right}");
        panic!("{context} 发生 u64 除法错误");
    };
    value
}
#[must_use]
pub(crate) fn rounded_div_u64(total: u64, count: u64, context: &str) -> u64 {
    if count == 0_u64 {
        eprintln!("{context} 的计数不能为 0。");
        panic!("{context} 的计数不能为 0");
    }
    let half = div_u64(count, 2_u64, context);
    div_u64(add_u64(total, half, context), count, context)
}
#[must_use]
pub(crate) fn add_u32(left: u32, right: u32, context: &str) -> u32 {
    let Some(value) = left.checked_add(right) else {
        eprintln!("{context} 发生 u32 加法溢出: {left} + {right}");
        panic!("{context} 发生 u32 加法溢出");
    };
    value
}
#[must_use]
pub(crate) fn div_u32(left: u32, right: u32, context: &str) -> u32 {
    let Some(value) = left.checked_div(right) else {
        eprintln!("{context} 发生 u32 除法错误: {left} / {right}");
        panic!("{context} 发生 u32 除法错误");
    };
    value
}
#[must_use]
pub(crate) fn rem_u32(left: u32, right: u32, context: &str) -> u32 {
    let Some(value) = left.checked_rem(right) else {
        eprintln!("{context} 发生 u32 取余错误: {left} % {right}");
        panic!("{context} 发生 u32 取余错误");
    };
    value
}
#[must_use]
pub(crate) fn add_usize(left: usize, right: usize, context: &str) -> usize {
    let Some(value) = left.checked_add(right) else {
        eprintln!("{context} 发生 usize 加法溢出: {left} + {right}");
        panic!("{context} 发生 usize 加法溢出");
    };
    value
}
#[must_use]
pub(crate) fn sub_usize(left: usize, right: usize, context: &str) -> usize {
    let Some(value) = left.checked_sub(right) else {
        eprintln!("{context} 发生 usize 减法下溢: {left} - {right}");
        panic!("{context} 发生 usize 减法下溢");
    };
    value
}
#[must_use]
pub(crate) fn mul_usize(left: usize, right: usize, context: &str) -> usize {
    let Some(value) = left.checked_mul(right) else {
        eprintln!("{context} 发生 usize 乘法溢出: {left} * {right}");
        panic!("{context} 发生 usize 乘法溢出");
    };
    value
}
#[must_use]
pub(crate) fn div_usize(left: usize, right: usize, context: &str) -> usize {
    let Some(value) = left.checked_div(right) else {
        eprintln!("{context} 发生 usize 除法错误: {left} / {right}");
        panic!("{context} 发生 usize 除法错误");
    };
    value
}
#[must_use]
pub(crate) fn rem_usize(left: usize, right: usize, context: &str) -> usize {
    let Some(value) = left.checked_rem(right) else {
        eprintln!("{context} 发生 usize 取余错误: {left} % {right}");
        panic!("{context} 发生 usize 取余错误");
    };
    value
}
#[must_use]
pub(crate) fn usize_to_u64(value: usize, context: &str) -> u64 {
    match u64::try_from(value) {
        Ok(converted) => converted,
        Err(err) => {
            eprintln!("{context} 从 usize 转换为 u64 失败: {value}, 错误: {err}");
            panic!("{context} 从 usize 转换为 u64 失败");
        }
    }
}
#[must_use]
pub(crate) fn u64_to_usize(value: u64, context: &str) -> usize {
    match usize::try_from(value) {
        Ok(converted) => converted,
        Err(err) => {
            eprintln!("{context} 从 u64 转换为 usize 失败: {value}, 错误: {err}");
            panic!("{context} 从 u64 转换为 usize 失败");
        }
    }
}
#[must_use]
pub(crate) fn usize_to_u32(value: usize, context: &str) -> u32 {
    match u32::try_from(value) {
        Ok(converted) => converted,
        Err(err) => {
            eprintln!("{context} 从 usize 转换为 u32 失败: {value}, 错误: {err}");
            panic!("{context} 从 usize 转换为 u32 失败");
        }
    }
}
#[must_use]
pub(crate) fn usize_to_u16(value: usize, context: &str) -> u16 {
    match u16::try_from(value) {
        Ok(converted) => converted,
        Err(err) => {
            eprintln!("{context} 从 usize 转换为 u16 失败: {value}, 错误: {err}");
            panic!("{context} 从 usize 转换为 u16 失败");
        }
    }
}
#[must_use]
pub(crate) fn shl_u64(value: u64, shift_amount: usize, context: &str) -> u64 {
    let shift = usize_to_u32(shift_amount, context);
    let Some(shifted) = value.checked_shl(shift) else {
        eprintln!("{context} 左移超出 u64 范围: {shift_amount}");
        panic!("{context} 左移超出 u64 范围");
    };
    shifted
}
#[must_use]
pub(crate) fn shr_u64(value: u64, shift_amount: usize, context: &str) -> u64 {
    let shift = usize_to_u32(shift_amount, context);
    let Some(shifted) = value.checked_shr(shift) else {
        eprintln!("{context} 右移超出 u64 范围: {shift_amount}");
        panic!("{context} 右移超出 u64 范围");
    };
    shifted
}
#[must_use]
pub(crate) fn opponent_player(player: u8, context: &str) -> u8 {
    match player {
        1 => 2,
        2 => 1,
        _ => {
            eprintln!("{context} 收到非法玩家编号: {player}");
            panic!("{context} 收到非法玩家编号");
        }
    }
}

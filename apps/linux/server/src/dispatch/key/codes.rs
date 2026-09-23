//! 按键分派用的虚拟键码与字符解析。

use qingjian_platform::protocol::KeyEvent;

pub(crate) const BACK: u32 = 0x08;
pub(crate) const DELETE: u32 = 0x2E;
pub(crate) const TAB: u32 = 0x09;
pub(crate) const RETURN: u32 = 0x0D;
pub(crate) const ESCAPE: u32 = 0x1B;
pub(crate) const PRIOR: u32 = 0x21;
pub(crate) const NEXT: u32 = 0x22;
pub(crate) const END: u32 = 0x23;
pub(crate) const HOME: u32 = 0x24;
pub(crate) const LEFT: u32 = 0x25;
pub(crate) const UP: u32 = 0x26;
pub(crate) const RIGHT: u32 = 0x27;
pub(crate) const DOWN: u32 = 0x28;

/// 翻页键对 `(上一页, 下一页)`：返回 -1 / +1。
pub(crate) fn page_key(event: &KeyEvent, page_keys: (char, char)) -> Option<isize> {
    let c = event.character?;
    if c == page_keys.0 {
        Some(-1)
    } else if c == page_keys.1 {
        Some(1)
    } else {
        None
    }
}

/// 敲出来是数字 1–9 的键（选候选用）：按 `character` 认，Shift 出的 `!@#` 不算。
pub(crate) fn digit(event: &KeyEvent) -> Option<usize> {
    match event.character {
        Some(c) => ('1'..='9').contains(&c).then(|| c as usize - '0' as usize),
        None => digit_key(event.virtual_key),
    }
}

/// 主键盘区数字键 1–9 的键码，不管修饰键（修饰键 + 数字的快捷键按键位认）。
pub(crate) fn digit_key(virtual_key: u32) -> Option<usize> {
    (0x31..=0x39)
        .contains(&virtual_key)
        .then(|| (virtual_key - 0x30) as usize)
}

/// 协议保留小键盘来源，运算符保持半角。
pub(crate) fn is_keypad(virtual_key: u32) -> bool {
    (0x60..=0x6F).contains(&virtual_key)
}

/// 主键盘区的 J / K：调频键配的两个字母（vim 键位），`true` 是升、`false` 是降。
pub(crate) fn adjust_key(virtual_key: u32) -> Option<bool> {
    match virtual_key {
        0x4B => Some(true),
        0x4A => Some(false),
        _ => None,
    }
}

/// 修饰键的物理键码：Windows 的 VK 值，加上 fcitx5 送来的 X11 keysym
/// （`mapKey` 只把 Shift 映射成了 VK，Ctrl / Alt / Super 仍是 keysym 原值）。
pub(crate) fn is_modifier_key(virtual_key: u32) -> bool {
    matches!(
        virtual_key,
        0x10 | 0x11 | 0x12 // Shift / Ctrl / Alt（Windows VK）
            | 0xA0..=0xA5 | 0x5B | 0x5C
            | 0xFFE1..=0xFFE4 | 0xFFE7..=0xFFEE // X11: Shift / Ctrl, Meta / Alt / Super / Hyper
    )
}

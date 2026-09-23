use serde::{Deserialize, Serialize};

use super::modifiers::KeyModifiers;

/// DLL 从 TSF `OnKeyDown` / `OnTestKeyDown` 抓到的一次按键，发给 Server 判定。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct KeyEvent {
    /// Windows 虚拟键码（`VK_*`）。翻页、方向键、退格、回车等靠它区分。
    pub virtual_key: u32,

    /// 这次按键产生的字符（`ToUnicode` 的结果）；功能键没有字符时为 `None`。
    pub character: Option<char>,

    /// 按下时的修饰键状态。
    pub modifiers: KeyModifiers,

    /// 这是按键抬起（TSF 的 `OnKeyUp`）而不是按下。组句里 DLL 会把调频修饰键的按下与抬起都送过来，
    /// Server 据此开关频次预览；老 DLL 不带这个字段，读成 `false`（按下）。
    #[serde(default)]
    pub release: bool,
}

impl KeyEvent {
    pub fn new(virtual_key: u32, character: Option<char>, modifiers: KeyModifiers) -> Self {
        Self {
            virtual_key,
            character,
            modifiers,
            release: false,
        }
    }

    /// 同一次按键的抬起事件（见 [`Self::release`]）。
    pub fn released(mut self) -> Self {
        self.release = true;
        self
    }
}

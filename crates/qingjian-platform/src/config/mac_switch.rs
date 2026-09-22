//! macOS 的中 / 英切换键（`[shortcut] mac_switch_*` 那一组）：修饰键单击（左右分开算）或组合键。
//!
//! Windows 的切换键是 [`SwitchKey`](super::SwitchKey)：单击 Shift / Ctrl / Ctrl+Space 翻转，一个键一种行为。
//! macOS 多两件事：左右修饰键要分开（左 ⌘ 与右 ⌘ 是两个键），除了"一个键翻转"还能配"两个键各切一边"；
//! 两个开关可以**同时开**，也都能与 Caps Lock 并存（同一个键两边都配时以双键那边为准）。
//! 写法与 `[general] scheme` 一样懒解析：写坏了退回缺省并记一条警告，不让整份配置加载失败。
//!
//! Caps Lock 不在这里：它是**状态型**的（亮着就是英文），由 `[shortcut] mac_caps_lock_switch` 单独管，
//! 与这里"单击一下切一次"的语义不能混在一起。

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

use super::key_combo::KeyCombo;
use super::modifiers::Modifiers;

/// 可当切换键的修饰键，带左右区分。配置里写成 `left-command` / `right-shift` 这类。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum MacModifier {
    LeftCommand,
    RightCommand,
    LeftShift,
    RightShift,
    LeftOption,
    RightOption,
    LeftControl,
    RightControl,
}

impl MacModifier {
    /// 全部取值，按左右成对排列。
    pub const ALL: [Self; 8] = [
        Self::LeftCommand,
        Self::RightCommand,
        Self::LeftShift,
        Self::RightShift,
        Self::LeftOption,
        Self::RightOption,
        Self::LeftControl,
        Self::RightControl,
    ];

    /// 配置文件里的写法。
    pub const fn key(self) -> &'static str {
        match self {
            Self::LeftCommand => "left-command",
            Self::RightCommand => "right-command",
            Self::LeftShift => "left-shift",
            Self::RightShift => "right-shift",
            Self::LeftOption => "left-option",
            Self::RightOption => "right-option",
            Self::LeftControl => "left-control",
            Self::RightControl => "right-control",
        }
    }

    /// 界面上的名字。
    pub const fn label(self) -> &'static str {
        match self {
            Self::LeftCommand => "左 ⌘",
            Self::RightCommand => "右 ⌘",
            Self::LeftShift => "左 ⇧",
            Self::RightShift => "右 ⇧",
            Self::LeftOption => "左 ⌥",
            Self::RightOption => "右 ⌥",
            Self::LeftControl => "左 ⌃",
            Self::RightControl => "右 ⌃",
        }
    }
}

impl FromStr for MacModifier {
    type Err = String;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let normalized = text.trim().to_ascii_lowercase();
        let (side, name) = normalized.split_once('-').ok_or_else(|| {
            format!("expected left- or right- plus a modifier name, got {text:?}")
        })?;
        let left = match side {
            "left" | "l" => true,
            "right" | "r" => false,
            other => return Err(format!("unknown side: {other}")),
        };
        Ok(match name {
            "command" | "cmd" => {
                if left {
                    Self::LeftCommand
                } else {
                    Self::RightCommand
                }
            }
            "shift" => {
                if left {
                    Self::LeftShift
                } else {
                    Self::RightShift
                }
            }
            "option" | "alt" => {
                if left {
                    Self::LeftOption
                } else {
                    Self::RightOption
                }
            }
            "control" | "ctrl" => {
                if left {
                    Self::LeftControl
                } else {
                    Self::RightControl
                }
            }
            other => return Err(format!("unknown modifier: {other}")),
        })
    }
}

/// macOS 上的一个切换键：修饰键单击（`left-command`）或修饰键 + 字母 / 数字的组合键（`control+option+z`）。
///
/// 组合键不分左右（按住哪个 ⌘ 都算），也不接受空格 / 方向键这类没有字符的键：写法沿用 [`KeyCombo`] 那一套。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub enum MacSwitchKey {
    Modifier(MacModifier),
    Combo(KeyCombo),
}

impl MacSwitchKey {
    /// 单键切换的缺省键：左 ⌘ 单击（打字时不与别的键组合，误触也小）。
    pub const DEFAULT_TOGGLE: Self = Self::Modifier(MacModifier::LeftCommand);

    /// 双键切换的缺省：左 ⌘ 切英文、右 ⌘ 切中文。
    pub const DEFAULT_ENGLISH: Self = Self::Modifier(MacModifier::LeftCommand);
    pub const DEFAULT_CHINESE: Self = Self::Modifier(MacModifier::RightCommand);

    /// 配置文件里的写法。
    pub fn key_string(&self) -> String {
        match self {
            Self::Modifier(modifier) => modifier.key().to_owned(),
            Self::Combo(combo) => combo.key_string(),
        }
    }

    /// 给人看的写法。
    pub fn label(&self) -> String {
        match self {
            Self::Modifier(modifier) => modifier.label().to_owned(),
            Self::Combo(combo) => combo.label(),
        }
    }

    /// 这个键是单击的修饰键吗；是就返回它。
    pub const fn modifier(&self) -> Option<MacModifier> {
        match self {
            Self::Modifier(modifier) => Some(*modifier),
            Self::Combo(_) => None,
        }
    }
}

impl FromStr for MacSwitchKey {
    type Err = String;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let trimmed = text.trim();
        if let Ok(modifier) = trimmed.parse::<MacModifier>() {
            return Ok(Self::Modifier(modifier));
        }
        trimmed.parse::<KeyCombo>().map(Self::Combo).map_err(|_| {
            format!("expected a modifier (left-command) or a combo (control+option+z), got {text:?}")
        })
    }
}

impl TryFrom<String> for MacSwitchKey {
    type Error = String;

    fn try_from(text: String) -> Result<Self, Self::Error> {
        text.parse()
    }
}

impl From<MacSwitchKey> for String {
    fn from(key: MacSwitchKey) -> Self {
        key.key_string()
    }
}

impl fmt::Display for MacSwitchKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.key_string())
    }
}

/// 切换键命中后切到哪边。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MacSwitchAction {
    /// 单键模式：翻转当前模式。
    Toggle,

    /// 切到英文模式。
    English,

    /// 切到中文模式。
    Chinese,
}

/// 校验之后的方案：两个开关加上键位，坏了的组合整组退掉。两个开关可以同时开，互不排斥。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct MacSwitchPlan {
    /// 单键切换：单击这个键在中 / 英之间翻转。
    pub toggle: Option<MacSwitchKey>,

    /// 双键切换：前一个单击切英文，后一个切中文。
    pub dual: Option<(MacSwitchKey, MacSwitchKey)>,
}

impl MacSwitchPlan {
    /// 一个额外的切换键都没配（只剩 Caps Lock 与界面上的按钮）。
    pub fn is_empty(&self) -> bool {
        self.toggle.is_none() && self.dual.is_none()
    }

    /// 这个修饰键在方案里吗；在就返回它切到哪边。
    ///
    /// 同一个键两边都配了时以双键那边为准：明确指定切哪边比"翻转"更具体。
    pub fn modifier_action(&self, key: MacModifier) -> Option<MacSwitchAction> {
        let hit = |candidate: &MacSwitchKey| candidate.modifier() == Some(key);
        if let Some((english, chinese)) = &self.dual {
            if hit(english) {
                return Some(MacSwitchAction::English);
            }
            if hit(chinese) {
                return Some(MacSwitchAction::Chinese);
            }
        }
        self.toggle
            .as_ref()
            .filter(|only| hit(only))
            .map(|_| MacSwitchAction::Toggle)
    }

    /// 这次按下的组合键在方案里吗；在就返回它切到哪边。`modifiers` 是事件里的修饰键，`key` 是不带修饰键的字符（小写）。
    pub fn combo_action(&self, modifiers: Modifiers, key: char) -> Option<MacSwitchAction> {
        let hit = |candidate: &MacSwitchKey| match candidate {
            MacSwitchKey::Combo(combo) => combo.modifiers == modifiers && combo.key == key,
            MacSwitchKey::Modifier(_) => false,
        };
        if let Some((english, chinese)) = &self.dual {
            if hit(english) {
                return Some(MacSwitchAction::English);
            }
            if hit(chinese) {
                return Some(MacSwitchAction::Chinese);
            }
        }
        self.toggle
            .as_ref()
            .filter(|only| hit(only))
            .map(|_| MacSwitchAction::Toggle)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn modifiers_parse_with_aliases_and_sides() {
        assert_eq!(
            "left-command".parse::<MacModifier>().unwrap(),
            MacModifier::LeftCommand
        );
        assert_eq!(
            "right-cmd".parse::<MacModifier>().unwrap(),
            MacModifier::RightCommand
        );
        assert_eq!(
            "l-shift".parse::<MacModifier>().unwrap(),
            MacModifier::LeftShift
        );
        assert_eq!(
            "Right-Option".parse::<MacModifier>().unwrap(),
            MacModifier::RightOption
        );
        assert_eq!(
            "right-ctrl".parse::<MacModifier>().unwrap(),
            MacModifier::RightControl
        );
        assert!("command".parse::<MacModifier>().is_err());
        assert!("middle-shift".parse::<MacModifier>().is_err());
        assert!("left-hyper".parse::<MacModifier>().is_err());
        for modifier in MacModifier::ALL {
            assert_eq!(modifier.key().parse::<MacModifier>(), Ok(modifier));
        }
    }

    #[test]
    fn switch_key_takes_modifiers_and_combos() {
        let modifier: MacSwitchKey = "right-command".parse().unwrap();
        assert_eq!(modifier.modifier(), Some(MacModifier::RightCommand));
        assert_eq!(modifier.key_string(), "right-command");
        assert_eq!(modifier.label(), "右 ⌘");

        let combo: MacSwitchKey = "control+option+z".parse().unwrap();
        assert_eq!(combo.modifier(), None);
        assert_eq!(combo.key_string(), "control+option+z");
        assert_eq!(combo.label(), "⌃⌥Z");
        assert_eq!(combo.to_string(), "control+option+z");

        assert!("hyper".parse::<MacSwitchKey>().is_err());
        assert!("option+".parse::<MacSwitchKey>().is_err());
    }

    #[test]
    fn plan_matches_modifiers_by_side() {
        let dual = MacSwitchPlan {
            toggle: None,
            dual: Some((
                MacSwitchKey::Modifier(MacModifier::LeftCommand),
                MacSwitchKey::Modifier(MacModifier::RightCommand),
            )),
        };
        assert_eq!(
            dual.modifier_action(MacModifier::LeftCommand),
            Some(MacSwitchAction::English)
        );
        assert_eq!(
            dual.modifier_action(MacModifier::RightCommand),
            Some(MacSwitchAction::Chinese)
        );
        assert_eq!(dual.modifier_action(MacModifier::LeftShift), None);

        let single = MacSwitchPlan {
            toggle: Some(MacSwitchKey::Modifier(MacModifier::LeftControl)),
            dual: None,
        };
        assert_eq!(
            single.modifier_action(MacModifier::LeftControl),
            Some(MacSwitchAction::Toggle)
        );
        assert_eq!(
            single.modifier_action(MacModifier::RightControl),
            None,
            "左右是两个键"
        );

        assert!(MacSwitchPlan::default().is_empty());
        assert_eq!(
            MacSwitchPlan::default().modifier_action(MacModifier::LeftCommand),
            None
        );
    }

    #[test]
    fn both_switches_can_be_on_and_dual_wins_on_a_shared_key() {
        let both = MacSwitchPlan {
            toggle: Some(MacSwitchKey::Modifier(MacModifier::LeftShift)),
            dual: Some((
                MacSwitchKey::Modifier(MacModifier::LeftCommand),
                MacSwitchKey::Modifier(MacModifier::RightCommand),
            )),
        };
        assert!(!both.is_empty());
        assert_eq!(
            both.modifier_action(MacModifier::LeftShift),
            Some(MacSwitchAction::Toggle)
        );
        assert_eq!(
            both.modifier_action(MacModifier::LeftCommand),
            Some(MacSwitchAction::English)
        );

        // 同一个键两边都配：以双键那边为准
        let shared = MacSwitchPlan {
            toggle: Some(MacSwitchKey::Modifier(MacModifier::LeftCommand)),
            dual: Some((
                MacSwitchKey::Modifier(MacModifier::LeftCommand),
                MacSwitchKey::Modifier(MacModifier::RightCommand),
            )),
        };
        assert_eq!(
            shared.modifier_action(MacModifier::LeftCommand),
            Some(MacSwitchAction::English)
        );
    }

    #[test]
    fn plan_matches_combos_by_modifiers_and_key() {
        let combo: KeyCombo = "control+option+z".parse().unwrap();
        let plan = MacSwitchPlan {
            toggle: Some(MacSwitchKey::Combo(combo)),
            dual: None,
        };
        assert_eq!(
            plan.combo_action(combo.modifiers, 'z'),
            Some(MacSwitchAction::Toggle)
        );
        assert_eq!(plan.combo_action(combo.modifiers, 'x'), None);
        assert_eq!(plan.combo_action(Modifiers::CONTROL, 'z'), None);
        // 组合键与修饰键单击不会互相命中
        let modifier_only = MacSwitchPlan {
            toggle: Some(MacSwitchKey::Modifier(MacModifier::LeftCommand)),
            dual: None,
        };
        assert_eq!(modifier_only.combo_action(combo.modifiers, 'z'), None);
        assert_eq!(plan.modifier_action(MacModifier::LeftCommand), None);
    }
}

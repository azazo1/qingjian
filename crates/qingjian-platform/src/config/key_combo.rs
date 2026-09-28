use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

use super::modifiers::Modifiers;

/// 修饰键 + 一个键的组合，配置里写成 `control+option+t`、`control+.`。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct KeyCombo {
    /// 修饰键，至少一个。
    pub modifiers: Modifiers,

    /// 字母, 数字或标点键（字母小写；标点没有大小写）。`+` 写不出来：它是写法里的分隔符。
    pub key: char,
}

impl KeyCombo {
    pub const TRANSLATE_DEFAULT: Self = Self {
        modifiers: Modifiers {
            option: true,
            shift: false,
            control: true,
            command: false,
        },
        key: 't',
    };

    /// 组句里把候选高亮往下挪一格的缺省键 (emacs 的下一行, 配置 `[shortcut] highlight_down`).
    pub const HIGHLIGHT_DOWN: Self = Self {
        modifiers: Modifiers::CONTROL,
        key: 'n',
    };

    /// 往上挪一格的缺省键 (配置 `[shortcut] highlight_up`).
    pub const HIGHLIGHT_UP: Self = Self {
        modifiers: Modifiers::CONTROL,
        key: 'p',
    };

    /// 弹出「录入词组」窗口的缺省键 (配置 `[shortcut] learn_phrase`): 与「翻译选中文字」的 ⌃⌥T 同一族, P 取 phrase.
    pub const LEARN_PHRASE: Self = Self {
        modifiers: Modifiers {
            option: true,
            shift: false,
            control: true,
            command: false,
        },
        key: 'p',
    };

    /// 中文模式下切换中文 / 英文标点的缺省键 (配置 `[shortcut] punctuation_toggle`), 微软拼音的惯例.
    pub const PUNCTUATION_TOGGLE: Self = Self {
        modifiers: Modifiers::CONTROL,
        key: '.',
    };

    /// 配置文件里的写法。
    pub fn key_string(&self) -> String {
        format!("{}+{}", self.modifiers.key(), self.key)
    }

    /// 给人看的写法：`⌃⌥T`。
    pub fn label(&self) -> String {
        format!(
            "{}{}",
            self.modifiers.label(),
            self.key.to_ascii_uppercase()
        )
    }
}

impl Default for KeyCombo {
    fn default() -> Self {
        Self::TRANSLATE_DEFAULT
    }
}

impl FromStr for KeyCombo {
    type Err = String;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let (modifiers, key) = text
            .trim()
            .rsplit_once('+')
            .ok_or_else(|| format!("expected modifiers+key, got {text:?}"))?;
        let mut chars = key.trim().chars();
        let (Some(key), None) = (chars.next(), chars.next()) else {
            return Err(format!("key must be a single character: {key:?}"));
        };
        if !key.is_ascii_graphic() {
            return Err(format!("key must be an ASCII printable character: {key:?}"));
        }
        Ok(Self {
            modifiers: modifiers.parse()?,
            key: key.to_ascii_lowercase(),
        })
    }
}

impl TryFrom<String> for KeyCombo {
    type Error = String;

    fn try_from(text: String) -> Result<Self, Self::Error> {
        text.parse()
    }
}

impl From<KeyCombo> for String {
    fn from(combo: KeyCombo) -> Self {
        combo.key_string()
    }
}

impl fmt::Display for KeyCombo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.key_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_and_labels() {
        let combo: KeyCombo = "control+option+t".parse().unwrap();
        assert_eq!(combo, KeyCombo::TRANSLATE_DEFAULT);
        assert_eq!(combo.label(), "⌃⌥T");
        assert_eq!(combo.key_string(), "control+option+t");
        assert!("t".parse::<KeyCombo>().is_err());
        assert!("option+tt".parse::<KeyCombo>().is_err());
        // 空格与非 ASCII 不能当键; 标点可以 (中文 / 英文标点切换的 Ctrl+.)
        assert!("option+ ".parse::<KeyCombo>().is_err());
        assert!("option+…".parse::<KeyCombo>().is_err());
        let period: KeyCombo = "control+.".parse().unwrap();
        assert_eq!(period, KeyCombo::PUNCTUATION_TOGGLE);
        assert_eq!(period.key_string(), "control+.");
        assert_eq!(period.label(), "⌃.");
        for option in ["control+shift+t", "control+option+e", "shift+command+9"] {
            assert_eq!(option.parse::<KeyCombo>().unwrap().key_string(), option);
        }
    }
}

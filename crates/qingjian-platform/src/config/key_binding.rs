//! 一项可以留空的快捷键: 配置里写成 `none` 就是不用这项, 敲它不再有输入法的动作.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// 一项快捷键的配置写法: 配着某个键, 或者 `none` 关掉.
///
/// `T` 是键的写法: [`Modifiers`](super::Modifiers) 只记修饰键 (配数字键上屏译词 / 删候选三组),
/// 需要字母的用 [`KeyCombo`](super::KeyCombo) (翻译选中文字).
/// 空串不算关掉, 那是写坏了; 只有明确的 `none` / `off` / `disabled` 才是这项不用.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyBinding<T> {
    /// 关着: 这项快捷键没有绑定.
    Off,

    /// 配着的键.
    On(T),
}

impl<T> KeyBinding<T> {
    /// 配着这个键.
    pub const fn on(key: T) -> Self {
        Self::On(key)
    }

    /// 配着的键; 关着就是 `None`.
    pub fn key(self) -> Option<T> {
        match self {
            Self::Off => None,
            Self::On(key) => Some(key),
        }
    }

    /// 关着吗.
    pub const fn is_off(&self) -> bool {
        matches!(self, Self::Off)
    }

    /// 换一种键, 关着的还是关着的.
    pub fn map<U>(self, convert: impl FnOnce(T) -> U) -> KeyBinding<U> {
        match self {
            Self::Off => KeyBinding::Off,
            Self::On(key) => KeyBinding::On(convert(key)),
        }
    }
}

impl<T> From<Option<T>> for KeyBinding<T> {
    fn from(key: Option<T>) -> Self {
        match key {
            Some(key) => Self::On(key),
            None => Self::Off,
        }
    }
}

impl<T: FromStr<Err = String>> FromStr for KeyBinding<T> {
    type Err = String;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let text = text.trim();
        if matches!(text.to_ascii_lowercase().as_str(), "none" | "off" | "disabled") {
            return Ok(Self::Off);
        }
        text.parse::<T>().map(Self::On)
    }
}

impl<T: fmt::Display> fmt::Display for KeyBinding<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Off => f.write_str("none"),
            Self::On(key) => key.fmt(f),
        }
    }
}

// serde 手写而不是 derive: 泛型参数只需要能解析能打印, 不必自己满足 serde 的 bound.
impl<'de, T: FromStr<Err = String>> Deserialize<'de> for KeyBinding<T> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        text.parse().map_err(serde::de::Error::custom)
    }
}

impl<T: fmt::Display> Serialize for KeyBinding<T> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{KeyCombo, Modifiers};

    #[derive(Deserialize)]
    struct Wrapper {
        key: KeyBinding<Modifiers>,
    }

    #[test]
    fn none_turns_a_binding_off() {
        let parsed: Wrapper = toml::from_str("key = \"none\"").unwrap();
        assert!(parsed.key.is_off());
        assert_eq!(parsed.key.key(), None);
        assert_eq!(parsed.key.to_string(), "none");
        for alias in ["off", "disabled", " NONE "] {
            let parsed: KeyBinding<Modifiers> = alias.parse().unwrap();
            assert!(parsed.is_off(), "{alias}");
        }
        let combo: KeyBinding<KeyCombo> = "none".parse().unwrap();
        assert_eq!(combo.key(), None);
    }

    #[test]
    fn a_real_key_parses_and_prints_as_before() {
        let parsed: Wrapper = toml::from_str("key = \"shift+option\"").unwrap();
        assert_eq!(parsed.key, KeyBinding::on(Modifiers::SHIFT_OPTION));
        assert_eq!(parsed.key.to_string(), "shift+option");
        // 空串与写坏的键是错误, 不是「关掉」
        assert!("".parse::<KeyBinding<Modifiers>>().is_err());
        assert!("hyper".parse::<KeyBinding<Modifiers>>().is_err());
    }
}

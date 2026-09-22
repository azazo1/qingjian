use qingjian_core::ModeKeys;
use serde::{Deserialize, Serialize};

use super::key_combo::KeyCombo;
use super::mac_switch::{MacSwitchKey, MacSwitchPlan};
use super::modifiers::Modifiers;
use super::switch_key::SwitchKey;

/// 配置文件 `[shortcut]` 分节：前缀模式键（Core 的 [`ModeKeys`]）加壳层的修饰键组合。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ShortcutConfig {
    /// 表达式 / 问字模式键，键名与以前一样直接在分节下（`expression` / `question`）。
    #[serde(flatten)]
    pub mode: ModeKeys,

    /// 中 / 英切换键（单击，Windows 用）：`shift` / `control` / `none`。详见 [`SwitchKey`]。
    pub switch_mode: SwitchKey,

    /// macOS：单键切换开关（`[shortcut] mac_switch_single`）。开着时 `mac_switch_toggle` 那个键单击在中英之间翻转。
    pub mac_switch_single: bool,

    /// 单键切换用的那个键或组合键（`left-command` / `control+option+z`）。
    pub mac_switch_toggle: String,

    /// macOS：双键切换开关（`[shortcut] mac_switch_dual`）。开着时下面两个键单击各切一边。
    pub mac_switch_dual: bool,

    /// 双键切换的两个键或组合键：先切英文、后切中文。
    pub mac_switch_english: String,
    pub mac_switch_chinese: String,

    /// Caps Lock 是否也切中 / 英（缺省 true，与以前一致）；false 时它只当大小写锁，亮着敲字母直接上屏大写。
    pub mac_caps_lock_switch: bool,

    /// 数字键配这些修饰键：上屏候选的第一个译词。
    pub translation: Modifiers,

    /// 数字键配这些修饰键：上屏候选的第二个译词（候选右侧有两个译词时）。
    pub translation_second: Modifiers,

    /// 把应用里选中的文字译成学习语言（需要云服务开着）。
    pub translate_selection: KeyCombo,

    /// 数字键配这些修饰键：删掉候选（用户词整个删掉，词库词清掉对它的学习）。
    pub delete_candidate: Modifiers,
}

impl Default for ShortcutConfig {
    fn default() -> Self {
        // Windows 上 Alt+数字被系统当菜单快捷键截走（TSF 收不到），译词键缺省用 Ctrl；macOS 用 Option。
        #[cfg(windows)]
        let (translation, translation_second) = (Modifiers::CONTROL, Modifiers::SHIFT_CONTROL);
        #[cfg(not(windows))]
        let (translation, translation_second) = (Modifiers::OPTION, Modifiers::SHIFT_OPTION);
        Self {
            mode: ModeKeys::default(),
            switch_mode: SwitchKey::default(),
            mac_switch_single: false,
            mac_switch_toggle: MacSwitchKey::DEFAULT_TOGGLE.key_string(),
            mac_switch_dual: false,
            mac_switch_english: MacSwitchKey::DEFAULT_ENGLISH.key_string(),
            mac_switch_chinese: MacSwitchKey::DEFAULT_CHINESE.key_string(),
            mac_caps_lock_switch: true,
            translation,
            translation_second,
            translate_selection: KeyCombo::TRANSLATE_DEFAULT,
            delete_candidate: Modifiers::SHIFT,
        }
    }
}

impl ShortcutConfig {
    /// 删候选的修饰键；为空或与任一组译词键撞了就退回缺省。
    pub fn delete_keys(&self) -> Modifiers {
        let (first, second) = self.translation_keys();
        if self.delete_candidate.is_empty()
            || self.delete_candidate == first
            || self.delete_candidate == second
        {
            Self::default().delete_candidate
        } else {
            self.delete_candidate
        }
    }

    /// 两组译词修饰键；两组相同或有一组为空时整个退回缺省，不做一半。
    pub fn translation_keys(&self) -> (Modifiers, Modifiers) {
        if self.translation == self.translation_second
            || self.translation.is_empty()
            || self.translation_second.is_empty()
        {
            let default = Self::default();
            (default.translation, default.translation_second)
        } else {
            (self.translation, self.translation_second)
        }
    }

    /// 单键切换用的那个键；写坏了退回缺省并记一条警告。
    pub fn mac_switch_toggle_key(&self) -> MacSwitchKey {
        parse_switch_key(&self.mac_switch_toggle, MacSwitchKey::DEFAULT_TOGGLE)
    }

    /// 双键切换用的两个键；任一个写坏了就整对退回缺省，两个键相同也算没配（翻转语义就重复了）。
    pub fn mac_switch_dual_keys(&self) -> (MacSwitchKey, MacSwitchKey) {
        let (Ok(english), Ok(chinese)) = (
            self.mac_switch_english.parse::<MacSwitchKey>(),
            self.mac_switch_chinese.parse::<MacSwitchKey>(),
        ) else {
            if !self.mac_switch_english.trim().is_empty()
                || !self.mac_switch_chinese.trim().is_empty()
            {
                tracing::warn!(
                    english = %self.mac_switch_english,
                    chinese = %self.mac_switch_chinese,
                    "双键切换的两个键有一个认不出来，整对退回缺省"
                );
            }
            return (MacSwitchKey::DEFAULT_ENGLISH, MacSwitchKey::DEFAULT_CHINESE);
        };
        if english == chinese {
            tracing::warn!("双键切换的两个键相同，整对退回缺省");
            return (MacSwitchKey::DEFAULT_ENGLISH, MacSwitchKey::DEFAULT_CHINESE);
        }
        (english, chinese)
    }

    /// macOS 的切换键方案：两个开关各管一组键位，开关关着的一组不参与（两组可以同时开）。
    pub fn mac_switch_plan(&self) -> MacSwitchPlan {
        MacSwitchPlan {
            toggle: self.mac_switch_single.then(|| self.mac_switch_toggle_key()),
            dual: self.mac_switch_dual.then(|| self.mac_switch_dual_keys()),
        }
    }
}

/// 解析一个切换键；空的（老配置文件里没有这一项）不算错，写坏了才记警告。
fn parse_switch_key(text: &str, fallback: MacSwitchKey) -> MacSwitchKey {
    if text.trim().is_empty() {
        return fallback;
    }
    match text.parse::<MacSwitchKey>() {
        Ok(key) => key,
        Err(_) => {
            tracing::warn!(value = %text, "macOS 的切换键认不出来，按缺省");
            fallback
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::mac_switch::MacModifier;
    use super::*;

    #[test]
    fn old_files_without_modifier_keys_still_parse_and_get_defaults() {
        // 缺省值分平台（Windows 用 Ctrl 系、其余用 Option 系），断言跟着平台的 Default 走
        let default = ShortcutConfig::default();
        let parsed: ShortcutConfig = toml::from_str("expression = \"i\"\n").unwrap();
        assert_eq!(parsed.mode.expression, 'i');
        assert_eq!(parsed.switch_mode, SwitchKey::Shift);
        assert_eq!(
            parsed.translation_keys(),
            (default.translation, default.translation_second)
        );
        let same: ShortcutConfig =
            toml::from_str("translation = \"option\"\ntranslation_second = \"option\"\n").unwrap();
        assert_eq!(
            same.translation_keys(),
            (default.translation, default.translation_second)
        );
        let swapped: ShortcutConfig =
            toml::from_str("translation = \"control+option\"\ntranslation_second = \"option\"\n")
                .unwrap();
        assert_eq!(swapped.translation_keys().1, Modifiers::OPTION);
    }

    #[test]
    fn delete_keys_fall_back_when_clashing_with_translation_keys() {
        let default = ShortcutConfig::default();
        let parsed: ShortcutConfig = toml::from_str("").unwrap();
        assert_eq!(parsed.delete_keys(), default.delete_candidate);
        // 与平台缺省的译词键撞上才算「冲突」，两边平台都成立
        let clash: ShortcutConfig = toml::from_str(&format!(
            "delete_candidate = \"{}\"\n",
            default.translation.key()
        ))
        .unwrap();
        assert_eq!(clash.delete_keys(), default.delete_candidate);
        // 不与任何一组译词键冲突的修饰键：平台上取一个，断言它原样生效
        let free = [Modifiers::OPTION, Modifiers::CONTROL, Modifiers::SHIFT]
            .into_iter()
            .find(|m| *m != default.translation && *m != default.translation_second)
            .unwrap();
        let custom: ShortcutConfig =
            toml::from_str(&format!("delete_candidate = \"{}\"\n", free.key())).unwrap();
        assert_eq!(custom.delete_keys(), free);
    }

    #[test]
    fn switch_mode_parses_and_defaults_to_shift() {
        let parsed: ShortcutConfig = toml::from_str("switch_mode = \"ctrl\"\n").unwrap();
        assert_eq!(parsed.switch_mode, SwitchKey::Control);
        let off: ShortcutConfig = toml::from_str("switch_mode = \"none\"\n").unwrap();
        assert_eq!(off.switch_mode, SwitchKey::None);
        let missing: ShortcutConfig = toml::from_str("").unwrap();
        assert_eq!(missing.switch_mode, SwitchKey::Shift);
    }

    #[test]
    fn mac_switches_are_off_until_asked_for() {
        let default: ShortcutConfig = toml::from_str("").unwrap();
        assert!(!default.mac_switch_single);
        assert!(!default.mac_switch_dual);
        assert!(default.mac_switch_plan().is_empty());
        assert!(default.mac_caps_lock_switch, "缺省 Caps Lock 就是切换键");
        assert_eq!(
            default.mac_switch_toggle_key(),
            MacSwitchKey::DEFAULT_TOGGLE
        );
    }

    #[test]
    fn both_switches_can_be_on_at_once() {
        let both: ShortcutConfig = toml::from_str(
            "mac_switch_single = true\nmac_switch_toggle = \"left-shift\"\nmac_switch_dual = true\n",
        )
        .unwrap();
        let plan = both.mac_switch_plan();
        assert_eq!(
            plan.toggle,
            Some(MacSwitchKey::Modifier(MacModifier::LeftShift))
        );
        assert_eq!(
            plan.dual,
            Some((MacSwitchKey::DEFAULT_ENGLISH, MacSwitchKey::DEFAULT_CHINESE))
        );

        // 只开双键：单键那组不参与
        let dual_only: ShortcutConfig = toml::from_str("mac_switch_dual = true\n").unwrap();
        let plan = dual_only.mac_switch_plan();
        assert_eq!(plan.toggle, None);
        assert!(plan.dual.is_some());
    }

    #[test]
    fn mac_switch_plan_falls_back_instead_of_doing_half() {
        let single: ShortcutConfig =
            toml::from_str("mac_switch_single = true\nmac_switch_toggle = \"right-command\"\n")
                .unwrap();
        assert_eq!(
            single.mac_switch_plan().toggle,
            Some(MacSwitchKey::Modifier(MacModifier::RightCommand))
        );

        // 单键写坏：退回缺省键，开关本身照旧生效
        let broken: ShortcutConfig =
            toml::from_str("mac_switch_single = true\nmac_switch_toggle = \"hyper\"\n").unwrap();
        assert_eq!(
            broken.mac_switch_plan().toggle,
            Some(MacSwitchKey::DEFAULT_TOGGLE)
        );

        let dual: ShortcutConfig = toml::from_str(
            "mac_switch_dual = true\nmac_switch_english = \"left-command\"\nmac_switch_chinese = \"right-cmd\"\n",
        )
        .unwrap();
        assert_eq!(
            dual.mac_switch_plan().dual,
            Some((
                MacSwitchKey::Modifier(MacModifier::LeftCommand),
                MacSwitchKey::Modifier(MacModifier::RightCommand)
            ))
        );

        // 两个键相同 / 有一个写坏：整对退回缺省
        for text in [
            "mac_switch_dual = true\nmac_switch_english = \"left-command\"\nmac_switch_chinese = \"left-command\"\n",
            "mac_switch_dual = true\nmac_switch_english = \"left-command\"\nmac_switch_chinese = \"hyper\"\n",
        ] {
            let half: ShortcutConfig = toml::from_str(text).unwrap();
            assert_eq!(
                half.mac_switch_plan().dual,
                Some((MacSwitchKey::DEFAULT_ENGLISH, MacSwitchKey::DEFAULT_CHINESE))
            );
        }

        // 组合键也能当切换键
        let combo: ShortcutConfig =
            toml::from_str("mac_switch_single = true\nmac_switch_toggle = \"control+option+z\"\n")
                .unwrap();
        let Some(MacSwitchKey::Combo(combo)) = combo.mac_switch_plan().toggle else {
            panic!("组合键应当原样生效");
        };
        assert_eq!(combo.key, 'z');
    }
}

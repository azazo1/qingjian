//! 中 / 英模式状态与切换键的单击判定。
//!
//! 以前 macOS 的模式是"每次按键现读 Caps Lock"，没有状态可存；现在 Caps Lock 可以不当切换键，
//! 配置里还能有单键 / 双键切换，所以模式是一份显式状态：Caps Lock 的物理跳变、配置的切换键单击，都改它。

use qingjian_platform::{MacModifier, MacSwitchAction, MacSwitchPlan};

use super::Host;
use crate::imk::modifiers;

impl Host {
    /// Caps Lock 参与中 / 英切换吗：内置英文模式关掉后（`[general] english_mode`），
    /// 或 `[shortcut] mac_caps_lock_switch` 关掉后，它只当大小写锁。
    pub fn caps_lock_switches_mode(&self) -> bool {
        self.english_mode && self.mac_caps_lock_switch
    }

    /// 用给定的 Caps Lock 状态刷新模式，返回当前是否英文模式（手头已经读过系统状态时用这个）。
    pub fn sync_mode(&mut self, caps: bool) -> bool {
        if !self.english_mode {
            // 内置英文模式关掉：固定中文模式，Caps Lock 只当大小写锁
            self.mode.sync_caps(caps, false);
            self.mode.set_english(false);
            return false;
        }
        self.mode.sync_caps(caps, self.mac_caps_lock_switch);
        self.mode.english()
    }

    /// 读一次物理 Caps Lock 刷新模式，返回当前是否英文模式。文本、标点、菜单栏都走这一条，看到的才是同一个值。
    pub fn refresh_mode(&mut self) -> bool {
        self.sync_mode(modifiers::caps_lock_on())
    }
}

/// 中 / 英模式。
#[derive(Debug, Default)]
pub struct ModeState {
    /// 当前是英文模式。
    english: bool,

    /// 上次读到的物理 Caps Lock，用来认出"跳变"。
    caps: bool,
}

impl ModeState {
    pub const fn english(&self) -> bool {
        self.english
    }

    /// 读一次物理 Caps Lock。`switches_mode` 为真时它的跳变直接决定模式（亮 = 英文，与以前一致）；
    /// 为假时只记下大小写锁的状态，模式保持不变。返回模式是否被改过。
    pub fn sync_caps(&mut self, caps: bool, switches_mode: bool) -> bool {
        if caps == self.caps {
            return false;
        }
        self.caps = caps;
        if !switches_mode {
            return false;
        }
        let changed = self.english != caps;
        self.english = caps;
        changed
    }

    /// 把模式设成英文 / 中文；返回是否真的变了。
    pub fn set_english(&mut self, english: bool) -> bool {
        if self.english == english {
            return false;
        }
        self.english = english;
        true
    }
}

/// 单击判定：切换键按下到抬起之间没插进别的键，才算一次单击（照 Windows 的 `KeyTap`）。
///
/// 判定只认修饰键：macOS 上 ⌘ / ⇧ / ⌥ / ⌃ 的按下抬起走 flagsChanged，普通键走 keyDown。
#[derive(Debug, Default)]
pub struct SwitchMatcher {
    /// 切换键已按下，还没被别的键打断。
    held: Option<MacModifier>,
}

impl SwitchMatcher {
    /// 任一普通键按下：打断本次单击判定。组句中的字母、⌘C 这类快捷键都算。
    pub fn key_down(&mut self) {
        self.held = None;
    }

    /// 修饰键按下 / 抬起；构成一次单击时返回它切到哪边。
    pub fn modifier_changed(
        &mut self,
        key: MacModifier,
        down: bool,
        plan: &MacSwitchPlan,
    ) -> Option<MacSwitchAction> {
        let action = plan.modifier_action(key);
        if down {
            // 只记切换键本身；⇧ / ⌥ 这类别的修饰键按下不算打断（⌘C 那种真正打断的是普通键）
            if action.is_some() {
                self.held = Some(key);
            }
            return None;
        }
        // 抬起的不是按着的那个切换键（别的修饰键，或另一个切换键）：判定留着，别把它清掉
        if self.held != Some(key) {
            return None;
        }
        self.held = None;
        action
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use qingjian_platform::MacSwitchKey;

    fn dual() -> MacSwitchPlan {
        MacSwitchPlan {
            toggle: None,
            dual: Some((
                MacSwitchKey::Modifier(MacModifier::LeftCommand),
                MacSwitchKey::Modifier(MacModifier::RightCommand),
            )),
        }
    }

    #[test]
    fn caps_lock_drives_the_mode_when_it_is_the_switch_key() {
        let mut mode = ModeState::default();
        assert!(!mode.english());
        assert!(mode.sync_caps(true, true), "亮起来就是切到英文");
        assert!(mode.english());
        assert!(!mode.sync_caps(true, true), "状态没变就不算改过");
        assert!(mode.sync_caps(false, true));
        assert!(!mode.english());
    }

    #[test]
    fn caps_lock_only_locks_case_when_it_is_not_the_switch_key() {
        let mut mode = ModeState::default();
        mode.set_english(true);
        assert!(!mode.sync_caps(true, false), "不参与切换时跳变不改模式");
        assert!(mode.english());
        assert!(!mode.sync_caps(false, false));
        assert!(mode.english());
    }

    #[test]
    fn set_english_reports_changes() {
        let mut mode = ModeState::default();
        assert!(mode.set_english(true));
        assert!(!mode.set_english(true));
        assert!(mode.set_english(false));
    }

    #[test]
    fn a_clean_tap_fires_once() {
        let mut matcher = SwitchMatcher::default();
        let plan = dual();
        assert_eq!(
            matcher.modifier_changed(MacModifier::LeftCommand, true, &plan),
            None
        );
        assert_eq!(
            matcher.modifier_changed(MacModifier::LeftCommand, false, &plan),
            Some(MacSwitchAction::English)
        );
        // 抬起只算一次：再报一次抬起不该再触发
        assert_eq!(
            matcher.modifier_changed(MacModifier::LeftCommand, false, &plan),
            None
        );
        // 右键走另一条：切中文
        matcher.modifier_changed(MacModifier::RightCommand, true, &plan);
        assert_eq!(
            matcher.modifier_changed(MacModifier::RightCommand, false, &plan),
            Some(MacSwitchAction::Chinese)
        );
    }

    #[test]
    fn another_key_in_between_cancels_the_tap() {
        let mut matcher = SwitchMatcher::default();
        let plan = dual();
        matcher.modifier_changed(MacModifier::LeftCommand, true, &plan);
        matcher.key_down(); // 中间敲了 ⌘C 的 C、或者一个字母
        assert_eq!(
            matcher.modifier_changed(MacModifier::LeftCommand, false, &plan),
            None
        );
    }

    #[test]
    fn other_modifiers_do_not_cancel_but_are_not_switch_keys() {
        let mut matcher = SwitchMatcher::default();
        let plan = dual();
        matcher.modifier_changed(MacModifier::LeftCommand, true, &plan);
        // 按住 ⌘ 再按 ⇧：⇧ 不是切换键，判定还在
        assert_eq!(
            matcher.modifier_changed(MacModifier::LeftShift, true, &plan),
            None
        );
        matcher.modifier_changed(MacModifier::LeftShift, false, &plan);
        assert_eq!(
            matcher.modifier_changed(MacModifier::LeftCommand, false, &plan),
            Some(MacSwitchAction::English)
        );
        // 不是切换键的修饰键按下抬起不触发任何动作
        matcher.modifier_changed(MacModifier::LeftOption, true, &plan);
        assert_eq!(
            matcher.modifier_changed(MacModifier::LeftOption, false, &plan),
            None
        );
    }

    #[test]
    fn single_key_toggles() {
        let mut matcher = SwitchMatcher::default();
        let plan = MacSwitchPlan {
            toggle: Some(MacSwitchKey::Modifier(MacModifier::RightControl)),
            dual: None,
        };
        matcher.modifier_changed(MacModifier::RightControl, true, &plan);
        assert_eq!(
            matcher.modifier_changed(MacModifier::RightControl, false, &plan),
            Some(MacSwitchAction::Toggle)
        );
    }
}

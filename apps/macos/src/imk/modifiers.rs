//! 当前修饰键状态。IMK 的 `inputText:client:` 不带事件对象，Caps Lock / Shift 只能从系统当前状态读；
//! 左右修饰键则从 flagsChanged 事件的键码认。

use objc2_app_kit::{NSEvent, NSEventModifierFlags};
use qingjian_platform::{MacModifier, Modifiers};

/// 事件标志位里的修饰键组合（不算 Caps Lock：它是状态型的大小写锁，不是修饰键）。
/// 调频预览要拿它与 `[shortcut] adjust_frequency` 比，看调频键是不是正按着。
pub fn from_flags(flags: NSEventModifierFlags) -> Modifiers {
    Modifiers {
        option: flags.contains(NSEventModifierFlags::Option),
        shift: flags.contains(NSEventModifierFlags::Shift),
        control: flags.contains(NSEventModifierFlags::Control),
        command: flags.contains(NSEventModifierFlags::Command),
    }
}

/// Caps Lock 亮着。它是**状态型**的：参与中 / 英切换时（见配置 `[shortcut] mac_caps_lock_switch`）
/// 亮着就是英文模式；不参与时只当大小写锁，亮着敲字母直接上屏大写。
pub fn caps_lock_on() -> bool {
    NSEvent::modifierFlags_class().contains(NSEventModifierFlags::CapsLock)
}

/// Shift 正按着。读的是此刻的硬件状态而不是事件自带的标志，但 Shift 是按住不放的键，处理按键时它几乎总还按着。
/// macOS 上 Caps Lock 亮着时按住 Shift 送来的仍是大写（不像 Windows 会反转），所以英文模式的大小写只能靠它判断。
pub fn shift_down() -> bool {
    NSEvent::modifierFlags_class().contains(NSEventModifierFlags::Shift)
}

/// flagsChanged 事件的键码对应的修饰键（含左右）。
///
/// 这些是 macOS 的虚拟键码，与键盘布局无关。认不出来的键码（远程桌面工具常送 0）返回 `None`：
/// 那不是青简能当切换键用的修饰键，忽略即可。
pub fn modifier_key(key_code: u16) -> Option<MacModifier> {
    Some(match key_code {
        55 => MacModifier::LeftCommand,
        54 => MacModifier::RightCommand,
        56 => MacModifier::LeftShift,
        60 => MacModifier::RightShift,
        58 => MacModifier::LeftOption,
        61 => MacModifier::RightOption,
        59 => MacModifier::LeftControl,
        62 => MacModifier::RightControl,
        _ => return None,
    })
}

/// flagsChanged 事件里这个修饰键对应的标志位：还在就是按下，没了就是抬起。
pub fn modifier_flag(key: MacModifier) -> NSEventModifierFlags {
    match key {
        MacModifier::LeftCommand | MacModifier::RightCommand => NSEventModifierFlags::Command,
        MacModifier::LeftShift | MacModifier::RightShift => NSEventModifierFlags::Shift,
        MacModifier::LeftOption | MacModifier::RightOption => NSEventModifierFlags::Option,
        MacModifier::LeftControl | MacModifier::RightControl => NSEventModifierFlags::Control,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keycodes_tell_left_from_right() {
        assert_eq!(modifier_key(55), Some(MacModifier::LeftCommand));
        assert_eq!(modifier_key(54), Some(MacModifier::RightCommand));
        assert_eq!(modifier_key(56), Some(MacModifier::LeftShift));
        assert_eq!(modifier_key(60), Some(MacModifier::RightShift));
        assert_eq!(modifier_key(58), Some(MacModifier::LeftOption));
        assert_eq!(modifier_key(61), Some(MacModifier::RightOption));
        assert_eq!(modifier_key(59), Some(MacModifier::LeftControl));
        assert_eq!(modifier_key(62), Some(MacModifier::RightControl));
        // Caps Lock（57）不是单击型的切换键；远程桌面送来的 0 与普通键也不认
        assert_eq!(modifier_key(57), None);
        assert_eq!(modifier_key(0), None);
        assert_eq!(modifier_key(0x41), None);
    }

    #[test]
    fn every_switchable_modifier_has_a_keycode() {
        let keys: Vec<MacModifier> = [55u16, 54, 56, 60, 58, 61, 59, 62]
            .into_iter()
            .filter_map(modifier_key)
            .collect();
        assert_eq!(keys.len(), MacModifier::ALL.len());
        for key in MacModifier::ALL {
            assert!(keys.contains(&key), "{key:?} 少了对应的键码");
        }
    }
}

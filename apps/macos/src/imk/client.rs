//! 对 IMK 客户端对象（实现 `IMKTextInput` 协议的代理）的薄封装。
//!
//! objc2-input-method-kit 没有为 IMKTextInput 生成绑定，这里用 `msg_send!` 直接发消息。

use objc2::msg_send;
use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2_foundation::{NSAttributedString, NSDictionary, NSNotFound, NSRange, NSRect, NSString};

/// `{NSNotFound, 0}`：不替换任何已有文本，插到当前位置。
const NO_REPLACEMENT: NSRange = NSRange::new(NSNotFound as usize, 0);

#[derive(Clone, Copy)]
pub struct TextClient<'a> {
    /// IMK 传进来的 `sender`。
    object: &'a AnyObject,
    /// 这个 client 收不下写回的文字（[`Self::rejects_text`] 的结果，按键分发时填）。
    rejects: bool,
    /// 收不下，且能改用合成键盘事件上屏（有辅助功能权限）：`insert_text` 改走 [`super::synthetic`]，
    /// `set_marked_text` 什么都不做（拼音只显示在候选窗口里）。
    detached: bool,
}

impl<'a> TextClient<'a> {
    pub fn new(object: &'a AnyObject) -> Self {
        Self {
            object,
            rejects: false,
            detached: false,
        }
    }

    /// 带上这一键判断出的 client 状态：`rejects` 见 [`Self::rejects_text`]，`can_post` 是能不能发合成键盘事件。
    pub fn with_rejects(self, rejects: bool, can_post: bool) -> Self {
        Self {
            rejects,
            detached: rejects && can_post,
            ..self
        }
    }

    /// 这一键面对的 client 收不下写回的文字。
    pub fn rejects(&self) -> bool {
        self.rejects
    }

    /// 上屏改走合成键盘事件（见 `detached` 字段）。
    pub fn detached(&self) -> bool {
        self.detached
    }

    /// 设置 marked text（带下划线的未上屏文本），光标放在第 `cursor` 个字符处。空串等于清除。
    pub fn set_marked_text(&self, text: &str, cursor: usize) {
        // 写了也会被丢掉；拼音由候选窗口顶部那行显示
        if self.detached {
            return;
        }
        let string = NSString::from_str(text);
        let cursor = NSRange::new(cursor.min(text.chars().count()), 0);
        unsafe {
            let _: () = msg_send![
                self.object,
                setMarkedText: &*string,
                selectionRange: cursor,
                replacementRange: NO_REPLACEMENT
            ];
        }
    }

    /// 上屏。client 收不下时改成攒进合成键盘事件的发件箱，这一键分发完再发（见 [`super::synthetic`]）。
    pub fn insert_text(&self, text: &str) {
        if self.detached {
            super::synthetic::queue_text(text);
            return;
        }
        let string = NSString::from_str(text);
        unsafe {
            let _: () =
                msg_send![self.object, insertText: &*string, replacementRange: NO_REPLACEMENT];
        }
    }

    /// 这个 client 收不下输入法写回的文字（`insertText:` / `setMarkedText:` 都会被丢掉），按键只能原样交还应用。
    ///
    /// 已知场景：Zen 等 Gecko 浏览器开着 `widget.macos.native-popovers` 时，书签面板、扩展弹窗这类 native popover
    /// 里的输入框。输入视图没进 `_NSPopoverWindow` 的响应链，IMK 拿到的 client 背后没有编辑框：
    /// `length` 是 `INT32_MAX`（2147483647）、`selectedRange` 是 NotFound（正常的空输入框是 `0` 与 `0+0`）。
    /// 按键交还应用时走的是应用自己的 `interpretKeyEvents:`，能正常出字（系统 ABC、Rime 的英文状态就是这样）。
    /// 见 zen-browser/desktop#12626。
    pub fn rejects_text(&self) -> bool {
        let (length, selected): (usize, NSRange) = unsafe {
            (
                msg_send![self.object, length],
                msg_send![self.object, selectedRange],
            )
        };
        length == i32::MAX as usize && selected.location == NSNotFound as usize
    }

    /// 应用里当前选中的文字与它的范围（翻译用）。没有选区、应用不支持读文本、超过 `max_chars` 个字符都返回 `None`。
    pub fn selected_text(&self, max_chars: usize) -> Option<(String, NSRange)> {
        let selected: NSRange = unsafe { msg_send![self.object, selectedRange] };
        if selected.location == NSNotFound as usize
            || selected.length == 0
            || selected.length > max_chars
        {
            return None;
        }
        let text: Option<Retained<NSAttributedString>> =
            unsafe { msg_send![self.object, attributedSubstringFromRange: selected] };
        let text = text?.string().to_string();
        (!text.trim().is_empty()).then_some((text, selected))
    }

    /// 用 `text` 替换应用里 `range` 那段文字（翻译结果替换选区）。
    pub fn replace_range(&self, text: &str, range: NSRange) {
        let string = NSString::from_str(text);
        unsafe {
            let _: () = msg_send![self.object, insertText: &*string, replacementRange: range];
        }
    }

    /// 正在输入的应用的 bundle identifier (`com.apple.Terminal`), 按应用改行为用; 应用没给返回 `None`.
    pub fn bundle_identifier(&self) -> Option<String> {
        let bundle: Option<Retained<NSString>> =
            unsafe { msg_send![self.object, bundleIdentifier] };
        bundle.map(|b| b.to_string()).filter(|b| !b.is_empty())
    }

    /// 光标（marked text 起点）所在行在屏幕坐标系里的矩形，用来定位候选窗口。
    /// 应用不支持时返回零矩形，窗口就会落在屏幕左下角，至少看得见。
    pub fn caret_rect(&self) -> NSRect {
        let mut rect = NSRect::ZERO;
        unsafe {
            let _: Option<Retained<NSDictionary>> = msg_send![self.object, attributesForCharacterIndex: 0usize, lineHeightRectangle: &mut rect];
        }
        rect
    }
}

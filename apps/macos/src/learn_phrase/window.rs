//! 录入词组窗口本体: 词组框, 拼音框, 状态行, 录入 / 取消.

use std::cell::Cell;

use objc2::rc::Retained;
use objc2::runtime::ProtocolObject;
use objc2::{MainThreadMarker, sel};
use objc2_app_kit::{NSButton, NSColor, NSFont, NSTextAlignment, NSTextField, NSView};
use objc2_foundation::{NSPoint, NSRect, NSSize, NSString};

use super::panel::LearnPhrasePanel;
use super::target::LearnPhraseTarget;

/// 词组框的 tag, 文本变化时靠它认出是哪一格.
pub(super) const PHRASE_TAG: isize = 1;

/// 拼音框的 tag.
pub(super) const PINYIN_TAG: isize = 2;

const WIDTH: f64 = 420.0;
const HEIGHT: f64 = 176.0;
const PAD: f64 = 20.0;
const LABEL_W: f64 = 44.0;
const ROW: f64 = 24.0;
const GAP: f64 = 12.0;
const BUTTON_W: f64 = 88.0;

/// 菜单栏点开的录入词组窗口.
pub struct LearnPhraseWindow {
    /// 窗口.
    panel: Retained<LearnPhrasePanel>,

    /// 待录入的词组.
    phrase: Retained<NSTextField>,

    /// 全拼, 空格分音节; 词组变化时自动填, 手改过后不再覆盖.
    pinyin: Retained<NSTextField>,

    /// 错误 (红) 或「已录入」 (灰).
    status: Retained<NSTextField>,

    /// 用户改过拼音框, 再改词组就不要覆盖拼音.
    dirty: Cell<bool>,

    /// 正在程序化改拼音, 这次变化不算手改.
    updating: Cell<bool>,

    /// 控件 target, 要和窗口活得一样久.
    _target: Retained<LearnPhraseTarget>,
}

impl LearnPhraseWindow {
    pub fn new(mtm: MainThreadMarker) -> Self {
        let target = LearnPhraseTarget::new(mtm);
        let panel = LearnPhrasePanel::new(
            mtm,
            NSRect::new(NSPoint::ZERO, NSSize::new(WIDTH, HEIGHT)),
        );
        panel.setTitle(&NSString::from_str("录入词组"));

        let content = NSView::initWithFrame(
            mtm.alloc(),
            NSRect::new(NSPoint::ZERO, NSSize::new(WIDTH, HEIGHT)),
        );

        let phrase = field(mtm, PHRASE_TAG, &target);
        phrase.setPlaceholderString(Some(&NSString::from_str("待录入词组")));
        let pinyin = field(mtm, PINYIN_TAG, &target);
        pinyin.setPlaceholderString(Some(&NSString::from_str("自动生成, 可改")));

        place_row(&content, mtm, "词组", &phrase, y_from_top(0.0));
        place_row(&content, mtm, "拼音", &pinyin, y_from_top(ROW + GAP));

        let confirm = button(mtm, "录入", sel!(confirm:), &target);
        confirm.setKeyEquivalent(&NSString::from_str("\r"));
        let cancel = button(mtm, "取消", sel!(cancel:), &target);
        cancel.setKeyEquivalent(&NSString::from_str("\u{1b}"));
        let button_y = y_from_top(2.0 * (ROW + GAP));
        let field_x = PAD + LABEL_W + 8.0;
        confirm.setFrame(NSRect::new(
            NSPoint::new(field_x, button_y),
            NSSize::new(BUTTON_W, ROW),
        ));
        cancel.setFrame(NSRect::new(
            NSPoint::new(field_x + BUTTON_W + 10.0, button_y),
            NSSize::new(BUTTON_W, ROW),
        ));
        content.addSubview(&confirm);
        content.addSubview(&cancel);

        let status = NSTextField::labelWithString(&NSString::from_str(""), mtm);
        status.setFont(Some(&NSFont::systemFontOfSize(11.0)));
        status.setFrame(NSRect::new(
            NSPoint::new(field_x, PAD),
            NSSize::new(WIDTH - field_x - PAD, 18.0),
        ));
        content.addSubview(&status);

        panel.setContentView(Some(&content));
        Self {
            panel,
            phrase,
            pinyin,
            status,
            dirty: Cell::new(false),
            updating: Cell::new(false),
            _target: target,
        }
    }

    /// 打开 (或带到最前). 关着再开时清空草稿.
    pub fn show(&self) {
        if !self.panel.isVisible() {
            self.clear();
        }
        self.panel.present();
        self.panel.makeFirstResponder(Some(&self.phrase));
    }

    pub fn close(&self) {
        self.panel.close();
    }

    pub fn phrase_text(&self) -> String {
        self.phrase.stringValue().to_string()
    }

    pub fn pinyin_text(&self) -> String {
        self.pinyin.stringValue().to_string()
    }

    pub fn pinyin_dirty(&self) -> bool {
        self.dirty.get()
    }

    pub fn set_pinyin(&self, syllables: Option<&[String]>) {
        self.updating.set(true);
        let text = syllables.map(|s| s.join(" ")).unwrap_or_default();
        self.pinyin.setStringValue(&NSString::from_str(&text));
        self.updating.set(false);
    }

    pub fn mark_pinyin_dirty(&self) {
        if !self.updating.get() {
            self.dirty.set(true);
        }
    }

    pub fn clear_dirty_if_phrase_empty(&self) {
        if self.phrase_text().trim().is_empty() {
            self.dirty.set(false);
        }
    }

    pub fn set_error(&self, text: &str) {
        self.status.setTextColor(Some(&NSColor::systemRedColor()));
        self.status.setStringValue(&NSString::from_str(text));
    }

    pub fn set_success(&self, text: &str) {
        self.status
            .setTextColor(Some(&NSColor::secondaryLabelColor()));
        self.status.setStringValue(&NSString::from_str(text));
    }

    /// 录入成功后清表单, 方便接着录下一条.
    pub fn clear_after_success(&self, learned: &str) {
        self.clear();
        self.set_success(&format!("已录入「{learned}」"));
        self.panel.makeFirstResponder(Some(&self.phrase));
    }

    fn clear(&self) {
        self.updating.set(true);
        self.phrase.setStringValue(&NSString::from_str(""));
        self.pinyin.setStringValue(&NSString::from_str(""));
        self.status.setStringValue(&NSString::from_str(""));
        self.updating.set(false);
        self.dirty.set(false);
    }
}

fn y_from_top(top: f64) -> f64 {
    HEIGHT - PAD - ROW - top
}

fn field(
    mtm: MainThreadMarker,
    tag: isize,
    target: &LearnPhraseTarget,
) -> Retained<NSTextField> {
    let field = NSTextField::initWithFrame(mtm.alloc(), NSRect::ZERO);
    field.setBezeled(true);
    field.setEditable(true);
    field.setSelectable(true);
    field.setDrawsBackground(true);
    field.setUsesSingleLineMode(true);
    field.setTag(tag);
    unsafe {
        field.setDelegate(Some(ProtocolObject::from_ref(target)));
    }
    field
}

fn button(
    mtm: MainThreadMarker,
    title: &str,
    action: objc2::runtime::Sel,
    target: &LearnPhraseTarget,
) -> Retained<NSButton> {
    // SAFETY: 选择器与 LearnPhraseTarget 上定义的 confirm: / cancel: 一致, 签名 (id) -> void
    unsafe {
        NSButton::buttonWithTitle_target_action(
            &NSString::from_str(title),
            Some(target),
            Some(action),
            mtm,
        )
    }
}

fn place_row(
    content: &NSView,
    mtm: MainThreadMarker,
    title: &str,
    field: &NSTextField,
    y: f64,
) {
    let label = NSTextField::labelWithString(&NSString::from_str(title), mtm);
    label.setAlignment(NSTextAlignment::Right);
    label.setFrame(NSRect::new(
        NSPoint::new(PAD, y),
        NSSize::new(LABEL_W, ROW),
    ));
    let field_x = PAD + LABEL_W + 8.0;
    field.setFrame(NSRect::new(
        NSPoint::new(field_x, y),
        NSSize::new(WIDTH - field_x - PAD, ROW),
    ));
    content.addSubview(&label);
    content.addSubview(field);
}

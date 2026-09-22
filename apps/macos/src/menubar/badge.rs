//! 光标旁的「中」/「英」徽标：切换模式时在光标行旁边闪一下，一秒后自己收掉（照 Rime 的状态提示）。
//!
//! 与菜单栏状态项（[`super::ModeIndicator`]）是两条显示：状态项常驻在菜单栏，徽标只在切换的那一刻出现，
//! 让人不用去瞟菜单栏就知道现在是什么模式。面板的建法与摆放跟候选窗口共用（[`crate::candidates`]），
//! 同样不抢焦点、不吃鼠标、跟着所有 Space。

use std::cell::RefCell;

use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send, sel};
use objc2_app_kit::{
    NSAppearance, NSAppearanceCustomization, NSAppearanceNameAqua, NSAppearanceNameDarkAqua,
    NSAttributedStringNSStringDrawing, NSBezierPath, NSColor, NSFont, NSFontAttributeName,
    NSForegroundColorAttributeName, NSPanel, NSView,
};
use objc2_foundation::{
    NSAttributedString, NSDictionary, NSObject, NSObjectProtocol, NSPoint, NSRect, NSSize,
    NSString, NSTimer,
};
use qingjian_platform::ThemeMode;

use crate::candidates::{build_float_panel, place_at_caret};

/// 徽标显示多久。
const BADGE_SECONDS: f64 = 1.0;

/// 徽标字号。
const FONT_SIZE: f64 = 14.0;

/// 左右 / 上下内边距。
const PADDING_X: f64 = 10.0;
const PADDING_Y: f64 = 5.0;

/// 光标旁的当前模式徽标。
pub struct ModeBadge {
    panel: Retained<NSPanel>,
    view: Retained<BadgeView>,

    /// 收起用的定时器；显示新一条时重来。
    timer: Option<Retained<NSTimer>>,

    /// 当前外观（跟随系统时为 `None`）。
    appearance: Option<Retained<NSAppearance>>,

    mtm: MainThreadMarker,
}

impl ModeBadge {
    pub fn new(mtm: MainThreadMarker) -> Self {
        let view = BadgeView::new(mtm);
        let panel = build_float_panel(mtm, &view);
        Self {
            panel,
            view,
            timer: None,
            appearance: None,
            mtm,
        }
    }

    /// 在光标行旁边显示 `text`（「中」/「英」），一秒后自动收。
    pub fn show(&mut self, text: &str, anchor: NSRect) {
        let size = self.view.set_text(text);
        let origin = place_at_caret(self.mtm, size, anchor);
        self.panel.setFrame_display(NSRect::new(origin, size), true);
        self.panel.orderFrontRegardless();
        if let Some(timer) = self.timer.take() {
            timer.invalidate();
        }
        let target = BadgeTicker::new(self.mtm);
        let timer = unsafe {
            NSTimer::scheduledTimerWithTimeInterval_target_selector_userInfo_repeats(
                BADGE_SECONDS,
                &target,
                sel!(tick:),
                None,
                false,
            )
        };
        self.timer = Some(timer);
        tracing::debug!(%text, ?anchor, ?origin, "模式徽标已显示");
    }

    /// 收起徽标；没在显示就什么都不做。
    pub fn hide(&mut self) {
        if let Some(timer) = self.timer.take() {
            timer.invalidate();
        }
        self.panel.orderOut(None);
    }

    /// 外观：跟随系统时不指定，否则强制浅色 / 深色，与候选窗口保持一致。
    pub fn set_theme(&mut self, mode: ThemeMode) {
        // SAFETY: 只读 AppKit 导出的常量名
        let name = unsafe {
            match mode {
                ThemeMode::System => None,
                ThemeMode::Light => Some(NSAppearanceNameAqua),
                ThemeMode::Dark => Some(NSAppearanceNameDarkAqua),
            }
        };
        let appearance = name.and_then(NSAppearance::appearanceNamed);
        self.panel.setAppearance(appearance.as_deref());
        self.appearance = appearance;
    }
}

/// 徽标视图的状态：一段排好版的文字、底色与圆角、内边距。
pub struct BadgeIvars {
    /// 当前文字（字体与颜色已在 [`BadgeView::set_text`] 里定好）。
    text: RefCell<Option<Retained<NSAttributedString>>>,

    /// 底色与文字色。
    background: Retained<NSColor>,
    text_color: Retained<NSColor>,

    /// 圆角。
    corner: f64,
}

define_class!(
    // SAFETY: NSView 允许子类化；没有实现 Drop。
    #[unsafe(super(NSView))]
    #[thread_kind = MainThreadOnly]
    #[ivars = BadgeIvars]
    /// 一块圆角小牌子，中间一行「中」/「英」。
    struct BadgeView;

    impl BadgeView {
        #[unsafe(method(isFlipped))]
        fn is_flipped(&self) -> bool {
            true
        }

        #[unsafe(method(drawRect:))]
        fn draw_rect(&self, _dirty: NSRect) {
            let ivars = self.ivars();
            let bounds = self.bounds();
            let path = NSBezierPath::bezierPathWithRoundedRect_xRadius_yRadius(
                bounds,
                ivars.corner,
                ivars.corner,
            );
            ivars.background.setFill();
            path.fill();
            let Some(text) = ivars.text.borrow().clone() else {
                return;
            };
            let size = text.size();
            let origin = NSPoint::new(
                (bounds.size.width - size.width) / 2.0,
                (bounds.size.height - size.height) / 2.0,
            );
            text.drawAtPoint(origin);
        }
    }

    unsafe impl NSObjectProtocol for BadgeView {}
);

impl BadgeView {
    fn new(mtm: MainThreadMarker) -> Retained<Self> {
        let this = mtm.alloc::<Self>().set_ivars(BadgeIvars {
            text: RefCell::new(None),
            background: NSColor::windowBackgroundColor(),
            text_color: NSColor::labelColor(),
            corner: 8.0,
        });
        unsafe { msg_send![super(this), initWithFrame: NSRect::ZERO] }
    }

    /// 换一行文字，返回视图该有多大（文字尺寸加内边距）。
    fn set_text(&self, text: &str) -> NSSize {
        let font = NSFont::systemFontOfSize(FONT_SIZE);
        let string = attributed(text, &font, &self.ivars().text_color);
        let size = string.size();
        *self.ivars().text.borrow_mut() = Some(string);
        self.setNeedsDisplay(true);
        NSSize::new(size.width + PADDING_X * 2.0, size.height + PADDING_Y * 2.0)
    }
}

/// 排一段带字体与颜色的文字。
fn attributed(text: &str, font: &NSFont, color: &NSColor) -> Retained<NSAttributedString> {
    // SAFETY: 只读 AppKit 导出的属性名常量
    let (keys, objects): (Vec<&NSString>, Vec<&AnyObject>) = unsafe {
        (
            vec![NSFontAttributeName, NSForegroundColorAttributeName],
            vec![font, color],
        )
    };
    let attributes = NSDictionary::from_slices(&keys, &objects);
    unsafe { NSAttributedString::new_with_attributes(&NSString::from_str(text), &attributes) }
}

define_class!(
    // SAFETY: NSObject 没有子类化要求；没有实现 Drop。
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[ivars = ()]
    /// 徽标的收起定时器：到点让 Host 收起来。
    struct BadgeTicker;

    impl BadgeTicker {
        #[unsafe(method(tick:))]
        fn tick(&self, _timer: Option<&AnyObject>) {
            crate::host::with(|h| h.badge.hide());
        }
    }

    unsafe impl NSObjectProtocol for BadgeTicker {}
);

impl BadgeTicker {
    fn new(mtm: MainThreadMarker) -> Retained<Self> {
        let this = mtm.alloc::<Self>().set_ivars(());
        unsafe { msg_send![super(this), init] }
    }
}

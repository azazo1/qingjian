//! 候选窗口：非激活的浮动 NSPanel，跟随光标，内容由 [`CandidateView`] 绘制。

use objc2::MainThreadMarker;
use objc2::rc::Retained;
use objc2_app_kit::{
    NSAppearance, NSAppearanceCustomization, NSAppearanceNameAqua, NSAppearanceNameDarkAqua,
    NSBackingStoreType, NSColor, NSEvent, NSPanel, NSScreen, NSView, NSWindowCollectionBehavior,
    NSWindowLevel, NSWindowStyleMask,
};
use objc2_foundation::{NSPoint, NSRect, NSSize};
use qingjian_platform::{CandidateRenderer, LayoutMode, ThemeMode};

use super::frame::Frame;
use super::theme::Theme;
use super::view::CandidateView;

#[link(name = "CoreGraphics", kind = "framework")]
unsafe extern "C" {
    fn CGShieldingWindowLevel() -> i32;
}

/// 截屏工具的覆盖层通常在 overlay / screensaver (102 / 1000) 之上, popup menu (101) 会被整块压住.
/// `CGShieldingWindowLevel()` 与 Squirrel 同级, 候选窗和模式徽标才能浮在截屏标注界面上.
fn shielding_window_level() -> NSWindowLevel {
    // SAFETY: 无参数查询, CoreGraphics 随 AppKit 一起可用.
    unsafe { CGShieldingWindowLevel() as NSWindowLevel }
}

/// 候选窗口与光标行之间的间隙。
const CARET_GAP: f64 = 4.0;

/// 应用给不出光标位置时，拿鼠标位置当光标，按这个高度算一行。
const FALLBACK_LINE_HEIGHT: f64 = 16.0;

pub struct CandidateWindow {
    /// 面板本体。不在当前 Space 时会整个换新（见 [`Self::order_front_on_active_space`]）。
    panel: Retained<NSPanel>,

    /// 内容视图；换面板时搬过去。
    view: Retained<CandidateView>,

    /// 当前外观（跟随系统时为 `None`）；换面板时要重设。
    appearance: Option<Retained<NSAppearance>>,

    /// 用来取屏幕尺寸。
    mtm: MainThreadMarker,
}

impl CandidateWindow {
    pub fn new(mtm: MainThreadMarker) -> Self {
        let view = CandidateView::new(mtm, Theme::system_default());
        let panel = build_float_panel(mtm, &view);
        Self {
            panel,
            view,
            appearance: None,
            mtm,
        }
    }

    /// 显示一帧。`anchor` 是光标行的屏幕矩形，窗口贴在它下方，放不下就放上方。
    pub fn show(&mut self, frame: Frame, anchor: NSRect) {
        if frame.is_empty() {
            self.hide();
            return;
        }
        let size = self.view.set_frame(&frame);
        let origin = self.place(size, anchor);
        self.panel.setFrame_display(NSRect::new(origin, size), true);
        self.order_front_on_active_space();
        if !self.panel.isVisible() {
            tracing::warn!(?anchor, ?origin, "候选窗口 orderFront 之后仍不可见");
        } else {
            tracing::debug!(
                ?anchor,
                ?origin,
                on_active_space = self.panel.isOnActiveSpace(),
                "候选窗口已显示"
            );
        }
    }

    pub fn hide(&self) {
        self.panel.orderOut(None);
    }

    /// 排到最前，并确认真在当前 Space 上；不在就换一块新面板。
    ///
    /// collection behavior 是「所有 Space + 全屏辅助」，可 macOS 26 上 WindowServer 只把面板绑到它创建时已有的 Space：
    /// 之后新开的全屏 Space（Zed 全屏就是一个）里没有它，候选框留在桌面那个 Space。用 CGS 查过，面板的 Space 列表只有桌面，
    /// 而同样设置新建的面板会绑到全部 Space；重设 collection behavior、收起再排前都不会让它重算，只有新建有用。
    /// `isOnActiveSpace` 能反映这个状态，所以每次显示后查一下，不在就把内容视图搬到新面板上。
    fn order_front_on_active_space(&mut self) {
        self.panel.orderFrontRegardless();
        if self.panel.isOnActiveSpace() {
            return;
        }
        let frame = self.panel.frame();
        self.panel.orderOut(None);
        let panel = build_float_panel(self.mtm, &self.view);
        panel.setAppearance(self.appearance.as_deref());
        panel.setFrame_display(frame, true);
        panel.orderFrontRegardless();
        self.panel = panel;
        tracing::warn!(
            recovered = self.panel.isOnActiveSpace(),
            "候选窗口不在当前 Space，已换新面板"
        );
    }

    /// 外观：跟随系统时不指定，否则强制浅色 / 深色。
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

    /// 竖排 / 横排。下一帧生效。
    pub fn set_layout(&self, layout: LayoutMode) {
        self.view.set_layout(layout);
    }

    /// 青简渲染器 / 系统绘制。下一帧生效。
    pub fn set_renderer(&self, renderer: CandidateRenderer) {
        self.view.set_renderer(renderer);
    }

    /// 候选窗字体（字族名，空为系统字体），只对青简渲染器生效。
    pub fn set_font(&self, font: &str) {
        self.view.set_font(font);
    }

    pub fn max_rows(&self) -> usize {
        self.view.theme().max_rows
    }

    /// 窗口左下角坐标：贴在光标行下方；下方放不下放上方；不出光标所在的那块屏幕。
    /// 光标矩形是零或落在所有屏幕之外（应用不支持、或给的是胡话）时以鼠标位置为准，至少落在用户看着的屏幕上。
    fn place(&self, size: NSSize, anchor: NSRect) -> NSPoint {
        place_at_caret(self.mtm, size, anchor)
    }
}

/// 把一块浮动面板摆在光标行旁边：优先下方，下方放不下放上方，都不行贴屏幕底边。
/// 光标矩形是零或落在所有屏幕之外（应用不支持、或给的是胡话）时以鼠标位置为准，至少落在用户看着的屏幕上。
/// 候选窗口与「中 / 英」徽标共用这一套，两个面板不会各摆各的。
pub(crate) fn place_at_caret(mtm: MainThreadMarker, size: NSSize, anchor: NSRect) -> NSPoint {
    let (anchor, screen) = match screen_containing(mtm, anchor.origin) {
        Some(screen) if !(anchor.size.height == 0.0 && anchor.origin == NSPoint::ZERO) => {
            (anchor, screen)
        }
        _ => {
            let mouse = NSEvent::mouseLocation();
            let anchor = NSRect::new(mouse, NSSize::new(0.0, FALLBACK_LINE_HEIGHT));
            (
                anchor,
                screen_containing(mtm, mouse).unwrap_or_else(|| main_screen_or_anywhere(mtm)),
            )
        }
    };
    let min_x = screen.origin.x;
    let max_x = (screen.origin.x + screen.size.width - size.width).max(min_x);
    let x = anchor.origin.x.clamp(min_x, max_x);
    let below = anchor.origin.y - CARET_GAP - size.height;
    let above = anchor.origin.y + anchor.size.height + CARET_GAP;
    let top = screen.origin.y + screen.size.height;
    let y = if below >= screen.origin.y {
        below
    } else if above + size.height <= top {
        above
    } else {
        // 上下都放不下（屏幕很矮或窗口很高）：贴屏幕底边，宁可盖住光标也别出屏
        screen.origin.y
    };
    // 无论怎么算，最后都要落在这块屏幕里：出屏等于不显示
    let max_y = (top - size.height).max(screen.origin.y);
    NSPoint::new(x, y.clamp(screen.origin.y, max_y))
}

/// 建一块浮动面板并把内容视图装进去：无边框、不抢焦点、透明背景带阴影、不吃鼠标。
/// 候选窗口与「中 / 英」徽标共用，两者除了内容视图没有区别。
pub(crate) fn build_float_panel(mtm: MainThreadMarker, view: &NSView) -> Retained<NSPanel> {
    let panel = NSPanel::initWithContentRect_styleMask_backing_defer(
        mtm.alloc::<NSPanel>(),
        NSRect::new(NSPoint::ZERO, NSSize::new(200.0, 100.0)),
        NSWindowStyleMask::Borderless | NSWindowStyleMask::NonactivatingPanel,
        NSBackingStoreType::Buffered,
        false,
    );
    panel.setOpaque(false);
    panel.setBackgroundColor(Some(&NSColor::clearColor()));
    panel.setHasShadow(true);
    panel.setBecomesKeyOnlyIfNeeded(true);
    panel.setIgnoresMouseEvents(true);
    // NSPanel 缺省在应用失活时自动隐藏；输入法进程从来不是前台应用，不能靠这个
    panel.setHidesOnDeactivate(false);
    panel.setCollectionBehavior(collection_behavior());
    // 不要 setFloatingPanel(true): 它会把层级改回 NSFloatingWindowLevel (3), 全屏应用的 Space 里就看不见了;
    // 层级最后设, 别被前面任何一项覆盖
    panel.setLevel(shielding_window_level());
    panel.setContentView(Some(view));
    panel
}

/// 面板的 Space 归属：出现在所有 Space 上、能与全屏应用同处一个 Space、Mission Control 里不动。
fn collection_behavior() -> NSWindowCollectionBehavior {
    NSWindowCollectionBehavior::CanJoinAllSpaces
        | NSWindowCollectionBehavior::FullScreenAuxiliary
        | NSWindowCollectionBehavior::Stationary
}

/// 包含 `point` 的那块屏幕的可见区域；哪块都不包含返回 `None`。
fn screen_containing(mtm: MainThreadMarker, point: NSPoint) -> Option<NSRect> {
    let screens = NSScreen::screens(mtm);
    let hit = screens.iter().find(|screen| {
        let frame = screen.frame();
        point.x >= frame.origin.x
            && point.x < frame.origin.x + frame.size.width
            && point.y >= frame.origin.y
            && point.y < frame.origin.y + frame.size.height
    });
    hit.map(|screen| screen.visibleFrame())
}

/// 主屏的可见区域；连主屏都没有时不限制。
fn main_screen_or_anywhere(mtm: MainThreadMarker) -> NSRect {
    NSScreen::mainScreen(mtm)
        .map(|s| s.visibleFrame())
        .unwrap_or_else(|| {
            NSRect::new(
                NSPoint::new(f64::MIN / 2.0, f64::MIN / 2.0),
                NSSize::new(f64::MAX, f64::MAX),
            )
        })
}

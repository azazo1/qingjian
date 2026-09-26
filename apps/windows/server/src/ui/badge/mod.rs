//! 模式徽标：切换中 / 英的那一刻在光标旁闪一块小面板（「中」/「英」），一秒后自己收。
//! 与悬浮状态条同样走分层窗口 + GDI 文字，但它不吃鼠标、不抢焦点、不能拖动，也没有常驻位置；
//! 触发点在 [`crate::dispatch::status`]：只在模式真的变了的那一下发来，配置 `[general] mode_badge`
//! 关掉就不闪，UI 线程收起它也不通知 Router。

use core::cell::Cell;
use std::cell::RefCell;
use std::rc::Rc;

use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, WPARAM};
use windows::Win32::Graphics::Gdi::{GetDC, ReleaseDC, SetBkMode, TRANSPARENT};
use windows::Win32::UI::HiDpi::{GetDpiForSystem, GetDpiForWindow};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, GetCursorPos, KillTimer, MA_NOACTIVATE,
    SW_HIDE, SW_SHOWNA, SetTimer, ShowWindow, WM_MOUSEACTIVATE, WM_TIMER, WNDCLASSEXW,
    WS_EX_LAYERED, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_POPUP,
};
use windows::core::{PCWSTR, Result, w};

use qingjian_platform::ThemeMode;
use qingjian_platform::protocol::ScreenRect;

use super::candidates::resolve_dark;
use super::candidates::theme::Theme;
use super::candidates::view;
use super::layered::{self, Layered};
use super::monitor;
use super::window_class::WindowClass;
use crate::dispatch::BadgeView;

const CLASS_NAME: PCWSTR = w!("QingjianModeBadge");
static CLASS: WindowClass = WindowClass::new();

/// 显示多久（毫秒）：闪一下就收，不打扰输入。
const LIFE_MS: u32 = 1000;

/// 自己收起用的定时器编号。
const TIMER_ID: usize = 1;

/// 与光标矩形之间的缝（逻辑像素）。
const CARET_GAP: i32 = 4;

/// 拿鼠标位置兜底时的偏移（逻辑像素）：别正好压在指针底下。
const CURSOR_GAP: i32 = 12;

/// 模式徽标窗口。
pub(super) struct Badge {
    hwnd: HWND,

    /// 按 DPI / 深浅造好的主题（复用候选窗口那套）。
    theme: RefCell<Rc<Theme>>,

    /// 上次用的 DPI，变了重建主题。
    dpi: Cell<u32>,

    /// 上次解析出的深浅，变了重建配色。
    dark: Cell<bool>,
}

impl Badge {
    /// 建一个隐藏的徽标窗口。
    pub(super) fn new() -> Result<Self> {
        CLASS.ensure(|| WNDCLASSEXW {
            lpfnWndProc: Some(wndproc),
            hInstance: super::module_handle(),
            lpszClassName: CLASS_NAME,
            ..Default::default()
        })?;
        let dpi = unsafe { GetDpiForSystem() }.max(96);
        let dark = resolve_dark(ThemeMode::default());
        // NOACTIVATE：显示时不抢应用焦点；TOOLWINDOW：不进任务栏与 Alt+Tab。
        let hwnd = unsafe {
            CreateWindowExW(
                WS_EX_LAYERED | WS_EX_TOOLWINDOW | WS_EX_TOPMOST | WS_EX_NOACTIVATE,
                CLASS_NAME,
                w!("青简模式徽标"),
                WS_POPUP,
                0,
                0,
                0,
                0,
                None,
                None,
                Some(super::module_handle()),
                None,
            )?
        };
        Ok(Self {
            hwnd,
            theme: RefCell::new(Rc::new(Theme::new(dpi, dark))),
            dpi: Cell::new(dpi),
            dark: Cell::new(dark),
        })
    }

    /// 闪一下：量出「中」/「英」的宽高、贴到锚点旁、显示并起一秒的定时器（再闪一次重新计时）。
    pub(super) fn flash(&self, view: BadgeView) {
        let text = if view.english { "英" } else { "中" };
        let theme = self.sync_theme(view.theme);
        let size = {
            let hdc = unsafe { GetDC(Some(self.hwnd)) };
            let size = view::measure(hdc, theme.text_font, text);
            unsafe { ReleaseDC(Some(self.hwnd), hdc) };
            size
        };
        let content = (size.cx + theme.padding * 2, size.cy + theme.padding);
        if content.0 <= 0 || content.1 <= 0 {
            return;
        }
        let margin = layered::shadow_margin(self.dpi.get());
        let anchor = self.anchor(view.anchor, content, margin);
        let (background, corner_radius, font, color) = (
            theme.background,
            theme.corner_radius,
            theme.text_font,
            theme.cloud_color,
        );
        let updated = layered::composite(
            self.hwnd,
            &Layered {
                content,
                margin,
                win_pos: (anchor.0 - margin, anchor.1 - margin),
                win_size: (content.0 + margin * 2, content.1 + margin * 2),
                background,
                corner_radius,
                paint: &|hdc, client| {
                    unsafe { SetBkMode(hdc, TRANSPARENT) };
                    let x = (client.right - size.cx) / 2;
                    let y = (client.bottom - size.cy) / 2;
                    view::draw_text(hdc, font, color, x, y, text);
                },
            },
        );
        if updated.is_err() {
            self.hide();
            return;
        }
        let _ = unsafe { ShowWindow(self.hwnd, SW_SHOWNA) };
        unsafe { SetTimer(Some(self.hwnd), TIMER_ID, LIFE_MS, None) };
    }

    /// 收起（定时器到点由窗口过程调，这里给画不出来时兜底）。
    pub(super) fn hide(&self) {
        let _ = unsafe { KillTimer(Some(self.hwnd), TIMER_ID) };
        let _ = unsafe { ShowWindow(self.hwnd, SW_HIDE) };
    }

    /// DPI 或深浅变了就重建主题（DPI 取窗口所在显示器）。
    fn sync_theme(&self, mode: ThemeMode) -> Rc<Theme> {
        let dpi = match unsafe { GetDpiForWindow(self.hwnd) } {
            0 => self.dpi.get(),
            dpi => dpi.max(96),
        };
        let dark = resolve_dark(mode);
        if dpi != self.dpi.get() || dark != self.dark.get() {
            *self.theme.borrow_mut() = Rc::new(Theme::new(dpi, dark));
            self.dpi.set(dpi);
            self.dark.set(dark);
        }
        self.theme.borrow().clone()
    }

    /// 内容左上角：光标矩形下方（下方放不下就翻到它上方），没有光标矩形就看鼠标位置；再夹进工作区。
    fn anchor(&self, caret: Option<ScreenRect>, content: (i32, i32), margin: i32) -> (i32, i32) {
        let scale = |px: i32| ((px * self.dpi.get() as i32) / 96).max(1);
        let (x, mut y, probe) = match caret {
            Some(rect) => (
                rect.left,
                rect.bottom + scale(CARET_GAP),
                POINT {
                    x: rect.left,
                    y: rect.bottom,
                },
            ),
            None => {
                let mut point = POINT::default();
                let _ = unsafe { GetCursorPos(&mut point) };
                let gap = scale(CURSOR_GAP);
                (point.x + gap, point.y + gap, point)
            }
        };
        let work = monitor::work_area_near(probe);
        if let Some(rect) = caret
            && y + content.1 + margin > work.bottom
        {
            y = rect.top - scale(CARET_GAP) - content.1;
        }
        let x = x.clamp(
            work.left + margin,
            (work.right - margin - content.0).max(work.left + margin),
        );
        let y = y.clamp(
            work.top + margin,
            (work.bottom - margin - content.1).max(work.top + margin),
        );
        (x, y)
    }
}

impl Drop for Badge {
    fn drop(&mut self) {
        let _ = unsafe { DestroyWindow(self.hwnd) };
    }
}

/// 徽标不吃鼠标（`MA_NOACTIVATE` 不抢焦点），到点自己收。
unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        WM_MOUSEACTIVATE => LRESULT(MA_NOACTIVATE as isize),
        WM_TIMER if wparam.0 == TIMER_ID => {
            let _ = unsafe { KillTimer(Some(hwnd), TIMER_ID) };
            let _ = unsafe { ShowWindow(hwnd, SW_HIDE) };
            LRESULT(0)
        }
        _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}

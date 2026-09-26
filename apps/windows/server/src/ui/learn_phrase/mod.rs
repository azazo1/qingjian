//! 「录入词组」窗口：一块普通 Win32 窗（两个 EDIT 框 + 一个按钮），词组变了就请工人线程反查拼音，
//! 点「录入」请它调 Core 写入。
//!
//! 词库与 Engine 都在工人线程，窗口这里只做界面：每次询问都投给工人线程（[`LearnPhraseWork`]），
//! 答完由工人线程唤醒本线程回来读（[`LearnPhraseReply`]），见 `ui::run` 的 `WM_WAKE` 分支。
//! 窗口关掉只是藏起来（`WM_CLOSE` 不销毁），进程活着就一直复用同一个。

use core::cell::Cell;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::mpsc::Sender;

use windows::Win32::Foundation::{E_FAIL, HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::UI::HiDpi::GetDpiForSystem;
use windows::Win32::UI::Input::KeyboardAndMouse::SetFocus;
use windows::Win32::UI::WindowsAndMessaging::{
    BN_CLICKED, CreateWindowExW, DefWindowProcW, DestroyWindow, EN_CHANGE, GetWindowTextLengthW,
    GetWindowTextW, HMENU, IsWindowVisible, SW_HIDE, SW_SHOW, SetForegroundWindow, SetWindowTextW,
    ShowWindow, WINDOW_EX_STYLE, WM_CLOSE, WM_COMMAND, WNDCLASSEXW, WS_BORDER, WS_CAPTION,
    WS_CHILD, WS_EX_TOOLWINDOW, WS_OVERLAPPED, WS_SYSMENU, WS_TABSTOP, WS_VISIBLE,
};
use windows::core::{Error, PCWSTR, Result, w};

use crate::dispatch::{LearnPhraseReply, LearnPhraseWork};
use crate::ipc::Work;

use super::window_class::WindowClass;

const CLASS_NAME: PCWSTR = w!("QingjianLearnPhrase");
static CLASS: WindowClass = WindowClass::new();

/// 词组框。
const ID_PHRASE: usize = 1;
/// 拼音框。
const ID_PINYIN: usize = 2;
/// 「录入」按钮。
const ID_CONFIRM: usize = 3;

/// 窗口尺寸（逻辑像素，含标题栏与边框）。
const WIDTH: i32 = 424;
const HEIGHT: i32 = 232;

thread_local! {
    /// UI 线程上唯一一个录入窗口：窗口过程按它读写状态。
    static WINDOW: RefCell<Option<Rc<LearnPhraseWindow>>> = const { RefCell::new(None) };
}

/// 「录入词组」窗口。
pub(super) struct LearnPhraseWindow {
    hwnd: HWND,

    /// 词组框。
    phrase: HWND,

    /// 拼音框。
    pinyin: HWND,

    /// 提示行（错误 / 成功 / 查不到）。
    hint: HWND,

    /// 拼音框被用户手改过：不再用反查结果覆盖它（清空词组后解除）。
    pinyin_dirty: Cell<bool>,

    /// 程序性设置文本时抑制 `EN_CHANGE`，免得把填进去的拼音当成用户手改。
    setting: Cell<bool>,

    /// 发给工人线程的活。
    work: Sender<Work>,

    /// 答复通道：本端放进 [`Work::LearnPhrase`]，工人线程答完写回。
    reply: Sender<LearnPhraseReply>,

    /// UI 线程 id：工人线程答完用它唤醒本线程。
    thread_id: u32,
}

impl LearnPhraseWindow {
    /// 建一个隐藏的窗口与控件，并登记到本线程（窗口过程要用）。
    pub(super) fn new(
        work: Sender<Work>,
        reply: Sender<LearnPhraseReply>,
        thread_id: u32,
    ) -> Result<Rc<Self>> {
        CLASS.ensure(|| WNDCLASSEXW {
            lpfnWndProc: Some(wndproc),
            hInstance: super::module_handle(),
            lpszClassName: CLASS_NAME,
            ..Default::default()
        })?;
        let dpi = unsafe { GetDpiForSystem() }.max(96);
        let scale = |px: i32| (px * dpi as i32) / 96;
        // 有标题栏、能关、能拖，但不进任务栏（TOOLWINDOW）。
        let hwnd = unsafe {
            CreateWindowExW(
                WS_EX_TOOLWINDOW,
                CLASS_NAME,
                w!("录入词组"),
                WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU,
                120,
                120,
                scale(WIDTH),
                scale(HEIGHT),
                None,
                None,
                Some(super::module_handle()),
                None,
            )?
        };
        // 控件形状按逻辑像素给，换算到物理像素；`id` 为 0 的是标签（不发 WM_COMMAND）。
        let control =
            |class: PCWSTR, text: PCWSTR, id: usize, style, rect: (i32, i32, i32, i32)| {
                let (x, y, w, h) = rect;
                let menu = (id != 0).then_some(HMENU(id as *mut core::ffi::c_void));
                unsafe {
                    CreateWindowExW(
                        WINDOW_EX_STYLE::default(),
                        class,
                        text,
                        style,
                        scale(x),
                        scale(y),
                        scale(w),
                        scale(h),
                        Some(hwnd),
                        menu,
                        Some(super::module_handle()),
                        None,
                    )
                }
                .unwrap_or_default()
            };
        let phrase = control(
            w!("EDIT"),
            w!(""),
            ID_PHRASE,
            WS_CHILD | WS_VISIBLE | WS_TABSTOP | WS_BORDER,
            (72, 16, 320, 24),
        );
        let pinyin = control(
            w!("EDIT"),
            w!(""),
            ID_PINYIN,
            WS_CHILD | WS_VISIBLE | WS_TABSTOP | WS_BORDER,
            (72, 52, 320, 24),
        );
        let _phrase_label = control(
            w!("STATIC"),
            w!("词组"),
            0,
            WS_CHILD | WS_VISIBLE,
            (16, 20, 48, 20),
        );
        let _pinyin_label = control(
            w!("STATIC"),
            w!("拼音"),
            0,
            WS_CHILD | WS_VISIBLE,
            (16, 56, 48, 20),
        );
        let hint = control(
            w!("STATIC"),
            w!(""),
            0,
            WS_CHILD | WS_VISIBLE,
            (16, 88, 380, 48),
        );
        let confirm = control(
            w!("BUTTON"),
            w!("录入"),
            ID_CONFIRM,
            WS_CHILD | WS_VISIBLE | WS_TABSTOP,
            (312, 152, 80, 30),
        );
        if phrase.is_invalid() || pinyin.is_invalid() || hint.is_invalid() || confirm.is_invalid() {
            let _ = unsafe { DestroyWindow(hwnd) };
            return Err(Error::from(E_FAIL));
        }
        let window = Rc::new(Self {
            hwnd,
            phrase,
            pinyin,
            hint,
            pinyin_dirty: Cell::new(false),
            setting: Cell::new(false),
            work,
            reply,
            thread_id,
        });
        WINDOW.with(|slot| *slot.borrow_mut() = Some(window.clone()));
        Ok(window)
    }

    /// 打开（菜单点的）：连着点两次只是提到前面，不清掉已经填了一半的内容。
    pub(super) fn open(&self) {
        if unsafe { IsWindowVisible(self.hwnd) }.as_bool() {
            let _ = unsafe { SetForegroundWindow(self.hwnd) };
            let _ = unsafe { SetFocus(Some(self.phrase)) };
            return;
        }
        self.set_text(self.phrase, "");
        self.set_text(self.pinyin, "");
        self.set_text(self.hint, "");
        self.pinyin_dirty.set(false);
        let _ = unsafe { ShowWindow(self.hwnd, SW_SHOW) };
        let _ = unsafe { SetForegroundWindow(self.hwnd) };
        let _ = unsafe { SetFocus(Some(self.phrase)) };
    }

    /// 工人线程的答复。
    pub(super) fn apply_reply(&self, reply: LearnPhraseReply) {
        match reply {
            LearnPhraseReply::Pinyin(Some(pinyin)) => {
                if !self.pinyin_dirty.get() {
                    self.set_text(self.pinyin, &pinyin);
                    self.set_text(self.hint, "");
                }
            }
            LearnPhraseReply::Pinyin(None) => {
                if !self.pinyin_dirty.get() {
                    self.set_text(self.hint, "词库里查不到这个词, 请自己填拼音");
                }
            }
            LearnPhraseReply::Learned => {
                let learned = self.text_of(self.phrase);
                self.set_text(self.phrase, "");
                self.set_text(self.pinyin, "");
                self.pinyin_dirty.set(false);
                self.set_text(self.hint, &format!("已录入「{learned}」"));
                let _ = unsafe { SetFocus(Some(self.phrase)) };
            }
            LearnPhraseReply::Error(message) => self.set_text(self.hint, &message),
        }
    }

    /// 词组框每变一下：拼音框没被手改过就按词库反查填进去。
    fn on_phrase_changed(&self) {
        let text = self.text_of(self.phrase);
        if text.trim().is_empty() {
            // 清空词组：解除「手改过」，下次重新反查。
            self.pinyin_dirty.set(false);
            self.set_text(self.hint, "");
            return;
        }
        if self.pinyin_dirty.get() {
            return;
        }
        if text.chars().any(char::is_whitespace) {
            self.set_text(self.hint, "词组里不要有空格");
            return;
        }
        self.ask(LearnPhraseWork::Suggest(text));
    }

    /// 点「录入」：拼音交给工人线程校验（音节数要对得上字数）。
    fn on_confirm(&self) {
        self.set_text(self.hint, "");
        self.ask(LearnPhraseWork::Confirm {
            text: self.text_of(self.phrase),
            pinyin: self.text_of(self.pinyin),
        });
    }

    /// 把一件活投给工人线程；答复回来时本线程会被唤醒（见 `ui::run`）。
    fn ask(&self, work: LearnPhraseWork) {
        let _ = self.work.send(Work::LearnPhrase {
            work,
            reply: self.reply.clone(),
            wake: self.thread_id,
        });
    }

    /// 填文本；顺带抑制这次改动产生的 `EN_CHANGE`。
    fn set_text(&self, control: HWND, text: &str) {
        self.setting.set(true);
        let wide: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
        let _ = unsafe { SetWindowTextW(control, PCWSTR(wide.as_ptr())) };
        self.setting.set(false);
    }

    fn text_of(&self, control: HWND) -> String {
        let len = unsafe { GetWindowTextLengthW(control) };
        let mut buffer = vec![0u16; len as usize + 1];
        let copied = unsafe { GetWindowTextW(control, &mut buffer) };
        String::from_utf16_lossy(&buffer[..copied as usize])
    }
}

impl Drop for LearnPhraseWindow {
    fn drop(&mut self) {
        WINDOW.with(|slot| *slot.borrow_mut() = None);
        let _ = unsafe { DestroyWindow(self.hwnd) };
    }
}

/// 本线程上的录入窗口（窗口过程读状态用）。
fn current() -> Option<Rc<LearnPhraseWindow>> {
    WINDOW.with(|slot| slot.borrow().clone())
}

unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        WM_COMMAND => {
            let id = wparam.0 & 0xFFFF;
            let code = ((wparam.0 >> 16) & 0xFFFF) as u32;
            if let Some(window) = current() {
                match (id, code) {
                    (ID_PHRASE, EN_CHANGE) => window.on_phrase_changed(),
                    (ID_PINYIN, EN_CHANGE) => {
                        // 程序填的拼音不算「手改」。
                        if !window.setting.get() {
                            window.pinyin_dirty.set(true);
                        }
                    }
                    (ID_CONFIRM, BN_CLICKED) => window.on_confirm(),
                    _ => {}
                }
            }
            LRESULT(0)
        }
        // 关掉只是藏起来：Engine 的活还在，下次从菜单点开接着用。
        WM_CLOSE => {
            let _ = unsafe { ShowWindow(hwnd, SW_HIDE) };
            LRESULT(0)
        }
        _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}

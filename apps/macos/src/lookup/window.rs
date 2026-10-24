//! 查字窗口: 一个输入框, 一个查询按钮, 一块结果.
//!
//! 窗口照「录入词组」做: 独立 NSWindow, 不跟光标跑, 也不打断正在打的拼音.
//! 输入一个汉字查详情, 输入几个部件 (如 `木木木`) 就按部件查字.
//! 网络在 [`super::fetch`] 里跑一次性线程, 这里用 0.1 秒的定时器等结果.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::{MainThreadMarker, MainThreadOnly, define_class, msg_send, sel};
use objc2_app_kit::{
    NSBorderType, NSButton, NSColor, NSFont, NSScrollView, NSTextField, NSTextView, NSView,
};
use objc2_foundation::{NSObject, NSObjectProtocol, NSPoint, NSRect, NSSize, NSString, NSTimer};

use super::fetch::{self, Entry, LookupError};
use super::panel::LookupPanel;
use super::target::LookupTarget;

/// 窗口尺寸与间距.
const WIDTH: f64 = 460.0;
const HEIGHT: f64 = 430.0;
const PAD: f64 = 20.0;
const ROW: f64 = 24.0;
const GAP: f64 = 12.0;
const LABEL_W: f64 = 44.0;
const BUTTON_W: f64 = 88.0;

/// 等结果的间隔.
const POLL_SECONDS: f64 = 0.1;

/// 一次查询最多几个部件: 再多站点也排不出可信的结果.
const MAX_PARTS: usize = 6;

/// 缓存多少个单字详情.
const CACHE_LIMIT: usize = 64;

/// 一次进行中的查询.
struct Job {
    /// 查询线程写这里.
    state: Arc<Mutex<Option<Outcome>>>,
    /// 查的是哪几个部件 (部件查字时用来回显).
    parts: Vec<char>,
    started: Instant,
}

/// 一次查询的结果.
enum Outcome {
    Detail(Result<Entry, LookupError>),
    Components(Result<Vec<char>, LookupError>),
}

/// 查字窗口.
pub struct LookupWindow {
    panel: Retained<LookupPanel>,
    input: Retained<NSTextField>,
    result: Retained<NSTextView>,
    status: Retained<NSTextField>,
    timer: Option<Retained<NSTimer>>,
    job: Option<Job>,
    /// 单字详情缓存: 同一个字反复查只请求一次.
    cache: HashMap<char, Entry>,
    /// 控件 target, 要和窗口活得一样久.
    _target: Retained<LookupTarget>,
}

impl LookupWindow {
    pub fn new(mtm: MainThreadMarker) -> Self {
        let target = LookupTarget::new(mtm);
        let panel = LookupPanel::new(mtm, NSRect::new(NSPoint::ZERO, NSSize::new(WIDTH, HEIGHT)));
        panel.setTitle(&NSString::from_str("查字"));

        let content = NSView::initWithFrame(
            mtm.alloc(),
            NSRect::new(NSPoint::ZERO, NSSize::new(WIDTH, HEIGHT)),
        );

        let input = NSTextField::initWithFrame(mtm.alloc(), NSRect::ZERO);
        input.setBezeled(true);
        input.setEditable(true);
        input.setSelectable(true);
        input.setDrawsBackground(true);
        input.setUsesSingleLineMode(true);
        input.setPlaceholderString(Some(&NSString::from_str(
            "一个字查详情, 几个部件查字 (如 木木木)",
        )));
        // 输入框里回车直接查
        unsafe {
            input.setTarget(Some(&target));
            input.setAction(Some(sel!(query:)));
        }

        let label = NSTextField::labelWithString(&NSString::from_str("查字"), mtm);
        label.setFrame(NSRect::new(
            NSPoint::new(PAD, y_from_top(0.0)),
            NSSize::new(LABEL_W, ROW),
        ));
        let field_x = PAD + LABEL_W + 8.0;
        input.setFrame(NSRect::new(
            NSPoint::new(field_x, y_from_top(0.0)),
            NSSize::new(WIDTH - field_x - PAD, ROW),
        ));
        content.addSubview(&label);
        content.addSubview(&input);

        let query = unsafe {
            NSButton::buttonWithTitle_target_action(
                &NSString::from_str("查询"),
                Some(&target),
                Some(sel!(query:)),
                mtm,
            )
        };
        query.setKeyEquivalent(&NSString::from_str("\r"));
        let close = unsafe {
            NSButton::buttonWithTitle_target_action(
                &NSString::from_str("关闭"),
                Some(&target),
                Some(sel!(closeWindow:)),
                mtm,
            )
        };
        close.setKeyEquivalent(&NSString::from_str("\u{1b}"));
        let button_y = y_from_top(ROW + GAP);
        query.setFrame(NSRect::new(
            NSPoint::new(field_x, button_y),
            NSSize::new(BUTTON_W, ROW),
        ));
        close.setFrame(NSRect::new(
            NSPoint::new(field_x + BUTTON_W + 10.0, button_y),
            NSSize::new(BUTTON_W, ROW),
        ));
        content.addSubview(&query);
        content.addSubview(&close);

        let result_top = button_y - GAP;
        let result_bottom = PAD + 18.0 + GAP;
        let result_size = NSSize::new(WIDTH - 2.0 * PAD, result_top - result_bottom);

        let scroll = NSScrollView::initWithFrame(
            mtm.alloc(),
            NSRect::new(NSPoint::new(PAD, result_bottom), result_size),
        );
        scroll.setHasVerticalScroller(true);
        scroll.setBorderType(NSBorderType::BezelBorder);
        let result = NSTextView::initWithFrame(
            mtm.alloc(),
            NSRect::new(
                NSPoint::ZERO,
                NSSize::new(result_size.width, result_size.height),
            ),
        );
        result.setEditable(false);
        result.setSelectable(true);
        result.setFont(Some(&NSFont::systemFontOfSize(12.0)));
        result.setVerticallyResizable(true);
        result.setHorizontallyResizable(false);
        if let Some(container) = unsafe { result.textContainer() } {
            container.setWidthTracksTextView(true);
        }
        scroll.setDocumentView(Some(&result));
        content.addSubview(&scroll);

        let status = NSTextField::labelWithString(&NSString::from_str(""), mtm);
        status.setFont(Some(&NSFont::systemFontOfSize(11.0)));
        status.setFrame(NSRect::new(
            NSPoint::new(PAD, PAD),
            NSSize::new(WIDTH - 2.0 * PAD, 18.0),
        ));
        content.addSubview(&status);

        panel.setContentView(Some(&content));
        Self {
            panel,
            input,
            result,
            status,
            timer: None,
            job: None,
            cache: HashMap::new(),
            _target: target,
        }
    }

    /// 打开 (或带到最前) 并把光标放进输入框.
    pub fn show(&self) {
        self.panel.present();
        self.panel.makeFirstResponder(Some(&self.input));
        self.set_status(
            "输入一个汉字查详情, 或 2 到 6 个部件查字 (如 木木木)",
            false,
        );
    }

    pub fn close(&self) {
        self.panel.close();
    }

    /// 输入框里的原文.
    pub fn input_text(&self) -> String {
        self.input.stringValue().to_string()
    }

    /// 按当前的输入发起查询.
    pub fn query(&mut self) {
        let han = fetch::han_chars(&self.input_text());
        match han.len() {
            0 => self.set_status("输入一个汉字查详情, 或 2 到 6 个部件查字 (如 木木木)", true),
            1 => self.start_detail(han[0]),
            count if count <= MAX_PARTS => self.start_components(&han),
            _ => self.set_status("最多 6 个部件", true),
        }
    }

    /// 定时器到点: 结果到了就显示, 超时就报错.
    pub fn poll(&mut self) {
        let Some(job) = self.job.as_ref() else {
            return;
        };
        let done = match job.state.lock() {
            Ok(mut slot) => slot.take(),
            Err(_) => Some(Outcome::Detail(Err(LookupError(
                "查询线程出错了".to_owned(),
            )))),
        };
        let parts = job.parts.clone();
        let elapsed = job.started.elapsed();
        if let Some(outcome) = done {
            self.job = None;
            self.stop_timer();
            match outcome {
                Outcome::Detail(Ok(entry)) if !entry.is_empty() => {
                    self.remember(entry.zi, entry.clone());
                    self.set_result(&entry.render());
                    self.set_status("", false);
                }
                Outcome::Detail(Ok(_)) => {
                    self.set_status("这个字在 zi.tools 上没有可显示的内容", true)
                }
                Outcome::Detail(Err(error)) => self.set_status(&format!("查询失败: {error}"), true),
                Outcome::Components(Ok(found)) if !found.is_empty() => {
                    self.set_result(&components_text(&parts, &found));
                    self.set_status("", false);
                }
                Outcome::Components(Ok(_)) => self.set_status("没有找到含这几个部件的字", true),
                Outcome::Components(Err(error)) => {
                    self.set_status(&format!("查询失败: {error}"), true)
                }
            }
            return;
        }
        if elapsed > Duration::from_secs(7) {
            self.job = None;
            self.stop_timer();
            self.set_status("查询超时, 站点没有回应", true);
        }
    }

    /// 查一个字的详情: 命中缓存立刻显示, 否则起一个线程.
    fn start_detail(&mut self, zi: char) {
        self.stop_timer();
        if let Some(entry) = self.cache.get(&zi).cloned() {
            self.set_result(&entry.render());
            self.set_status("", false);
            return;
        }
        self.set_result("");
        self.set_status("查询中…", false);
        self.job = Some(spawn_job(
            move || Outcome::Detail(fetch::detail(zi)),
            Vec::new(),
        ));
        self.start_timer();
    }

    /// 按部件查字.
    fn start_components(&mut self, parts: &[char]) {
        self.stop_timer();
        let parts = parts.to_vec();
        self.set_result("");
        self.set_status("查询中…", false);
        let query = parts.clone();
        self.job = Some(spawn_job(
            move || Outcome::Components(fetch::components(&query)),
            parts,
        ));
        self.start_timer();
    }

    fn remember(&mut self, zi: char, entry: Entry) {
        if self.cache.len() >= CACHE_LIMIT {
            self.cache.clear();
        }
        self.cache.insert(zi, entry);
    }

    fn set_result(&self, text: &str) {
        self.result.setString(&NSString::from_str(text));
    }

    fn set_status(&self, text: &str, error: bool) {
        let color = if error {
            NSColor::systemRedColor()
        } else {
            NSColor::secondaryLabelColor()
        };
        self.status.setTextColor(Some(&color));
        self.status.setStringValue(&NSString::from_str(text));
    }

    fn start_timer(&mut self) {
        self.stop_timer();
        let mtm = MainThreadMarker::new().expect("查字窗口只在主线程用");
        let target = LookupTicker::new(mtm);
        let timer = unsafe {
            NSTimer::scheduledTimerWithTimeInterval_target_selector_userInfo_repeats(
                POLL_SECONDS,
                &target,
                sel!(tick:),
                None,
                true,
            )
        };
        self.timer = Some(timer);
    }

    fn stop_timer(&mut self) {
        if let Some(timer) = self.timer.take() {
            timer.invalidate();
        }
    }
}

/// 起一个一次性线程, 结果写进共享格子; 线程结束由 [`LookupWindow::poll`] 收.
fn spawn_job(work: impl FnOnce() -> Outcome + Send + 'static, parts: Vec<char>) -> Job {
    let state: Arc<Mutex<Option<Outcome>>> = Arc::new(Mutex::new(None));
    let shared = Arc::clone(&state);
    std::thread::spawn(move || {
        let outcome = work();
        if let Ok(mut slot) = shared.lock() {
            *slot = Some(outcome);
        }
    });
    Job {
        state,
        parts,
        started: Instant::now(),
    }
}

/// 部件查字的结果: 回显部件, 再按行列出命中的字 (每行 8 个).
fn components_text(parts: &[char], found: &[char]) -> String {
    let parts = parts
        .iter()
        .map(|c| c.to_string())
        .collect::<Vec<_>>()
        .join(" ");
    let mut out = format!(
        "部件  {parts}\n命中  {} 个字 (来源 zi.tools 字統网)\n\n",
        found.len()
    );
    for (index, zi) in found.iter().enumerate() {
        out.push(*zi);
        if (index + 1) % 8 == 0 {
            out.push('\n');
        } else {
            out.push_str("  ");
        }
    }
    out
}

fn y_from_top(top: f64) -> f64 {
    HEIGHT - PAD - ROW - top
}

define_class!(
    // SAFETY: NSObject 没有子类化要求; 没有实现 Drop.
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[ivars = ()]
    /// 等结果的定时器: 到点看一眼查询线程写完没有.
    struct LookupTicker;

    impl LookupTicker {
        #[unsafe(method(tick:))]
        fn tick(&self, _timer: Option<&AnyObject>) {
            crate::host::with(|h| h.lookup.poll());
        }
    }

    unsafe impl NSObjectProtocol for LookupTicker {}
);

impl LookupTicker {
    fn new(mtm: MainThreadMarker) -> Retained<Self> {
        let this = mtm.alloc::<Self>().set_ivars(());
        unsafe { msg_send![super(this), init] }
    }
}

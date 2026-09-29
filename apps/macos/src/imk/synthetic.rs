//! 收不下写回文字的 client（Zen 的 native popover 面板等，见 [`super::TextClient::rejects_text`]）的上屏兜底：
//! 不走 IMK 的 `insertText:`，改成往系统里发带 Unicode 文本的合成键盘事件。
//!
//! 合成事件与真实按键走同一条路：窗口服务器 → 应用的 `keyDown:` → `interpretKeyEvents:` → 又回到青简的
//! `handleEvent:client:`。青简认出这是自己发的（[`take_echo`]）就原样放行，应用自己插字，
//! 系统 ABC 在这类面板里能打字走的正是这条路。
//!
//! 一次按键里要上屏的文字先攒进发件箱（[`queue_text`]），按键分发完再一起发（[`flush`]）：
//! 这一键如果本来要交还应用（组句中敲 `!`、回车、⌘V），也改成吞掉、排在文字后面重发，
//! 否则应用先收到这一键、后收到异步到达的文字，顺序就反了。
//!
//! 发事件要辅助功能权限（[`can_post`]）；没有权限时退回 `insertText:`，行为与修复前相同（字出不来）。

use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::ffi::c_void;
use std::time::{Duration, Instant};

type CGEventRef = *mut c_void;
type CGEventSourceRef = *mut c_void;

/// `kCGEventSourceStatePrivate`：合成事件不继承物理键盘此刻按着的修饰键。
const SOURCE_STATE_PRIVATE: i32 = -1;
/// `kCGSessionEventTap`：从登录会话这一层注入，和用户按键一样进前台应用。
const SESSION_EVENT_TAP: u32 = 1;
/// 一个合成事件最多带多少个 UTF-16 码元：`CGEventKeyboardSetUnicodeString` 超过 20 个会被截断。
const MAX_UNITS_PER_EVENT: usize = 20;
/// 发出去的事件多久没回到青简就不再等：没权限被系统丢掉、或应用没把它交给输入法时，别让它挡住后面的真按键。
const ECHO_TIMEOUT: Duration = Duration::from_secs(1);

#[link(name = "ApplicationServices", kind = "framework")]
unsafe extern "C" {
    fn CGEventSourceCreate(state: i32) -> CGEventSourceRef;
    fn CGEventCreateKeyboardEvent(
        source: CGEventSourceRef,
        key_code: u16,
        key_down: bool,
    ) -> CGEventRef;
    fn CGEventKeyboardSetUnicodeString(event: CGEventRef, length: usize, string: *const u16);
    fn CGEventSetFlags(event: CGEventRef, flags: u64);
    fn CGEventPost(tap: u32, event: CGEventRef);
    fn CGPreflightPostEventAccess() -> bool;
    fn CGRequestPostEventAccess() -> bool;
}

#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    fn CFRelease(object: *const c_void);
}

/// 一个发出去、等着回到青简的事件。
enum Echo {
    /// 带文字的合成事件，回来时 `characters` 就是这段文字。
    Text(String, Instant),
    /// 重发的原按键（回车、⌘V 这类），按键码认。
    Key(u16, Instant),
}

impl Echo {
    fn sent_at(&self) -> Instant {
        match self {
            Self::Text(_, at) | Self::Key(_, at) => *at,
        }
    }
}

thread_local! {
    /// 这一键面对的 client 收不收得下文字；组句中沿用组句开头那一键的判断（见 [`rejects_for`]）。
    static REJECTS: Cell<bool> = const { Cell::new(false) };
    /// 这一键要上屏、还没发的文字。
    static OUTBOX: RefCell<String> = const { RefCell::new(String::new()) };
    /// 发出去、还没回来的事件，按发出顺序。
    static ECHOES: RefCell<VecDeque<Echo>> = const { RefCell::new(VecDeque::new()) };
    /// 要排在文字之后重发的原按键（键码与修饰键）。
    static PASSTHROUGH: Cell<Option<(u16, u64)>> = const { Cell::new(None) };
    /// 这个进程里是否已经请求过一次权限（系统弹窗只弹一次就够）。
    static REQUESTED: Cell<bool> = const { Cell::new(false) };
}

/// 这一键面对的 client 收不收得下文字。组句开头（缓冲区空）时现问一次 client（两次同步 IPC），
/// 组句中沿用那次的结果：组句里 client 不会变（失焦会先结束组句），每键都问太费。
pub fn rejects_for(composing: bool, ask: impl FnOnce() -> bool) -> bool {
    if !composing {
        let rejects = ask();
        if rejects != REJECTS.get() {
            tracing::info!(
                rejects,
                "输入框收不下写回的文字 (native popover 等), 上屏改走合成键盘事件"
            );
        }
        REJECTS.set(rejects);
    }
    REJECTS.get()
}

/// 这是不是青简自己发出、刚回来的事件。是就从队列里取掉，调用方应当原样放行。
/// 过期的（等了超过 [`ECHO_TIMEOUT`]）顺手丢掉。
pub fn take_echo(key_code: u16, characters: Option<&str>) -> bool {
    ECHOES.with_borrow_mut(|echoes| {
        while let Some(front) = echoes.front() {
            let matched = match front {
                Echo::Text(text, _) => characters == Some(text.as_str()),
                Echo::Key(code, _) => *code == key_code,
            };
            if matched {
                echoes.pop_front();
                return true;
            }
            if front.sent_at().elapsed() < ECHO_TIMEOUT {
                return false;
            }
            tracing::warn!("合成事件没有回到输入法，丢弃");
            echoes.pop_front();
        }
        false
    })
}

/// 攒一段要上屏的文字，按键分发完由 [`flush`] 发出。
pub fn queue_text(text: &str) {
    OUTBOX.with_borrow_mut(|outbox| outbox.push_str(text));
}

/// 发件箱里有没有文字。
pub fn has_pending() -> bool {
    OUTBOX.with_borrow(|outbox| !outbox.is_empty())
}

/// 把这一键本来要交还应用的按键排到文字后面：可打印字符（不带 ⌘ / ⌃）直接并进文字，
/// 其他键（回车、方向键、⌘V）按原键码与修饰键重发。
pub fn queue_passthrough(key_code: u16, characters: Option<&str>, flags: u64, printable: bool) {
    match characters {
        Some(text) if printable => queue_text(text),
        _ => PASSTHROUGH.set(Some((key_code, flags))),
    }
}

/// 能不能发合成键盘事件（有没有辅助功能权限）。没有时第一次顺手请求一次（系统弹窗是异步的，不等结果），
/// 调用方退回 `insertText:`。
pub fn can_post() -> bool {
    // SAFETY: 无参数的纯查询函数。
    if unsafe { CGPreflightPostEventAccess() } {
        return true;
    }
    if !REQUESTED.replace(true) {
        tracing::warn!("没有辅助功能权限，发不了合成键盘事件，请求一次权限");
        // SAFETY: 无参数。
        unsafe { CGRequestPostEventAccess() };
    }
    false
}

/// 把发件箱里的文字（以及排在后面的原按键）作为合成键盘事件发出去；什么都没攒就什么都不做。
/// 只在 [`can_post`] 为真时往发件箱里放东西，这里不再查权限。
pub fn flush() {
    let text = OUTBOX.with_borrow_mut(std::mem::take);
    let passthrough = PASSTHROUGH.take();
    if text.is_empty() && passthrough.is_none() {
        return;
    }
    // SAFETY: 创建失败返回 NULL，下面各函数都接受 NULL source（按缺省来源）。
    let source = unsafe { CGEventSourceCreate(SOURCE_STATE_PRIVATE) };
    let units: Vec<u16> = text.encode_utf16().collect();
    let mut sent = 0;
    for chunk in utf16_chunks(&units) {
        let chunk_text = String::from_utf16_lossy(chunk);
        // 先登记再发：事件可能在 CGEventPost 返回前就被派发回来
        ECHOES.with_borrow_mut(|echoes| echoes.push_back(Echo::Text(chunk_text, Instant::now())));
        post_text(source, chunk);
        sent += chunk.len();
    }
    if let Some((code, flags)) = passthrough {
        ECHOES.with_borrow_mut(|echoes| echoes.push_back(Echo::Key(code, Instant::now())));
        // SAFETY: source 可为 NULL；返回的事件用完即释放。
        unsafe {
            let event = CGEventCreateKeyboardEvent(source, code, true);
            if !event.is_null() {
                CGEventSetFlags(event, flags);
                CGEventPost(SESSION_EVENT_TAP, event);
                CFRelease(event);
            }
        }
    }
    if !source.is_null() {
        // SAFETY: CGEventSourceCreate 返回的是 +1 引用。
        unsafe { CFRelease(source) };
    }
    tracing::debug!(units = sent, key = ?passthrough.map(|(code, _)| code), "合成键盘事件上屏");
}

/// 发一对带文字的按下 / 抬起事件。键码随便给一个（0 是 A 键），应用插的是事件里的文字；修饰键清零，
/// 免得物理键盘上还按着的 ⇧ / ⌘ 把它变成快捷键。
fn post_text(source: CGEventSourceRef, units: &[u16]) {
    for down in [true, false] {
        // SAFETY: source 可为 NULL；units 在调用期间有效；事件用完即释放。
        unsafe {
            let event = CGEventCreateKeyboardEvent(source, 0, down);
            if event.is_null() {
                continue;
            }
            CGEventKeyboardSetUnicodeString(event, units.len(), units.as_ptr());
            CGEventSetFlags(event, 0);
            CGEventPost(SESSION_EVENT_TAP, event);
            CFRelease(event);
        }
    }
}

/// 按 [`MAX_UNITS_PER_EVENT`] 切 UTF-16，不把代理对（emoji 等）切成两半。
fn utf16_chunks(units: &[u16]) -> Vec<&[u16]> {
    let mut chunks = Vec::new();
    let mut start = 0;
    while start < units.len() {
        let mut end = (start + MAX_UNITS_PER_EVENT).min(units.len());
        // 切点落在高位代理之后（低位代理之前）就往前退一格
        if end < units.len() && (0xDC00..=0xDFFF).contains(&units[end]) {
            end -= 1;
        }
        chunks.push(&units[start..end]);
        start = end;
    }
    chunks
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chunks_keep_surrogate_pairs_whole() {
        // 19 个 ASCII + 一个 emoji（代理对）：第一块不能在 emoji 中间断开
        let text = format!("{}😀tail", "a".repeat(19));
        let units: Vec<u16> = text.encode_utf16().collect();
        let chunks = utf16_chunks(&units);
        assert!(chunks.iter().all(|c| c.len() <= MAX_UNITS_PER_EVENT));
        let joined: String = chunks
            .iter()
            .map(|c| String::from_utf16(c).expect("每块都是完整的 UTF-16"))
            .collect();
        assert_eq!(joined, text);
    }
}

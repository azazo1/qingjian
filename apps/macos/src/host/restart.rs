//! 菜单里的「重启输入法」：落盘学习数据，然后把当前进程正常结束掉。
//!
//! 输入法进程由系统按需拉起（`apps/macos/scripts/bundle.sh --install` 里的 `pkill -x qingjian-macos`
//! 是同一个用法），所以重启就是把进程结束掉，系统在下次激活青简时起一个新的。
//!
//! 走 `NSApplication::stop` 而不是 `std::process::exit`：前者让 `main` 的 run loop 正常返回，
//! 日志的 `WorkerGuard` 才会析构、缓冲的日志才落盘（见 `app/logging`）。

use objc2::MainThreadMarker;
use objc2_app_kit::NSApplication;

use super::Host;

impl Host {
    /// 重启输入法进程。菜单动作与输入源菜单的回调都在主线程，所以这里可以直接拿 `NSApplication`。
    pub fn restart(&mut self) {
        tracing::info!("重启输入法：落盘学习数据后结束进程");
        // 先停掉定时器与在途请求，免得进程在收尾期间还在等云端的回包
        self.cancel_prediction();
        self.watch.stop();
        self.engine.flush_learning();
        // 候选窗是独立 NSPanel，不随 run loop 退出自动收，先自己收掉，免得屏幕上留一个孤儿窗口
        self.window.hide();
        let mtm = MainThreadMarker::new().expect("菜单动作在主线程");
        NSApplication::sharedApplication(mtm).stop(None);
    }
}

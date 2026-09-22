//! 本地整句模型的三个定时器：等后台加载接上、停键后的防抖（到点才把整句路径送去后台打分）与结果轮询（到了就重画，平时不占 CPU）。

use std::time::{Duration, Instant};

use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::{MainThreadMarker, MainThreadOnly, define_class, msg_send, sel};
use objc2_foundation::{NSObject, NSObjectProtocol, NSTimer};

/// 停键多久才请求重排：比一般的击键间隔短，连着敲时不请求。
const DEBOUNCE: f64 = 0.08;

/// 轮询间隔：模型一次二三十毫秒。
const POLL_INTERVAL: f64 = 0.02;

/// 最长等多久; 后台线程卡住时兜底. 本地整句模型二三十毫秒就回, 用这个缺省值;
/// 接决策模型 (云端一次判断要几秒) 时按它的请求超时放宽, 见 [`RescoreMonitor::set_max_wait`].
pub(super) const DEFAULT_MAX_WAIT: Duration = Duration::from_secs(2);

/// 等模型加载的间隔：加载要几百毫秒到几秒，接上晚几十毫秒无妨。
const LOAD_INTERVAL: f64 = 0.1;

pub struct RescoreMonitor {
    /// 等模型加载的定时器；没在加载为 `None`。
    loading: Option<Retained<NSTimer>>,

    /// 防抖定时器（一次性）；没在等为 `None`。
    debounce: Option<Retained<NSTimer>>,

    /// 轮询定时器；没在等结果为 `None`。
    poll: Option<Retained<NSTimer>>,

    /// 本轮开始等结果的时间。
    since: Option<Instant>,

    /// 本轮最多等多久 (见 [`DEFAULT_MAX_WAIT`]): 打分器慢的时候, 等过它就不再收结果.
    max_wait: Duration,

    mtm: MainThreadMarker,
}

impl RescoreMonitor {
    pub fn new(mtm: MainThreadMarker) -> Self {
        Self {
            loading: None,
            debounce: None,
            poll: None,
            since: None,
            max_wait: DEFAULT_MAX_WAIT,
            mtm,
        }
    }

    /// 换等待上限: 本地整句模型用缺省的两秒, 决策模型按它的请求超时放宽.
    /// 换打分器时都要调一次 (见 `Host::apply_rescorer`), 否则云端的结果到了壳已经不听了.
    pub(super) fn set_max_wait(&mut self, max_wait: Duration) {
        self.max_wait = max_wait;
    }

    /// 模型在后台加载：定时看一眼接上没有。按键路径也会看，但用户停键后没人再看，接上的那一刻就没人知道。
    pub fn watch_loading(&mut self) {
        if self.loading.is_some() {
            return;
        }
        let target = RescoreTicker::new(self.mtm);
        let timer = unsafe {
            NSTimer::scheduledTimerWithTimeInterval_target_selector_userInfo_repeats(
                LOAD_INTERVAL,
                &target,
                sel!(attach:),
                None,
                true,
            )
        };
        self.loading = Some(timer);
    }

    /// 加载有结果了（接上、失败或卸掉）：不再看。
    pub fn stop_watching(&mut self) {
        if let Some(timer) = self.loading.take() {
            timer.invalidate();
        }
    }

    /// 又敲了一键：重新计时。
    pub fn schedule(&mut self) {
        if let Some(timer) = self.debounce.take() {
            timer.invalidate();
        }
        let target = RescoreTicker::new(self.mtm);
        let timer = unsafe {
            NSTimer::scheduledTimerWithTimeInterval_target_selector_userInfo_repeats(
                DEBOUNCE,
                &target,
                sel!(fire:),
                None,
                false,
            )
        };
        self.debounce = Some(timer);
    }

    /// 请求已发出：开始轮询结果。
    pub fn start_polling(&mut self) {
        self.since = Some(Instant::now());
        if self.poll.is_some() {
            return;
        }
        let target = RescoreTicker::new(self.mtm);
        let timer = unsafe {
            NSTimer::scheduledTimerWithTimeInterval_target_selector_userInfo_repeats(
                POLL_INTERVAL,
                &target,
                sel!(poll:),
                None,
                true,
            )
        };
        self.poll = Some(timer);
    }

    /// 停下两个定时器。
    pub fn stop(&mut self) {
        if let Some(timer) = self.debounce.take() {
            timer.invalidate();
        }
        if let Some(timer) = self.poll.take() {
            timer.invalidate();
        }
        self.since = None;
    }

    pub fn expired(&self) -> bool {
        self.since.is_some_and(|since| since.elapsed() > self.max_wait)
    }
}

define_class!(
    // SAFETY: NSObject 没有子类化要求；没有实现 Drop。
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[ivars = ()]
    struct RescoreTicker;

    impl RescoreTicker {
        #[unsafe(method(attach:))]
        fn attach(&self, _timer: Option<&AnyObject>) {
            crate::host::with(|h| h.attach_loaded_model());
        }

        #[unsafe(method(fire:))]
        fn fire(&self, _timer: Option<&AnyObject>) {
            crate::host::with(|h| h.start_rescoring());
        }

        #[unsafe(method(poll:))]
        fn poll(&self, _timer: Option<&AnyObject>) {
            crate::host::with(|h| h.poll_rescoring());
        }
    }

    unsafe impl NSObjectProtocol for RescoreTicker {}
);

impl RescoreTicker {
    fn new(mtm: MainThreadMarker) -> Retained<Self> {
        let this = mtm.alloc::<Self>().set_ivars(());
        unsafe { msg_send![super(this), init] }
    }
}

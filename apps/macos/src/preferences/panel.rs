use objc2::rc::Retained;
use objc2::{MainThreadMarker, MainThreadOnly, define_class, msg_send};
use objc2_app_kit::{
    NSApplication, NSApplicationActivationPolicy, NSBackingStoreType, NSWindow,
    NSWindowCollectionBehavior, NSWindowStyleMask,
};
use objc2_foundation::{NSObjectProtocol, NSRect};

/// 把一块 Accessory 窗口摆到用户当前看着的 Space 上, 并带到最前.
///
/// 偏好设置 / 录入词组 / 查字这三块窗口都在输入法启动时一次建好, 之后反复显示; 而 macOS 只把普通窗口
/// 绑到它**创建时**那个 Space. 不声明 collection behavior 的话, 在别的 Space 里打开会先把用户拽回
/// 窗口所在的那个 Space, 再激活应用. 所以显示前声明 MoveToActiveSpace 让窗口自己挪过来, 并且
/// **先排前再激活应用**: 顺序反过来, 系统在激活那一刻就把 Space 切走了.
///
/// FullScreenAuxiliary 是为了在全屏应用 (自成一个 Space) 里也能就地打开, 而不是跳去桌面 Space.
pub(crate) fn present_accessory(window: &NSWindow) {
    let mtm = MainThreadMarker::from(window);
    window.setCollectionBehavior(
        NSWindowCollectionBehavior::MoveToActiveSpace
            | NSWindowCollectionBehavior::FullScreenAuxiliary,
    );
    if window.isVisible() && !window.isOnActiveSpace() {
        // 已经开着且在别的 Space: 收起再排前才会重新做一次挪动
        window.orderOut(None);
    }
    window.orderFrontRegardless();
    enter_accessory(mtm);
    window.makeKeyAndOrderFront(None);
    tracing::debug!(
        on_active_space = window.isOnActiveSpace(),
        "Accessory 窗口已排到当前 Space"
    );
}

/// 输入法是后台进程, 文本框要焦点必须临时切到 Accessory, 并装一份编辑菜单好让 ⌘V 有人接.
pub(crate) fn enter_accessory(mtm: MainThreadMarker) {
    let app = NSApplication::sharedApplication(mtm);
    super::edit_menu::install(mtm);
    app.setActivationPolicy(NSApplicationActivationPolicy::Accessory);
    #[allow(deprecated)]
    app.activateIgnoringOtherApps(true);
}

/// 关掉当前 Accessory 窗口之后: 偏好设置或录入词组还开着就保持 Accessory, 否则回到纯后台.
pub(crate) fn leave_accessory(mtm: MainThreadMarker) {
    let app = NSApplication::sharedApplication(mtm);
    let keep = app.windows().iter().any(|window| {
        window.isVisible()
            && window
                .styleMask()
                .contains(NSWindowStyleMask::Titled | NSWindowStyleMask::Closable)
    });
    if !keep {
        app.setActivationPolicy(NSApplicationActivationPolicy::Prohibited);
    }
}

define_class!(
    // SAFETY: NSWindow 允许子类化；没有实现 Drop。
    #[unsafe(super(NSWindow))]
    #[thread_kind = MainThreadOnly]
    #[ivars = ()]
    /// 设置窗口的 NSWindow：关窗时把进程的激活策略切回 Prohibited，让输入法回到纯后台。
    pub struct PreferencesPanel;

    impl PreferencesPanel {
        #[unsafe(method(close))]
        fn close(&self) {
            let mtm = MainThreadMarker::from(self);
            let _: () = unsafe { msg_send![super(self), close] };
            crate::preferences::leave_accessory(mtm);
        }
    }

    unsafe impl NSObjectProtocol for PreferencesPanel {}
);

impl PreferencesPanel {
    pub fn new(mtm: MainThreadMarker, content: NSRect) -> Retained<Self> {
        let this = mtm.alloc::<Self>().set_ivars(());
        let this: Retained<Self> = unsafe {
            msg_send![
                super(this),
                initWithContentRect: content,
                styleMask: NSWindowStyleMask::Titled | NSWindowStyleMask::Closable,
                backing: NSBackingStoreType::Buffered,
                defer: false,
            ]
        };
        // 程序建的 NSWindow 默认关窗即释放，我们还握着 Retained，必须关掉
        unsafe { this.setReleasedWhenClosed(false) };
        this
    }

    /// 切到 Accessory（有窗口、无 Dock 图标）并把窗口带到最前，文本框才拿得到键盘焦点。
    /// 先让窗口跟到用户当前看着的 Space, 再激活应用, 否则会把用户拽到窗口原来所在的 Space.
    pub fn present(&self) {
        present_accessory(self);
    }
}

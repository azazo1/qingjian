use objc2::rc::Retained;
use objc2::{MainThreadMarker, MainThreadOnly, define_class, msg_send};
use objc2_app_kit::{
    NSApplication, NSApplicationActivationPolicy, NSBackingStoreType, NSWindow, NSWindowStyleMask,
};
use objc2_foundation::{NSObjectProtocol, NSRect};

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
    pub fn present(&self) {
        enter_accessory(MainThreadMarker::from(self));
        self.makeKeyAndOrderFront(None);
    }
}

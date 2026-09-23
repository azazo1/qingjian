use objc2::rc::Retained;
use objc2::{MainThreadMarker, MainThreadOnly, define_class, msg_send};
use objc2_app_kit::{NSBackingStoreType, NSWindow, NSWindowStyleMask};
use objc2_foundation::{NSObjectProtocol, NSRect};

use crate::preferences::enter_accessory;

define_class!(
    // SAFETY: NSWindow 允许子类化; 没有实现 Drop.
    #[unsafe(super(NSWindow))]
    #[thread_kind = MainThreadOnly]
    #[ivars = ()]
    /// 录入词组窗口: 关窗时若没有别的 Accessory 窗口还开着, 把激活策略切回 Prohibited.
    pub struct LearnPhrasePanel;

    impl LearnPhrasePanel {
        #[unsafe(method(close))]
        fn close(&self) {
            let mtm = MainThreadMarker::from(self);
            let _: () = unsafe { msg_send![super(self), close] };
            crate::preferences::leave_accessory(mtm);
        }
    }

    unsafe impl NSObjectProtocol for LearnPhrasePanel {}
);

impl LearnPhrasePanel {
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
        unsafe { this.setReleasedWhenClosed(false) };
        this
    }

    pub fn present(&self) {
        let mtm = MainThreadMarker::from(self);
        enter_accessory(mtm);
        self.center();
        self.makeKeyAndOrderFront(None);
    }
}

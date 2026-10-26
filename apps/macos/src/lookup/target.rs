//! 查字窗口的按钮与输入框 target: 只把事件转给 Host.

use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::{MainThreadMarker, MainThreadOnly, define_class, msg_send};
use objc2_foundation::{NSObject, NSObjectProtocol};

use crate::host;

define_class!(
    // SAFETY: NSObject 没有子类化要求; 没有实现 Drop. 回调只转发到 Host.
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[ivars = ()]
    /// 查字窗口的查询按钮与输入框 (回车) target.
    pub struct LookupTarget;

    impl LookupTarget {
        #[unsafe(method(query:))]
        fn query(&self, _sender: Option<&AnyObject>) {
            host::with(|h| h.lookup.query());
        }

        #[unsafe(method(closeWindow:))]
        fn close_window(&self, _sender: Option<&AnyObject>) {
            host::with(|h| h.lookup.close());
        }

        /// 双击列表里的一行: 复制这个字.
        #[unsafe(method(copySelected:))]
        fn copy_selected(&self, _sender: Option<&AnyObject>) {
            host::with(|h| h.lookup.copy_selected());
        }
    }

    unsafe impl NSObjectProtocol for LookupTarget {}
);

impl LookupTarget {
    pub fn new(mtm: MainThreadMarker) -> Retained<Self> {
        let this = mtm.alloc::<Self>().set_ivars(());
        unsafe { msg_send![super(this), init] }
    }
}

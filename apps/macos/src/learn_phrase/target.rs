use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::{MainThreadMarker, MainThreadOnly, define_class, msg_send};
use objc2_app_kit::{NSControlTextEditingDelegate, NSTextField, NSTextFieldDelegate};
use objc2_foundation::{NSNotification, NSObject, NSObjectProtocol};

use crate::host;

use super::window::{PHRASE_TAG, PINYIN_TAG};

define_class!(
    // SAFETY: NSObject 没有子类化要求; 没有实现 Drop. 回调只转发到 Host.
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[ivars = ()]
    /// 录入词组窗口的按钮和文本框 target.
    pub struct LearnPhraseTarget;

    impl LearnPhraseTarget {
        #[unsafe(method(confirm:))]
        fn confirm(&self, _sender: Option<&AnyObject>) {
            host::with(|h| h.confirm_learn_phrase());
        }

        #[unsafe(method(cancel:))]
        fn cancel(&self, _sender: Option<&AnyObject>) {
            host::with(|h| h.learn_phrase.close());
        }

        #[unsafe(method(controlTextDidChange:))]
        fn text_did_change(&self, notification: &NSNotification) {
            let Some(object) = notification.object() else {
                return;
            };
            let Some(field) = object.downcast_ref::<NSTextField>() else {
                return;
            };
            match field.tag() {
                PHRASE_TAG => {
                    host::with(|h| h.learn_phrase_phrase_changed());
                }
                PINYIN_TAG => {
                    host::with(|h| h.learn_phrase_pinyin_changed());
                }
                _ => {}
            }
        }
    }

    unsafe impl NSObjectProtocol for LearnPhraseTarget {}
    unsafe impl NSControlTextEditingDelegate for LearnPhraseTarget {}
    unsafe impl NSTextFieldDelegate for LearnPhraseTarget {}
);

impl LearnPhraseTarget {
    pub fn new(mtm: MainThreadMarker) -> Retained<Self> {
        let this = mtm.alloc::<Self>().set_ivars(());
        unsafe { msg_send![super(this), init] }
    }
}

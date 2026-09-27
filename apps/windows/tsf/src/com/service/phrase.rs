//! 「录入词组」：任务栏中 / 英图标右键菜单里那一项与它的快捷键都落到 `IndicatorCommand::LearnPhrase`，
//! 由 Server 弹窗（Engine 只在 Server 进程里）。快捷键带 Alt，到不了击键 sink，所以登记成 TSF 保留键
//! （见 [`crate::com::key::preserved`]），组合来自 Server 经协议下发的 `[shortcut] learn_phrase`
//! （DLL 不读配置文件），设置里改了跟着换过来。

use windows::Win32::UI::TextServices::ITfKeystrokeMgr;
use windows::core::Interface;

use qingjian_platform::KeyCombo;
use qingjian_platform::protocol::IndicatorCommand;

use super::TextService_Impl;
use crate::com::key::preserved;
use crate::com::log::log;

impl TextService_Impl {
    /// 跟上 Server 下发的「录入词组」组合键：与当前登记的不同就撤掉旧的、登记新的；
    /// 配成 `none` 就只撤不登记。
    pub(super) fn sync_learn_phrase_preserved_key(&self, want: Option<KeyCombo>) {
        if self.learn_phrase_combo.get() == want {
            return;
        }
        let Some(thread_mgr) = self.thread_mgr.borrow().clone() else {
            return;
        };
        let Ok(keystroke) = thread_mgr.cast::<ITfKeystrokeMgr>() else {
            return;
        };
        // 先撤掉旧的那一个（记着的都是登记成功的），再登记新的。
        if let Some(previous) = self.learn_phrase_combo.get() {
            preserved::unregister_learn_phrase(&keystroke, previous);
            self.learn_phrase_combo.set(None);
        }
        let Some(combo) = want else {
            log("录入词组快捷键配成了 none, 不登记保留键");
            return;
        };
        match preserved::register_learn_phrase(&keystroke, self.client_id.get(), combo) {
            Ok(()) => {
                self.learn_phrase_combo.set(Some(combo));
                log(&format!("录入词组快捷键已登记为保留键: {combo}"));
            }
            // 没登记上就不记它，免得停用时拿着一个没登记过的键去撤
            Err(error) => log(&format!("登记录入词组快捷键失败: {error}")),
        }
    }

    /// 停用时撤掉「录入词组」的保留键登记（与 `drop_switch_preserved_key` 同一套时机）。
    pub(super) fn drop_learn_phrase_preserved_key(&self, keystroke: &ITfKeystrokeMgr) {
        if let Some(combo) = self.learn_phrase_combo.take() {
            preserved::unregister_learn_phrase(keystroke, combo);
        }
    }

    /// 保留键命中：请 Server 弹录入词组的窗口。前台权在敲键的这个进程上，`send_indicator` 会替我们
    /// 让给 Server，它的窗口才到得了前面（与点菜单那一项同样处理）。
    pub(super) fn open_learn_phrase(&self) {
        self.send_indicator(IndicatorCommand::LearnPhrase);
    }
}

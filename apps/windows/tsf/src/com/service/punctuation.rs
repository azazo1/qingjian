//! 中 / 英标点切换：任务栏中 / 英图标右键菜单里那一项与快捷键都落到 `IndicatorCommand::TogglePunctuation`，
//! 由 Server 翻转配置并刷新状态条（Engine 在 Server 进程里）。快捷键带不带 Alt 都登记成 TSF 保留键
//! （见 [`crate::com::key::preserved`]），组合来自 Server 经协议下发的 `[shortcut] punctuation_toggle`
//! （DLL 不读配置文件），设置里改了跟着换过来；任何时候都认，正在组句也不打断。

use windows::Win32::UI::TextServices::ITfKeystrokeMgr;
use windows::core::Interface;

use qingjian_platform::KeyCombo;
use qingjian_platform::protocol::IndicatorCommand;

use super::TextService_Impl;
use crate::com::key::preserved;
use crate::com::log::log;

impl TextService_Impl {
    /// 跟上 Server 下发的中 / 英标点切换组合键：与当前登记的不同就撤掉旧的、登记新的；
    /// 配成 `none` 就只撤不登记。
    pub(super) fn sync_punctuation_preserved_key(&self, want: Option<KeyCombo>) {
        if self.punctuation_combo.get() == want {
            return;
        }
        let Some(thread_mgr) = self.thread_mgr.borrow().clone() else {
            return;
        };
        let Ok(keystroke) = thread_mgr.cast::<ITfKeystrokeMgr>() else {
            return;
        };
        // 先撤掉旧的那一个（记着的都是登记成功的），再登记新的。
        if let Some(previous) = self.punctuation_combo.get() {
            preserved::unregister_punctuation(&keystroke, previous);
            self.punctuation_combo.set(None);
        }
        let Some(combo) = want else {
            log("中英文标点切换快捷键配成了 none, 不登记保留键");
            return;
        };
        match preserved::register_punctuation(&keystroke, self.client_id.get(), combo) {
            Ok(()) => {
                self.punctuation_combo.set(Some(combo));
                log(&format!("中英文标点切换快捷键已登记为保留键: {combo}"));
            }
            // 没登记上就不记它，免得停用时拿着一个没登记过的键去撤
            Err(error) => log(&format!("登记中英文标点切换快捷键失败: {error}")),
        }
    }

    /// 停用时撤掉中 / 英标点切换的保留键登记（与 `drop_learn_phrase_preserved_key` 同一套时机）。
    pub(super) fn drop_punctuation_preserved_key(&self, keystroke: &ITfKeystrokeMgr) {
        if let Some(combo) = self.punctuation_combo.take() {
            preserved::unregister_punctuation(keystroke, combo);
        }
    }

    /// 保留键命中：请 Server 翻转当前模式的全角标点（与点菜单、悬浮状态条那一格同样处理）。
    pub(super) fn toggle_punctuation(&self) {
        self.send_indicator(IndicatorCommand::TogglePunctuation);
    }
}

//! 中 / 英切换键命中之后：改模式、把组句中的字母原样上屏、刷新菜单栏指示器。

use super::*;

impl QingjianInputController {
    /// 切换键命中：按动作切到英文 / 中文 / 翻转。
    ///
    /// 组句中切模式先把已敲的字母原样上屏（与 Windows 切回中文时处理缓冲一致）：
    /// 否则拼音缓冲区会被当成英文词去补全，反过来英文缓冲区又会被当拼音继续匹配。
    pub(super) fn apply_switch(&self, action: MacSwitchAction, client: TextClient<'_>) {
        let target = host::with(|h| {
            let english = match action {
                MacSwitchAction::Toggle => !h.mode.english(),
                MacSwitchAction::English => true,
                MacSwitchAction::Chinese => false,
            };
            // 内置英文模式关掉后不再进英文：与 Windows 一样只在这一处拦
            if english && !h.english_mode {
                tracing::debug!("内置英文模式已关闭，忽略切到英文");
                return None;
            }
            h.mode.set_english(english).then_some(english)
        })
        .flatten();
        let Some(english) = target else {
            return;
        };
        if host::with(|h| !h.engine.composition().is_empty()).unwrap_or(false) {
            self.commit_raw(client);
        }
        // 终端、编辑器这类应用（`[apps] english_candidates_off`）里英文模式是纯直通，引擎那份状态跟着走
        let english_candidates = english
            && host::with(|h| h.english_candidates_in(client.bundle_identifier().as_deref()))
                .unwrap_or(false);
        tracing::debug!(english, english_candidates, "切换中 / 英模式");
        let anchor = client.caret_rect();
        host::with(|h| {
            h.engine.set_english_mode(english_candidates);
            h.indicator.update(english);
            h.flash_mode_badge(Some(anchor));
        });
    }
}

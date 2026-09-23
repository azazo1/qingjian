//! 调频键（配置 `[shortcut] adjust_frequency`，缺省 ⌃）配 K / J：把当前高亮的候选升 / 降一格。
//! 按住调频键看频次那件事在 [`super::QingjianInputController::dispatch_modifier_change`] 与
//! [`Host::render`](crate::host::Host::render) 里。

use super::*;

impl QingjianInputController {
    /// 调频键 + K / J 的一次按键：升 / 降当前高亮的候选。
    /// 计数真的动了就重新查一遍（次序会变），界面上频次数字与候选挪位就是反馈；
    /// 没动（降到底了 / 这个候选没有词频可调）才把一句话显示在候选窗右侧。
    ///
    /// 重新查询会把高亮归到第一个候选，所以查完按文本把它落回被调的那个词：连着按 K / J 调的还是同一个候选
    /// （它被调到别的页时页码也跟着过去）。
    pub(super) fn handle_adjust_key(&self, up: bool, client: TextClient<'_>) -> bool {
        let composing = host::with(|h| !h.engine.composition().is_empty()).unwrap_or(false);
        if !composing {
            return false;
        }
        let index = host::with(|h| h.session.highlighted).unwrap_or_default();
        let text = host::with(|h| h.session.candidate(index).map(|c| c.text)).flatten();
        let Some(text) = text else {
            tracing::debug!("这一格没有候选，没什么可调");
            return true;
        };
        let message = host::with(|h| h.adjust_candidate(index, up)).flatten();
        self.refresh(client);
        host::with(|h| h.session.highlight_text(&text));
        if let Some(message) = message {
            tracing::info!(%message);
            host::with(|h| h.status = Some(message));
        }
        self.render(client);
        true
    }
}

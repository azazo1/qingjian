//! 会话：DLL 每个应用线程一条，Server 记宿主应用；焦点在哪个会话，组句就属于谁。

mod info;

use qingjian_platform::protocol::SessionId;

pub(super) use self::info::SessionInfo;
use super::Router;

impl Router {
    pub fn session_count(&self) -> usize {
        self.sessions.len()
    }

    /// 聚焦会话所在应用的 exe 名；DLL 没报时为 `None`。
    pub(super) fn focused_app(&self) -> Option<&str> {
        self.focused
            .and_then(|session| self.sessions.get(&session))
            .and_then(|info| info.app.as_deref())
    }

    pub(super) fn ensure_focus(&mut self, session: SessionId) {
        if self.focused != Some(session) {
            self.reset_composition();
            self.focused = Some(session);
            let app = self.focused_app().map(str::to_owned);
            self.engine.set_application(app);
            let private = self.focused_private();
            self.engine.set_private(private);
        }
    }

    /// 当前聚焦的会话在私密输入框里（Engine 不学不记不发云端）。
    pub fn is_private(&self) -> bool {
        self.engine.is_private()
    }

    fn focused_private(&self) -> bool {
        self.focused
            .and_then(|session| self.sessions.get(&session))
            .is_some_and(|info| info.private)
    }

    /// DLL 报来该会话的输入框私密与否变了：记下；是当前会话就立刻让 Engine 进 / 出私密。
    pub(super) fn set_privacy(&mut self, session: SessionId, private: bool) {
        if let Some(info) = self.sessions.get_mut(&session) {
            info.private = private;
        }
        if self.focused == Some(session) {
            self.engine.set_private(private);
        }
    }

    /// 焦点离开：把缓冲区原样交出并清组句。组句不属于 `session` 时只清不交，别把 A 应用的拼音落进 B。
    pub(super) fn commit_raw_for(&mut self, session: SessionId) -> Option<String> {
        // 缓冲区空了但还有没交给应用的已选词时也要交出去（`take_raw` 把它们接在原样上屏的字母前面）
        let text = (self.focused == Some(session)
            && (!self.engine.composition().is_empty() || self.engine.has_pending()))
        .then(|| self.engine.take_raw())
        .filter(|text| !text.is_empty());
        self.reset_composition();
        text
    }

    /// 清掉组句、展示状态、在飞的云联想与两个评审态，收起候选窗口。
    /// 还没交给应用的已选词在这里丢掉：失焦那一路（[`Self::commit_raw_for`]）已经先 `take_raw` 交出去了。
    pub(super) fn reset_composition(&mut self) {
        self.engine.break_chain();
        self.engine.clear();
        self.cancel_prediction();
        self.stop_rescoring();
        self.composed = None;
        self.translation = None;
        self.phrase = None;
        self.phrase_notice = None;
        self.pending_selection = None;
        self.sentence = None;
        self.notice = None;
        self.highlight = 0;
        self.navigated = false;
        self.hide_candidate_window();
    }
}

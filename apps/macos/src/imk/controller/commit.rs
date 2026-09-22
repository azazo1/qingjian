//! 上屏：高亮候选、指定候选、拼音原样。

use super::*;

impl QingjianInputController {
    pub(super) fn commit_highlighted(&self, client: TextClient<'_>) -> bool {
        let index = host::with(|h| h.session.highlighted).unwrap_or(0);
        self.commit_index(index, client)
    }

    /// 上屏第 `index` 个候选；没有候选时上屏拼音本身。上屏后剩余拼音继续组句。
    pub(super) fn commit_index(&self, index: usize, client: TextClient<'_>) -> bool {
        let candidate = host::with(|h| h.session.candidate(index)).flatten();
        let Some(candidate) = candidate else {
            if host::with(|h| index < h.session.layout.len()).unwrap_or(false) {
                return true;
            }
            return self.commit_raw(client);
        };
        let Some(text) = host::with(|h| h.engine.commit(&candidate)) else {
            return false;
        };
        tracing::debug!(%text, "commit");
        // 这段拼音还没选完时上屏文本是空的：词留在 Engine 里等组句结束（见 [`Self::flush_pending`]），
        // 只有组句结束的那一次才真的交给应用
        if !text.is_empty() {
            client.insert_text(&text);
        }
        self.refresh(client);
        true
    }

    /// 把 Engine 里还没交给应用的已选词上屏。这一键要交给应用（文本流越过组句）、
    /// 组句被取消或失焦时调；没有就什么都不做。
    pub(super) fn flush_pending(&self, client: TextClient<'_>) {
        let pending = host::with(|h| h.engine.take_pending()).unwrap_or_default();
        if pending.is_empty() {
            return;
        }
        tracing::debug!(%pending, "还没上屏的已选词交给应用");
        client.insert_text(&pending);
        self.refresh(client);
    }

    /// 把拼音原样上屏并清空。缓冲区为空时返回 false。
    pub(super) fn commit_raw(&self, client: TextClient<'_>) -> bool {
        let Some(raw) = host::with(|h| h.engine.take_raw()) else {
            return false;
        };
        if raw.is_empty() {
            return false;
        }
        tracing::debug!(%raw, "commit raw");
        client.insert_text(&raw);
        self.refresh(client);
        true
    }
}

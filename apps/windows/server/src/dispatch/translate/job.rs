//! 「翻译选中文字」的进行态。

/// 一次「翻译选中文字」的进行态。
pub(crate) struct Translation {
    /// 本机释义表给出的译文; `None` 表示没有.
    pub(crate) result: Option<String>,
}

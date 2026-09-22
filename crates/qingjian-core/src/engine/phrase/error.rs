//! 记词组失败的原因。

use thiserror::Error;

use crate::parser::ParseError;

/// 一次「拼音 → 文本」没记成的原因。
#[derive(Debug, Error)]
pub enum PhraseError {
    /// 选中的文本是空的。
    #[error("empty phrase text")]
    EmptyText,

    /// 拼音是空的。
    #[error("empty pinyin")]
    EmptyPinyin,

    /// 拼音切不出音节：有字母与 `'` 之外的字符，或整串切不动。
    #[error("bad pinyin: {0}")]
    BadPinyin(ParseError),

    /// 拼音里有没打完的音节（简拼或半截）。
    #[error("incomplete pinyin")]
    Incomplete,

    /// 私密输入中不记任何学习数据。
    #[error("private input")]
    Private,

    /// 用户关掉了学习（`[general] learning = false`）。
    #[error("learning is off")]
    LearningOff,
}

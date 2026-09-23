/// 手动录入词组失败的原因.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum LearnPhraseError {
    /// 词组去掉首尾空白之后是空的.
    #[error("phrase is empty")]
    EmptyText,

    /// 词组中间有空白, 这是词不是句子.
    #[error("phrase must not contain whitespace")]
    HasWhitespace,

    /// 超过 [`super::MAX_PHRASE_CHARS`] 个字.
    #[error("phrase is longer than {0} characters")]
    TooLong(usize),

    /// 拼音去掉首尾空白之后是空的.
    #[error("pinyin is empty")]
    EmptyPinyin,

    /// 填的拼音切不成完整音节.
    #[error("pinyin is not a sequence of complete syllables")]
    InvalidPinyin,

    /// 音节个数和汉字个数对不上.
    #[error("expected {chars} syllables, got {syllables}")]
    SyllableCount {
        /// 词组的字数.
        chars: usize,

        /// 切出来的音节数.
        syllables: usize,
    },
}

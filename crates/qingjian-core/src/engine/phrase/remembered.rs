//! 一次记词组的结果。

/// 记下的一条用户词。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RememberedPhrase {
    /// 记下的文本，原样保留（含空格与标点）。
    pub text: String,

    /// 解析出的音节，小写、无分隔。
    pub syllables: Vec<String>,

    /// 这个文本原来记着的拼音（空格分隔）；原来不是用户词时为 `None`。
    pub replaced: Option<String>,
}

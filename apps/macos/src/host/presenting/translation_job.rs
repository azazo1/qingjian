use objc2_foundation::NSRange;

/// 一次「翻译选中文字」：从按下快捷键到用户接受或放弃。
#[derive(Debug, Clone)]
pub struct TranslationJob {
    /// 选区在应用里的范围，接受时用译文替换它。
    pub range: NSRange,

    /// 本机释义表给出的译文; `None` 表示没有.
    pub result: Option<String>,
}

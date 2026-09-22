/// 一次「记词组」的进行态。
pub(crate) struct Phrase {
    /// 选中的文本，提交时与拼音一起记成用户词。
    pub(crate) text: String,

    /// 正在编辑的拼音，音节之间用 `'` 分隔；预填的是词库反查出来的读音。
    pub(crate) pinyin: String,
}

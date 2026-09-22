/// 一次「记词组」：从按下快捷键到回车记下或 Esc 放弃。
///
/// 选中文本进评审态时已经读出来了，评审期间只动拼音；应用里的选区一个字符都不动。
#[derive(Debug, Clone)]
pub struct PhraseJob {
    /// 选中文本，提交时与拼音一起写进用户词。
    pub text: String,

    /// 正在编辑的拼音，音节之间用 `'` 分隔；预填的是词库反查出来的读音。
    pub pinyin: String,
}

impl PhraseJob {
    /// 候选窗口拼音行显示的内容：拼音为空时给一段空格，窗口才不至于收起。
    pub fn display_pinyin(&self) -> &str {
        if self.pinyin.is_empty() { " " } else { &self.pinyin }
    }
}

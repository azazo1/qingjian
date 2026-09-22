/// 一次读选区请求的用途：回包按它决定进哪个评审态。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum SelectionPurpose {
    /// 「翻译选中文字」：译文回来后回车替换选区。
    Translate,

    /// 「记词组」：读出来的文本配上拼音，回车记成用户词。
    LearnPhrase,
}

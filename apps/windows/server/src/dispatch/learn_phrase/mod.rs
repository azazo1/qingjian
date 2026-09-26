//! 「录入词组」：Engine 只在工人线程（Server 独占），窗口在 UI 线程，两边靠 [`Work`](crate::ipc::Work)
//! 的一问一答往返。反查与写入都走 Core：`suggest_pinyin` 给建议、`parse_phrase_pinyin` 校验手填、
//! `learn_phrase` 记词并记一次选择（与 macOS 壳的「录入词组…」同一套，见 `6f7e56d`）。

use qingjian_core::{Engine, LearnPhraseError};

use super::Router;

/// 窗口请工人线程办的一件活。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LearnPhraseWork {
    /// 词组框每变一下：按词库反查拼音（拼音框被用户手改过时窗口不该发这条）。
    Suggest(String),

    /// 点「录入」：校验拼音并记一次选择。
    Confirm { text: String, pinyin: String },
}

/// 工人线程答回去的结果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LearnPhraseReply {
    /// 建议的拼音（空格分隔）；词库里查不到是 `None`，留给用户手填。
    Pinyin(Option<String>),

    /// 已写入：词库里的词只记一次选择，新词先记进用户词。
    Learned,

    /// 出错文案，直接显示在窗口里。
    Error(String),
}

impl Router {
    /// 菜单点「录入词组…」：先把反查表建好，免得第一个字现扫整本词库。
    pub(super) fn prepare_learn_phrase(&self) {
        self.engine.prepare_phrase_readings();
    }

    /// 窗口的一件活。
    pub fn learn_phrase(&mut self, work: LearnPhraseWork) -> LearnPhraseReply {
        match work {
            LearnPhraseWork::Suggest(text) => LearnPhraseReply::Pinyin(
                self.engine
                    .suggest_pinyin(&text)
                    .map(|syllables| syllables.join(" ")),
            ),
            LearnPhraseWork::Confirm { text, pinyin } => {
                let text = text.trim().to_owned();
                let syllables = match Engine::parse_phrase_pinyin(&text, &pinyin) {
                    Ok(syllables) => syllables,
                    Err(error) => return LearnPhraseReply::Error(phrase_error_text(error)),
                };
                match self.engine.learn_phrase(&text, &syllables) {
                    Ok(()) => {
                        // 录入是管理操作，用户马上会去打一遍，立刻落盘。
                        self.engine.flush_learning();
                        LearnPhraseReply::Learned
                    }
                    Err(error) => LearnPhraseReply::Error(phrase_error_text(error)),
                }
            }
        }
    }
}

/// 与 macOS 壳同一套说法。
pub fn phrase_error_text(error: LearnPhraseError) -> String {
    match error {
        LearnPhraseError::EmptyText => "请输入词组".to_owned(),
        LearnPhraseError::HasWhitespace => "词组里不要有空格".to_owned(),
        LearnPhraseError::TooLong(max) => format!("词组最多 {max} 个字"),
        LearnPhraseError::EmptyPinyin => "请填写拼音".to_owned(),
        LearnPhraseError::InvalidPinyin => "拼音无法切成完整音节".to_owned(),
        LearnPhraseError::SyllableCount { chars, syllables } => {
            format!("拼音是 {syllables} 个音节, 词组是 {chars} 个字")
        }
    }
}

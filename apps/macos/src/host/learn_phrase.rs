//! 菜单栏「录入词组」: 调 Engine 反查拼音 / 记一次选择, 必要时重画正在组句的候选.

use qingjian_core::LearnPhraseError;

use super::Host;
use crate::candidates::Preedit;

impl Host {
    /// 词组框每敲一个字: 拼音框未脏时按词库反查填全拼.
    pub fn learn_phrase_phrase_changed(&mut self) {
        self.learn_phrase.clear_dirty_if_phrase_empty();
        if self.learn_phrase.pinyin_dirty() {
            return;
        }
        let text = self.learn_phrase.phrase_text();
        let suggested = self.engine.suggest_pinyin(&text);
        self.learn_phrase.set_pinyin(suggested.as_deref());
    }

    /// 用户改了拼音框, 之后不要再用反查覆盖.
    pub fn learn_phrase_pinyin_changed(&mut self) {
        self.learn_phrase.mark_pinyin_dirty();
    }

    /// 点「录入」: 校验拼音, 记一次选择, 立刻落盘.
    pub fn confirm_learn_phrase(&mut self) {
        let text = self.learn_phrase.phrase_text();
        let pinyin = self.learn_phrase.pinyin_text();
        let syllables = match qingjian_core::Engine::parse_phrase_pinyin(&text, &pinyin) {
            Ok(syllables) => syllables,
            Err(error) => {
                self.learn_phrase.set_error(&phrase_error_text(error));
                return;
            }
        };
        if let Err(error) = self.engine.learn_phrase(&text, &syllables) {
            self.learn_phrase.set_error(&phrase_error_text(error));
            return;
        }
        self.engine.flush_learning();
        let learned = text.trim().to_owned();
        self.learn_phrase.clear_after_success(&learned);
        self.refresh_candidates_after_learn();
    }

    /// 正在组句时录入成功: 格子缓存已作废, 重查一帧让新词立刻出现.
    fn refresh_candidates_after_learn(&mut self) {
        if self.engine.composition().is_empty() || self.translation.is_some() {
            return;
        }
        let Ok(mut query) = self.engine.query() else {
            return;
        };
        self.engine.annotate(&mut query.candidates);
        let preedit = Preedit::from_marked(&query.marked_segments(), query.marked_cursor());
        let cloud = self.session.layout.cloud().to_vec();
        self.reset_session(preedit, query.candidates.items);
        if !cloud.is_empty() {
            self.session.layout.set_cloud(cloud);
        }
        self.render();
    }
}

fn phrase_error_text(error: LearnPhraseError) -> String {
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

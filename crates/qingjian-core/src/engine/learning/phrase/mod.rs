//! 手动录入词组: 汉字反查拼音, 效果等同于打这段全拼并选一次该词.

use std::collections::HashMap;

use qingjian_dictionary::canonical_syllable;

use super::super::Engine;
use crate::candidate::{Candidate, CandidateKind};
use crate::parser;
use crate::sentence;

mod error;

pub use error::LearnPhraseError;

/// 手录词组的字数上限. 自动造词另有 4 字上限, 那条管的是上屏接续, 不限制这里.
pub const MAX_PHRASE_CHARS: usize = 32;

impl Engine {
    /// 给一段汉字猜全拼音节: 整词命中 (词频最高的读音) 优先, 否则按语言模型切开再查, 再否则逐字取单字最高频读音.
    /// 任一汉字在词库里没有读音就返回 `None`, 留给用户手填.
    pub fn suggest_pinyin(&self, text: &str) -> Option<Vec<String>> {
        let text = text.trim();
        if text.is_empty() || text.chars().any(char::is_whitespace) {
            return None;
        }
        self.ensure_phrase_readings();
        if let Some(syllables) = self.reading_of(text) {
            return Some(syllables);
        }
        let words: Vec<String> = match sentence::segment_text(text, &*self.language_model) {
            Some(clauses) => clauses.into_iter().flatten().collect(),
            None => text.chars().map(String::from).collect(),
        };
        let mut out = Vec::new();
        for word in words {
            if let Some(syllables) = self.reading_of(&word) {
                out.extend(syllables);
                continue;
            }
            for c in word.chars() {
                out.extend(self.reading_of(&c.to_string())?);
            }
        }
        (out.len() == text.chars().count()).then_some(out)
    }

    /// 打开录入窗口前把反查表建好, 第一个字就不用在按键回调里扫完整本词库.
    pub fn prepare_phrase_readings(&self) {
        self.ensure_phrase_readings();
    }

    /// 解析用户填的拼音 (空格 / `'` / 无分隔全拼), 校验每个都是完整音节且个数等于字数.
    pub fn parse_phrase_pinyin(text: &str, pinyin: &str) -> Result<Vec<String>, LearnPhraseError> {
        let chars = phrase_char_count(text)?;
        parse_syllables(pinyin, chars)
    }

    /// 等同于打这段全拼并选一次 `text`: 词库没有同音节的词就记进用户词, 再记一次词频和按输入串的选择.
    /// 没有真实组句上下文, 不记 n-gram, 不写下屏日志. 这是菜单上的管理操作, 学习开关关掉也照常写入.
    pub fn learn_phrase(
        &mut self,
        text: &str,
        syllables: &[String],
    ) -> Result<(), LearnPhraseError> {
        let chars = phrase_char_count(text)?;
        if syllables.len() != chars {
            return Err(LearnPhraseError::SyllableCount {
                chars,
                syllables: syllables.len(),
            });
        }
        if syllables
            .iter()
            .any(|s| !parser::is_syllable(canonical_syllable(s)))
        {
            return Err(LearnPhraseError::InvalidPinyin);
        }
        let syllables: Vec<String> = syllables
            .iter()
            .map(|s| canonical_syllable(s).to_owned())
            .collect();
        let text = text.trim();
        let candidate = Candidate {
            text: text.to_owned(),
            kind: CandidateKind::Chinese,
            syllables: syllables.clone(),
            reading: None,
            translation: None,
            aux_code: None,
        };
        let known = self.knows_word(&candidate);
        self.forget_span_cache();
        let learner = self.learner.inner_mut();
        if !known {
            learner.learn_word(text, &syllables);
        }
        learner.record(&candidate);
        let input: String = syllables.concat();
        learner.record_choice(&input, text);
        tracing::info!(text, pinyin = %syllables.join(" "), "录入词组");
        Ok(())
    }

    /// 主词库 + 附加词库的反查表, 没有就建. 用户词另查, 见 [`Self::reading_of`].
    fn ensure_phrase_readings(&self) {
        if self.phrase_readings.borrow().is_some() {
            return;
        }
        let mut dictionaries = Vec::with_capacity(self.extra_dictionaries.len() + 1);
        dictionaries.push(&self.dictionary);
        dictionaries.extend(self.extra_dictionaries.iter());
        *self.phrase_readings.borrow_mut() = Some(build_readings(&dictionaries));
    }

    /// 用户词反查. 词库对象换过 (录入或删除重建了 Dictionary) 才重扫.
    fn ensure_user_readings(&self) {
        let addr = self
            .learner
            .user_words()
            .map(|dictionary| dictionary as *const _ as usize)
            .unwrap_or(0);
        if self
            .user_phrase_readings
            .borrow()
            .as_ref()
            .is_some_and(|(cached, _)| *cached == addr)
        {
            return;
        }
        let readings = match self.learner.user_words() {
            Some(dictionary) => build_readings(std::slice::from_ref(dictionary)),
            None => HashMap::new(),
        };
        *self.user_phrase_readings.borrow_mut() = Some((addr, readings));
    }

    /// 同一文本取词频最高的读音. 词频打平留主词库 / 附加词库的, 用户词只有更高才盖过.
    fn reading_of(&self, text: &str) -> Option<Vec<String>> {
        self.ensure_user_readings();
        let static_map = self.phrase_readings.borrow();
        let user_slot = self.user_phrase_readings.borrow();
        let static_hit = static_map.as_ref().and_then(|readings| readings.get(text));
        let user_hit = user_slot
            .as_ref()
            .and_then(|(_, readings)| readings.get(text));
        let pinyin = match (static_hit, user_hit) {
            (Some((pinyin, freq)), Some((user_pinyin, user_freq))) if *user_freq > *freq => {
                user_pinyin.as_str()
            }
            (Some((pinyin, _)), _) => pinyin.as_str(),
            (None, Some((pinyin, _))) => pinyin.as_str(),
            (None, None) => return None,
        };
        Some(split_reading(pinyin))
    }
}

/// 同一文本留词频最高的那条拼音. 打平保留先扫到的 (主词库优先于附加词库).
fn build_readings(
    dictionaries: &[&qingjian_dictionary::Dictionary],
) -> HashMap<String, (String, u32)> {
    let mut best: HashMap<String, (String, u32)> = HashMap::new();
    for dictionary in dictionaries {
        for entry in dictionary.entries() {
            match best.get_mut(entry.text) {
                Some((_, freq)) if *freq >= entry.frequency => {}
                Some(slot) => *slot = (entry.pinyin.to_owned(), entry.frequency),
                None => {
                    best.insert(
                        entry.text.to_owned(),
                        (entry.pinyin.to_owned(), entry.frequency),
                    );
                }
            }
        }
    }
    best
}

fn split_reading(pinyin: &str) -> Vec<String> {
    pinyin
        .split(' ')
        .filter(|syllable| !syllable.is_empty())
        .map(|syllable| canonical_syllable(syllable).to_owned())
        .collect()
}

fn phrase_char_count(text: &str) -> Result<usize, LearnPhraseError> {
    let text = text.trim();
    if text.is_empty() {
        return Err(LearnPhraseError::EmptyText);
    }
    if text.chars().any(char::is_whitespace) {
        return Err(LearnPhraseError::HasWhitespace);
    }
    let chars = text.chars().count();
    if chars > MAX_PHRASE_CHARS {
        return Err(LearnPhraseError::TooLong(MAX_PHRASE_CHARS));
    }
    Ok(chars)
}

/// 空白和 `'` 切开; 每一段都是完整音节就直接用, 否则拼起来走 [`parser::segment`], 取音节数对得上且全部完整的切分.
fn parse_syllables(pinyin: &str, chars: usize) -> Result<Vec<String>, LearnPhraseError> {
    let pinyin = pinyin.trim().to_ascii_lowercase();
    if pinyin.is_empty() {
        return Err(LearnPhraseError::EmptyPinyin);
    }
    let tokens: Vec<&str> = pinyin
        .split(|c: char| c.is_ascii_whitespace() || c == '\'')
        .filter(|token| !token.is_empty())
        .collect();
    if tokens.is_empty() {
        return Err(LearnPhraseError::EmptyPinyin);
    }
    if tokens
        .iter()
        .all(|token| parser::is_syllable(canonical_syllable(token)))
    {
        if tokens.len() != chars {
            return Err(LearnPhraseError::SyllableCount {
                chars,
                syllables: tokens.len(),
            });
        }
        return Ok(tokens
            .into_iter()
            .map(|token| canonical_syllable(token).to_owned())
            .collect());
    }
    let joined: String = tokens.concat();
    let Ok(segmentations) = parser::segment(&joined) else {
        return Err(LearnPhraseError::InvalidPinyin);
    };
    let Some(matched) = segmentations.into_iter().find(|segmentation| {
        segmentation.syllables.len() == chars && segmentation.syllables.iter().all(|s| s.complete)
    }) else {
        return Err(LearnPhraseError::InvalidPinyin);
    };
    Ok(matched
        .syllables
        .into_iter()
        .map(|s| canonical_syllable(&s.text).to_owned())
        .collect())
}

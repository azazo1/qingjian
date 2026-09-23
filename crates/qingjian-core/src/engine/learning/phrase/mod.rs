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
        let readings = self.readings();
        if let Some(syllables) = readings.get(text) {
            return Some(syllables.clone());
        }
        let words: Vec<String> = match sentence::segment_text(text, &*self.language_model) {
            Some(clauses) => clauses.into_iter().flatten().collect(),
            None => text.chars().map(String::from).collect(),
        };
        let mut out = Vec::new();
        for word in words {
            if let Some(syllables) = readings.get(&word) {
                out.extend(syllables.iter().cloned());
                continue;
            }
            for c in word.chars() {
                out.extend(readings.get(&c.to_string())?.iter().cloned());
            }
        }
        (out.len() == text.chars().count()).then_some(out)
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

    /// 主词库 + 附加词库 + 用户词: 同一文本取词频最高的读音.
    fn readings(&self) -> HashMap<String, Vec<String>> {
        let mut best: HashMap<String, (Vec<String>, u32)> = HashMap::new();
        for dictionary in self.all_dictionaries() {
            for entry in dictionary.entries() {
                let syllables: Vec<String> = entry
                    .syllables()
                    .map(|s| canonical_syllable(s).to_owned())
                    .collect();
                match best.get_mut(entry.text) {
                    Some((_, freq)) if *freq >= entry.frequency => {}
                    Some(slot) => *slot = (syllables, entry.frequency),
                    None => {
                        best.insert(entry.text.to_owned(), (syllables, entry.frequency));
                    }
                }
            }
        }
        best.into_iter()
            .map(|(text, (syllables, _))| (text, syllables))
            .collect()
    }
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

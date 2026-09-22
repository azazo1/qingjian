//! 记词组：把应用里选中的文本连同用户给的拼音记成一条用户词。
//!
//! 与「敲这段拼音、再从候选里选中这段文本上屏」是同一笔账：写用户词表、在该输入串下记一次选择、
//! 再按一次用户选择记账。平台的入口是壳：读选区、显示拼音行、回车确认。
//! 平台层不做任何文本变换，拼音怎么解释、词怎么记都在这里定。

use std::time::Instant;

use super::Engine;
use crate::Transcriber;
use crate::candidate::{Candidate, CandidateKind};
use crate::parser::{self, Segmentation};

mod error;
mod remembered;

pub use error::PhraseError;
pub use remembered::RememberedPhrase;

impl Engine {
    /// 一段文本的默认拼音，音节之间用 `'` 分隔；有字查不到读音返回 `None`。
    pub fn pinyin_of(&mut self, text: &str) -> Option<String> {
        self.ensure_transcriber();
        let syllables = self
            .readings
            .as_ref()?
            .syllables(text, &*self.language_model)?;
        (!syllables.is_empty()).then(|| syllables.join("'"))
    }

    /// 把「拼音 → 文本」记成用户词：文本进用户词表（同一个文本原来的拼音被换掉），
    /// 并在该拼音下记一次选择，于是下次敲这段拼音时它是候选，而且排在同一输入串下最前。
    ///
    /// 拼音里的分隔符可以是 `'` 或空白；切分里有没打完的音节、或有字母与 `'` 之外的字符都不收。
    /// 私密输入中与用户关掉学习时返回错误、什么都不记。
    pub fn remember_phrase(
        &mut self,
        pinyin: &str,
        text: &str,
    ) -> Result<RememberedPhrase, PhraseError> {
        if text.is_empty() {
            return Err(PhraseError::EmptyText);
        }
        if pinyin.trim().is_empty() {
            return Err(PhraseError::EmptyPinyin);
        }
        if self.private {
            return Err(PhraseError::Private);
        }
        if self.learner.is_muted() {
            return Err(PhraseError::LearningOff);
        }
        let syllables = parse_syllables(pinyin)?;
        let letters: String = syllables.concat();
        let candidate = Candidate {
            text: text.to_owned(),
            kind: CandidateKind::Chinese,
            syllables: syllables.clone(),
            reading: None,
            translation: None,
            aux_code: None,
        };
        let learner = self.learner_mut();
        let replaced = learner.word_pinyin(text);
        learner.learn_word(text, &syllables);
        learner.record_choice(&letters, text);
        learner.record(&candidate);
        tracing::info!(text, pinyin = %syllables.join(" "), "记下用户词组");
        Ok(RememberedPhrase {
            text: text.to_owned(),
            syllables,
            replaced,
        })
    }

    /// 反查表惰性建一次就留着：建表要遍历全部词库，用户按键时不该每按一次都付这份代价。
    fn ensure_transcriber(&mut self) {
        if self.readings.is_some() {
            return;
        }
        let started = Instant::now();
        let readings = {
            let dictionaries = self.all_dictionaries();
            Transcriber::new(dictionaries)
        };
        tracing::info!(
            words = readings.len(),
            ms = started.elapsed().as_millis(),
            "读音反查表已建"
        );
        self.readings = Some(readings);
    }
}

/// 把用户给的拼音串切成音节：空白当分隔符（与 `'` 等价），字母的合法性交给 [`parser::segment`]。
fn parse_syllables(pinyin: &str) -> Result<Vec<String>, PhraseError> {
    let normalized: String = pinyin
        .chars()
        .map(|c| if c.is_whitespace() { '\'' } else { c })
        .collect();
    let segmentation = first_segmentation(&normalized)?;
    if segmentation
        .syllables
        .iter()
        .any(|syllable| !syllable.complete)
    {
        return Err(PhraseError::Incomplete);
    }
    Ok(segmentation
        .syllables
        .into_iter()
        .map(|syllable| syllable.text)
        .collect())
}

/// 取排序最靠前的那种切分（音节少、不完整音节少、前面的音节长的优先）。
fn first_segmentation(input: &str) -> Result<Segmentation, PhraseError> {
    let mut segmentations = parser::segment(input).map_err(PhraseError::BadPinyin)?;
    if segmentations.is_empty() {
        return Err(PhraseError::BadPinyin(parser::ParseError::NoSegmentation));
    }
    Ok(segmentations.remove(0))
}

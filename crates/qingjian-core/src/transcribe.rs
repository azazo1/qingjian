//! 汉字到读音的反查：按语言模型把句子切成词，每个词查词库里的读音，词库没有的词退到逐字查。
//!
//! 给两处用：CLI 的整句评测把中文原文转成用户会敲的拼音；「记词组」给选中的文本预填拼音。
//! 多音字取词库里那个词（或那个字最常用）的读音，任何一个字查不到就放弃整句。

use std::collections::HashMap;

use qingjian_dictionary::Dictionary;

use crate::sentence::{LanguageModel, segment_text};

/// 汉字到读音的反查表。
pub struct Transcriber {
    /// 词文本 → 音节；同一个词多个读音时取词频最高的。
    readings: HashMap<String, (Vec<String>, u32)>,
}

impl Transcriber {
    /// 从词库（主词库 + 附加词库）建反查表。建一次够几十万条词，壳只在需要时建一次并缓存。
    pub fn new<'a>(dictionaries: impl IntoIterator<Item = &'a Dictionary>) -> Self {
        let mut readings: HashMap<String, (Vec<String>, u32)> = HashMap::new();
        for dictionary in dictionaries {
            for entry in dictionary.entries() {
                let syllables: Vec<String> =
                    entry.pinyin.split(' ').map(str::to_owned).collect();
                match readings.get_mut(entry.text) {
                    Some(best) if best.1 >= entry.frequency => {}
                    Some(best) => *best = (syllables, entry.frequency),
                    None => {
                        readings.insert(entry.text.to_owned(), (syllables, entry.frequency));
                    }
                }
            }
        }
        Self { readings }
    }

    pub fn len(&self) -> usize {
        self.readings.len()
    }

    pub fn is_empty(&self) -> bool {
        self.readings.is_empty()
    }

    /// 一句文本的音节；有字查不到读音就 `None`。
    pub fn syllables(&self, text: &str, model: &dyn LanguageModel) -> Option<Vec<String>> {
        let words: Vec<String> = match segment_text(text, model) {
            Some(clauses) => clauses.into_iter().flatten().collect(),
            None => text.chars().map(String::from).collect(),
        };
        let mut syllables = Vec::new();
        for word in &words {
            match self.readings.get(word) {
                Some((reading, _)) => syllables.extend(reading.iter().cloned()),
                None => {
                    let mut buffer = [0u8; 4];
                    for c in word.chars() {
                        let (reading, _) = self.readings.get(c.encode_utf8(&mut buffer))?;
                        syllables.extend(reading.iter().cloned());
                    }
                }
            }
        }
        Some(syllables)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sentence::NoLanguageModel;

    const SAMPLE: &str =
        "长\tzhang\t900\n长\tchang\t800\n大\tda\t1000\n长度\tchang du\t500\n度\tdu\t700\n";

    #[test]
    fn prefers_the_word_reading_over_the_most_common_char_reading() {
        let dictionary = Dictionary::parse(SAMPLE).unwrap();
        let transcriber = Transcriber::new([&dictionary]);
        // 没有语言模型时逐字：长 取词频高的 zhang
        assert_eq!(
            transcriber.syllables("长大", &NoLanguageModel).as_deref(),
            Some(["zhang".to_owned(), "da".to_owned()].as_slice())
        );
        // 词库有「长度」这个词：整词查到 chang
        struct Model;
        impl LanguageModel for Model {
            fn log_prob(&self, _: Option<&str>, word: &str) -> Option<f64> {
                (word == "长度").then_some(-3.0)
            }
        }
        assert_eq!(
            transcriber.syllables("长度", &Model).as_deref(),
            Some(["chang".to_owned(), "du".to_owned()].as_slice())
        );
        assert_eq!(transcriber.syllables("长短", &NoLanguageModel), None);
    }
}

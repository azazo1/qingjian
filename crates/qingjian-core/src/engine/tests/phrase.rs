//! 手动录入词组: 反查拼音, 解析拼音, 效果等同于选一次.

use super::*;
use crate::engine::LearnPhraseError;
use crate::engine::learning::phrase::MAX_PHRASE_CHARS;
use crate::sentence::LanguageModel;

const READINGS: &str = "\
长\tzhang\t900
长\tchang\t800
度\tdu\t700
大\tda\t1000
长度\tchang du\t500
开\tkai\t20000
发\tfa\t100
开发\tkai fa\t9000
开放\tkai fang\t20000
";

fn reading_engine() -> Engine {
    Engine::new(Dictionary::parse(READINGS).unwrap())
}

struct TinyModel;

impl LanguageModel for TinyModel {
    fn log_prob(&self, _previous: Option<&str>, word: &str) -> Option<f64> {
        match word {
            "开发" | "开放" | "长度" => Some(-5.0),
            _ => None,
        }
    }
}

#[test]
fn suggest_pinyin_prefers_the_word_reading() {
    let engine = reading_engine();
    assert_eq!(
        engine.suggest_pinyin("长度").as_deref(),
        Some(["chang".to_owned(), "du".to_owned()].as_slice())
    );
}

#[test]
fn suggest_pinyin_falls_back_to_the_most_common_char_reading() {
    let engine = reading_engine();
    assert_eq!(
        engine.suggest_pinyin("长大").as_deref(),
        Some(["zhang".to_owned(), "da".to_owned()].as_slice())
    );
}

#[test]
fn suggest_pinyin_segments_known_words_without_a_whole_match() {
    let engine = reading_engine().with_language_model(Box::new(TinyModel));
    assert_eq!(
        engine.suggest_pinyin("开发开放").as_deref(),
        Some(
            [
                "kai".to_owned(),
                "fa".to_owned(),
                "kai".to_owned(),
                "fang".to_owned()
            ]
            .as_slice()
        )
    );
}

#[test]
fn suggest_pinyin_returns_none_when_a_char_has_no_reading() {
    let engine = reading_engine();
    assert_eq!(engine.suggest_pinyin("龘"), None);
    assert_eq!(engine.suggest_pinyin(""), None);
    assert_eq!(engine.suggest_pinyin("开 发"), None);
}

#[test]
fn parse_phrase_pinyin_accepts_spaces_apostrophes_and_concatenated() {
    for pinyin in ["kai fa", "kai'fa", "kaifa", "KAI FA", "  kai  fa  "] {
        assert_eq!(
            Engine::parse_phrase_pinyin("开发", pinyin).unwrap(),
            ["kai", "fa"]
        );
    }
}

#[test]
fn parse_phrase_pinyin_canonicalizes_lue() {
    assert_eq!(Engine::parse_phrase_pinyin("略", "lue").unwrap(), ["lve"]);
}

#[test]
fn parse_phrase_pinyin_rejects_count_mismatch_and_invalid() {
    assert_eq!(
        Engine::parse_phrase_pinyin("开发", "kai"),
        Err(LearnPhraseError::SyllableCount {
            chars: 2,
            syllables: 1
        })
    );
    assert_eq!(
        Engine::parse_phrase_pinyin("西安", "xian"),
        Err(LearnPhraseError::SyllableCount {
            chars: 2,
            syllables: 1
        })
    );
    assert_eq!(
        Engine::parse_phrase_pinyin("开发", "xyz"),
        Err(LearnPhraseError::InvalidPinyin)
    );
    assert_eq!(
        Engine::parse_phrase_pinyin("开发", ""),
        Err(LearnPhraseError::EmptyPinyin)
    );
    assert_eq!(
        Engine::parse_phrase_pinyin("", "kai"),
        Err(LearnPhraseError::EmptyText)
    );
    assert_eq!(
        Engine::parse_phrase_pinyin("开 发", "kai fa"),
        Err(LearnPhraseError::HasWhitespace)
    );
    let long: String = "字".repeat(MAX_PHRASE_CHARS + 1);
    assert_eq!(
        Engine::parse_phrase_pinyin(&long, "a"),
        Err(LearnPhraseError::TooLong(MAX_PHRASE_CHARS))
    );
}

#[test]
fn learn_phrase_ranks_a_known_word_first_for_that_pinyin() {
    let mut engine = engine().with_learner(Box::new(CountingLearner(HashMap::new())));
    engine
        .learn_phrase("开发", &["kai".into(), "fa".into()])
        .unwrap();
    engine.set_input("kaifa");
    assert_eq!(engine.query().unwrap().candidates.items[0].text, "开发");
    assert_eq!(engine.learner().choice_weight("kaifa", "开发"), 1);
    assert_eq!(engine.learner().weight("开发"), 1);
}

#[test]
fn learn_phrase_adds_an_unknown_word_as_a_user_word() {
    let mut engine = engine().with_learner(Box::new(WordLearner::default()));
    engine
        .learn_phrase("青简", &["qing".into(), "jian".into()])
        .unwrap();
    engine.set_input("qingjian");
    let items = engine.query().unwrap().candidates.items;
    assert_eq!(items[0].text, "青简");
    assert_eq!(engine.learner().choice_weight("qingjian", "青简"), 1);
}

#[test]
fn suggest_pinyin_uses_a_word_learned_after_the_index_was_built() {
    let mut engine = engine().with_learner(Box::new(WordLearner::default()));
    assert_eq!(engine.suggest_pinyin("青简"), None);
    engine
        .learn_phrase("青简", &["qing".into(), "jian".into()])
        .unwrap();
    assert_eq!(
        engine.suggest_pinyin("青简").as_deref(),
        Some(["qing".to_owned(), "jian".to_owned()].as_slice())
    );
}

#[test]
fn suggest_pinyin_picks_up_a_replaced_extra_dictionary() {
    let mut engine = reading_engine();
    assert_eq!(engine.suggest_pinyin("龘"), None);
    engine.set_extra_dictionaries(vec![Dictionary::parse("龘\tda\t1\n").unwrap()]);
    assert_eq!(
        engine.suggest_pinyin("龘").as_deref(),
        Some(["da".to_owned()].as_slice())
    );
}

#[test]
fn learn_phrase_works_when_learning_is_disabled() {
    let mut engine = engine().with_learner(Box::new(CountingLearner(HashMap::new())));
    engine.set_learning(false);
    engine
        .learn_phrase("开发", &["kai".into(), "fa".into()])
        .unwrap();
    assert_eq!(engine.learner().choice_weight("kaifa", "开发"), 1);
}

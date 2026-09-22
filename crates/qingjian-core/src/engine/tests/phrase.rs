//! 记词组：选中文本的读音反查，以及把「拼音 → 文本」记成用户词。

use super::*;

const READINGS: &str = "重庆\tchong qing\t1000\n重\tzhong\t5000\n庆\tqing\t2000\n";

/// 只认「重庆」这个词的假模型：让分词按整词走，而不是逐字。
struct ChongqingModel;

impl LanguageModel for ChongqingModel {
    fn log_prob(&self, _previous: Option<&str>, word: &str) -> Option<f64> {
        match word {
            "重庆" => Some(-2.0),
            "重" | "庆" => Some(-8.0),
            _ => None,
        }
    }
}

fn reading_engine() -> Engine {
    let dictionary = Dictionary::parse(READINGS).unwrap();
    Engine::new(dictionary).with_language_model(Box::new(ChongqingModel))
}

fn remembered_engine() -> Engine {
    engine().with_learner(Box::new(WordLearner::default()))
}

fn candidates_of(engine: &mut Engine, input: &str) -> Vec<String> {
    engine.set_input(input);
    engine
        .query()
        .unwrap()
        .candidates
        .items
        .into_iter()
        .map(|c| c.text)
        .collect()
}

#[test]
fn pinyin_of_prefers_the_word_reading_over_the_most_common_char_reading() {
    // 词库整词是 chong qing，逐字查「重」会取到词频更高的 zhong
    let mut engine = reading_engine();
    assert_eq!(engine.pinyin_of("重庆").as_deref(), Some("chong'qing"));
}

#[test]
fn pinyin_of_falls_back_to_chars_without_a_model() {
    let mut engine = Engine::new(Dictionary::parse(READINGS).unwrap());
    assert_eq!(engine.pinyin_of("重庆").as_deref(), Some("zhong'qing"));
    // 词库里没有的汉字：反查不出来
    assert_eq!(engine.pinyin_of("青简"), None);
}

#[test]
fn pinyin_of_skips_non_han_characters() {
    let mut engine = reading_engine();
    assert_eq!(engine.pinyin_of("重庆 123").as_deref(), Some("chong'qing"));
}

#[test]
fn a_remembered_phrase_becomes_a_candidate_of_its_pinyin() {
    let mut engine = remembered_engine();
    let remembered = engine.remember_phrase("qing'jian", "青简").unwrap();
    assert_eq!(remembered.text, "青简");
    assert_eq!(remembered.syllables, ["qing".to_owned(), "jian".to_owned()]);
    assert_eq!(remembered.replaced, None);
    // 词库里没有 qingjian，候选只能是刚记下的这条例
    assert_eq!(candidates_of(&mut engine, "qingjian"), ["青简"]);
}

#[test]
fn a_remembered_phrase_ranks_before_the_dictionary_words() {
    let mut engine = remembered_engine();
    engine.remember_phrase("kai'fan", "开饭了").unwrap();
    // 词库里的 开饭 是同一个输入串下的候选，但记下的这条例算选过一次，排它前面
    assert_eq!(
        candidates_of(&mut engine, "kaifan").first().map(String::as_str),
        Some("开饭了")
    );
}

#[test]
fn remembering_the_same_text_again_replaces_its_pinyin() {
    let mut engine = remembered_engine();
    engine.remember_phrase("qing'jian", "青简").unwrap();
    let again = engine.remember_phrase("qing'jianru", "青简").unwrap();
    assert_eq!(again.replaced.as_deref(), Some("qing jian"));
    assert_eq!(candidates_of(&mut engine, "qingjianru"), ["青简"]);
}

#[test]
fn bad_pinyin_is_rejected() {
    let mut engine = remembered_engine();
    assert!(matches!(
        engine.remember_phrase("kai1", "开发"),
        Err(PhraseError::BadPinyin(_))
    ));
    assert!(matches!(
        engine.remember_phrase("kaifa'zh", "开发"),
        Err(PhraseError::Incomplete)
    ));
    assert!(matches!(
        engine.remember_phrase("", "开发"),
        Err(PhraseError::EmptyPinyin)
    ));
    assert!(matches!(
        engine.remember_phrase("kai'fa", ""),
        Err(PhraseError::EmptyText)
    ));
}

#[test]
fn private_input_and_learning_off_refuse_to_remember() {
    let mut engine = remembered_engine();
    engine.set_private(true);
    assert!(matches!(
        engine.remember_phrase("kai'fa", "开发"),
        Err(PhraseError::Private)
    ));
    engine.set_private(false);
    engine.set_learning(false);
    assert!(matches!(
        engine.remember_phrase("kai'fa", "开发"),
        Err(PhraseError::LearningOff)
    ));
}

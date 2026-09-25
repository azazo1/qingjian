//! 双拼。

use super::*;

#[test]
fn shuangpin_decodes_keys_before_lookup_and_shows_full_pinyin() {
    let mut engine = xiaohe();
    engine.set_input("kdfa");
    let query = engine.query().unwrap();
    assert_eq!(query.candidates.items[0].text, "开发");
    assert_eq!(query.marked_text(), "kai'fa");
    assert_eq!(query.marked_cursor(), 6);
    assert_eq!(query.text, "kdfa");
    assert!(query.decoded_keys);
    // 末尾落单的键是声母前缀：`kdf` = kai f…
    engine.set_input("kdf");
    let query = engine.query().unwrap();
    assert_eq!(query.marked_text(), "kai'f");
    assert_eq!(query.candidates.items[0].text, "开放");
    // 配不出音节的键原样留作尾巴
    engine.set_input("kdbl");
    let query = engine.query().unwrap();
    assert_eq!(query.marked_text(), "kai'bl");
    assert_eq!(query.tail, "bl");
    assert_eq!(query.candidates.items[0].text, "开");
}

#[test]
fn shuangpin_shows_raw_keys_in_preedit_and_decoded_pinyin_in_segments() {
    let mut engine = xiaohe();
    engine.set_shuangpin_raw_preedit(true);
    engine.set_input("kdfa");
    let query = engine.query().unwrap();
    assert_eq!(query.candidates.items[0].text, "开发");
    // 输入框内 marked_text 显示原始输入按键
    assert_eq!(query.marked_text(), "kdfa");
    assert_eq!(query.marked_cursor(), 4);
    assert_eq!(query.text, "kdfa");
    // 候选窗口顶部拼音行保持显示解码全拼
    let segments_text: String = query
        .marked_segments()
        .iter()
        .map(|s| s.text.as_str())
        .collect();
    assert_eq!(segments_text, "kai'fa");
    assert_eq!(query.segments_cursor(), 6);

    // 移动光标到中间 kd|fa
    engine.move_cursor_left();
    engine.move_cursor_left();
    let query = engine.query().unwrap();
    assert_eq!(query.marked_text(), "kdfa");
    assert_eq!(query.marked_cursor(), 2);
    let segments_text: String = query
        .marked_segments()
        .iter()
        .map(|s| s.text.as_str())
        .collect();
    assert_eq!(segments_text, "kai'fa");
    assert_eq!(query.segments_cursor(), 3);

    // k|dfa：光标后的剩余段保留原始按键，和敲的对得上（双拼光标插进音节中间，单独解码会错开）
    engine.move_cursor_left();
    let query = engine.query().unwrap();
    assert_eq!(query.marked_text(), "kdfa");
    assert_eq!(query.marked_cursor(), 1);
    let segments_text: String = query
        .marked_segments()
        .iter()
        .map(|s| s.text.as_str())
        .collect();
    assert_eq!(segments_text, "k'dfa");
}

#[test]
fn shuangpin_commit_consumes_keys_per_syllable() {
    let mut engine = xiaohe();
    engine.set_input("kdfave");
    let query = engine.query().unwrap();
    let kaifa = query
        .candidates
        .items
        .iter()
        .find(|c| c.text == "开发")
        .cloned()
        .unwrap();
    engine.commit(&kaifa);
    assert_eq!(engine.composition().text(), "ve");
    // 选中的 开发 延迟上屏，preedit 里排在还没确认的拼音前面
    assert_eq!(engine.query().unwrap().marked_text(), "开发zhe");
    // 未打完的最后一个键也被候选吃掉
    engine.set_input("kdf");
    let kaifang = engine.query().unwrap().candidates.items[0].clone();
    engine.commit(&kaifang);
    assert!(engine.composition().is_empty());
}

#[test]
fn shuangpin_records_choices_by_full_pinyin() {
    let mut engine = xiaohe();
    engine.set_input("kdfa");
    let query = engine.query().unwrap();
    // 两键定局：`fa` 不再按前缀放宽成 fan / fang
    assert!(query.candidates.items.iter().all(|c| c.text != "开饭"));
    let kaifa = query.candidates.items[0].clone();
    assert_eq!(kaifa.text, "开发");
    engine.commit(&kaifa);
    // 学习记的是全拼 kaifa：切回全拼、同样的拼音也受益
    assert!(engine.recent_commits.last().unwrap().same_input("kaifa"));
}

#[test]
fn shuangpin_moves_mode_keys_to_shifted_letters() {
    let mut engine = xiaohe();
    engine.set_input("v");
    assert!(!engine.expression_mode());
    assert_eq!(engine.query().unwrap().marked_text(), "zh");
    engine.set_input("u1");
    assert!(!engine.question_mode());
    // Shift+V / Shift+U 进模式，之后与全拼下的 v / u 一样
    assert!(engine.takes_mode_letter('V') && engine.takes_mode_letter('U'));
    assert!(!engine.takes_mode_letter('v') && !engine.takes_mode_letter('I'));
    engine.set_input("V1+2");
    assert!(engine.expression_mode() && !engine.raw_mode());
    assert_eq!(engine.query().unwrap().candidates.items[0].text, "3");
    engine.set_input("U4e00");
    assert!(engine.question_mode() && engine.unicode_entry());
    assert_eq!(engine.query().unwrap().candidates.items[0].text, "一");
    engine.set_input("Unihc");
    assert!(engine.question_mode());
    assert_eq!(engine.query().unwrap().marked_text(), "Uni'hao");
    // 全拼下大写字母不是入口
    let mut full = super::engine();
    assert!(!full.takes_mode_letter('V'));
    full.set_input("V1+2");
    assert!(!full.expression_mode());
    // `?` 入口开着时照常进问字（缺省关）
    engine.set_input("?nihc");
    assert!(!engine.question_mode());
    engine.set_mode_keys(ModeKeys {
        question_mark: true,
        ..ModeKeys::default()
    });
    engine.set_input("?nihc");
    assert!(engine.question_mode());
    assert_eq!(engine.query().unwrap().marked_text(), "?ni'hao");
}

#[test]
fn microsoft_semicolon_is_a_final_only_after_a_lone_initial() {
    let mut engine = engine();
    engine.set_shuangpin(Some(Scheme::Microsoft));
    engine.set_input("x");
    assert!(engine.takes_semicolon());
    engine.push(';');
    assert!(!engine.takes_semicolon());
    assert!(!engine.raw_mode());
    assert_eq!(engine.query().unwrap().marked_text(), "xing");
    // 问字模式里也认：`?x;` 问的是 xing
    engine.set_mode_keys(ModeKeys {
        question_mark: true,
        ..ModeKeys::default()
    });
    engine.set_input("?x");
    assert!(engine.takes_semicolon());
    engine.push(';');
    assert!(engine.question_mode());
    assert_eq!(engine.query().unwrap().marked_text(), "?xing");
    engine.set_shuangpin(Some(Scheme::Xiaohe));
    engine.set_input("x");
    assert!(!engine.takes_semicolon());
}

#[test]
fn shuangpin_raw_commit_does_not_learn_decodable_keys_as_english() {
    let mut engine = xiaohe();
    engine.set_input("nihc");
    assert!(!looks_like_english_word_in(&engine));
    engine.set_input("gist");
    assert!(looks_like_english_word_in(&engine));
}

#[test]
fn adjusting_frequency_under_shuangpin_counts_on_the_decoded_pinyin() {
    // 排序查 "这个输入串下选过什么" 用的是解出的全拼 (`mokuai`), 调频也得记到那把键上:
    // 记到原始按键 (`mokk`) 上的次数排序看不到, 双拼下调频就完全不起作用
    let dict = Dictionary::parse("模块\tmo kuai\t1071\n莫快\tmo kuai\t5000\n").unwrap();
    let mut engine = Engine::new(dict).with_learner(Box::new(CountingLearner(HashMap::new())));
    engine.set_shuangpin(Some(Scheme::Xiaohe));
    engine.set_input("mokk");
    assert_eq!(texts_of(&engine)[0], "莫快");
    let word = engine
        .query()
        .unwrap()
        .candidates
        .items
        .into_iter()
        .find(|c| c.text == "模块")
        .unwrap();
    assert!(engine.adjust_frequency(&word, true).changed);
    assert_eq!(engine.learner().choice_weight("mokuai", "模块"), 1);
    assert_eq!(engine.learner().choice_weight("mokk", "模块"), 0);
    assert_eq!(texts_of(&engine)[0], "模块");
}

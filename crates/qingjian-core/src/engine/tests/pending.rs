//! 延迟上屏: 这段拼音还没选完时, 选中的词先留在 preedit 里 (退格能拆回, 见 `engine::pending`)。

use super::*;

/// 按文本找一个候选上屏, 返回 [`Engine::commit`] 交出来的文本。
fn commit_text(engine: &mut Engine, text: &str) -> String {
    let candidate = engine
        .query()
        .unwrap()
        .candidates
        .items
        .into_iter()
        .find(|c| c.text == text)
        .unwrap_or_else(|| panic!("候选 {text} 不在列表里"));
    engine.commit(&candidate)
}

#[test]
fn a_word_picked_before_the_end_of_the_buffer_waits_in_the_preedit() {
    let mut engine = engine();
    engine.set_input("kaifazhe");
    // 后面还有 zhe 没选: 这次上屏不交给应用, 词留在 Engine 里
    assert_eq!(commit_text(&mut engine, "开发"), "");
    assert!(engine.has_pending());
    assert_eq!(engine.pending_text(), "开发");
    assert_eq!(engine.composition().text(), "zhe");

    // preedit 里排在还没确认的拼音前面, 光标跟着往后挪
    let query = engine.query().unwrap();
    assert_eq!(query.pending, "开发");
    assert_eq!(query.marked_text(), "开发zhe");
    assert_eq!(query.marked_cursor(), 5);
}

/// 双拼开着 `shuangpin_raw_preedit` 时 preedit 换成原始按键, 延迟上屏的已选词仍要排在它前面.
#[test]
fn raw_preedit_keeps_the_pending_word_in_front() {
    let mut engine = xiaohe();
    engine.set_shuangpin_raw_preedit(true);
    engine.set_input("kdfave");
    assert_eq!(commit_text(&mut engine, "开发"), "");
    assert_eq!(engine.pending_text(), "开发");
    assert_eq!(engine.composition().text(), "ve");

    let query = engine.query().unwrap();
    // 拼音那半截是原始按键 (`ve`), 已选词照旧在前面, 光标从这里往后算
    assert_eq!(query.marked_text(), "开发ve");
    assert_eq!(query.marked_cursor(), 4);
    // 候选窗口那侧仍是解出的全拼
    let segments_text: String = query
        .marked_segments()
        .iter()
        .map(|s| s.text.as_str())
        .collect();
    assert_eq!(segments_text, "开发zhe");
    assert_eq!(query.segments_cursor(), 5);
}

#[test]
fn backspace_takes_the_selected_word_back_into_the_buffer() {
    let mut engine = engine();
    engine.set_input("kaifazhe");
    assert_eq!(commit_text(&mut engine, "开发"), "");

    // 退格先拆选中的词: 键还回缓冲区开头, 候选重新按整段拼音算
    assert!(engine.backspace());
    assert!(!engine.has_pending());
    assert_eq!(engine.pending_text(), "");
    assert_eq!(engine.composition().text(), "kaifazhe");
    assert_eq!(engine.composition().cursor(), 8);
    // 候选重新按整段拼音算
    assert!(
        engine
            .query()
            .unwrap()
            .candidates
            .items
            .iter()
            .any(|c| c.text == "开发者")
    );

    // 拆完再退格才是删字符
    assert!(engine.backspace());
    assert_eq!(engine.composition().text(), "kaifazh");
}

#[test]
fn shuangpin_keys_come_back_the_way_they_were_typed() {
    let mut engine = xiaohe();
    // 小鹤: kd = kai, fa = fa, ve = zhe
    engine.set_input("kdfave");
    assert_eq!(commit_text(&mut engine, "开发"), "");
    assert_eq!(engine.pending_text(), "开发");

    assert!(engine.backspace());
    assert_eq!(engine.composition().text(), "kdfave");
    assert_eq!(engine.decode("kdfave").unwrap().pinyin(), "kai'fa'zhe");
}

#[test]
fn finishing_the_buffer_hands_out_everything_at_once() {
    let mut engine = engine();
    engine.set_input("kaifazhe");
    assert_eq!(commit_text(&mut engine, "开发"), "");

    // 剩下的拼音回车原样上屏: 延迟的 开发 排在前面一起交出去
    assert_eq!(engine.take_raw(), "开发zhe");
    assert!(!engine.has_pending());
    assert!(engine.composition().is_empty());
}

#[test]
fn cancelling_the_buffer_keeps_the_selected_word() {
    let mut engine = engine();
    engine.set_input("kaifazhe");
    assert_eq!(commit_text(&mut engine, "开发"), "");

    // 取消组句取消的是还没确认的拼音, 选过的字照样上屏
    assert_eq!(engine.clear(), "开发");
    assert!(!engine.has_pending());
    assert!(engine.composition().is_empty());
}

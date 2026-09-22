//! 行内 (应用侧 marked text) 的显示: 双拼下显示敲的键, 候选窗口的拼音行仍显示解出的全拼.

use qingjian_dictionary::AuxCodeTable;

use super::*;

fn aux_codes() -> Arc<dyn AuxCodeLookup> {
    Arc::new(
        AuxCodeTable::from_pairs([
            ("开发".to_owned(), "kf".to_owned()),
            ("开发者".to_owned(), "kfz".to_owned()),
        ])
        .unwrap(),
    )
}

/// 双拼下窗口那侧是解出的全拼, 行内那侧是敲的键.
#[test]
fn shuangpin_inline_shows_the_typed_keys() {
    let mut engine = xiaohe();
    engine.set_input("kdfave");
    let query = engine.query().unwrap();
    assert_eq!(query.marked_text(), "kai'fa'zhe");
    assert_eq!(query.inline_text(), "kd'fa've");
    assert_eq!(query.marked_cursor(), 10);
    assert_eq!(query.inline_cursor(), 8);
    // 落单的键与解不动的尾巴不在全拼里, 但要留在行内
    engine.set_input("kdf");
    let query = engine.query().unwrap();
    assert_eq!(query.marked_text(), "kai'f");
    assert_eq!(query.inline_text(), "kd'f");
    engine.set_input("kdbl");
    let query = engine.query().unwrap();
    assert_eq!(query.marked_text(), "kai'bl");
    assert_eq!(query.inline_text(), "kd'bl");
}

/// 全拼没有"敲的键"这一层, 行内与窗口仍走同一条老路.
#[test]
fn full_pinyin_inline_keeps_the_expanded_form() {
    let mut engine = engine();
    engine.set_input("kaifa");
    let query = engine.query().unwrap();
    assert!(query.keys_display.is_none());
    assert_eq!(query.inline_text(), "kai'fa");
    assert_eq!(query.inline_text(), query.marked_text());
    assert_eq!(query.inline_cursor(), query.marked_cursor());
}

/// 注音行内显示的是注音符号, 不换成键.
#[test]
fn zhuyin_inline_stays_the_symbols() {
    let mut engine = engine();
    engine.set_zhuyin_mode(true);
    engine.set_input("1j4");
    let query = engine.query().unwrap();
    assert!(query.keys_display.is_none());
    assert_eq!(query.inline_text(), "ㄅㄨˋ");
    assert_eq!(query.inline_text(), query.marked_text());
}

/// 光标停在中间时行内仍是整段键: 光标后的键接在末尾, 光标落在敲的部分末尾.
#[test]
fn inline_keeps_the_keys_after_the_cursor() {
    let mut engine = xiaohe();
    engine.set_input("kdfave");
    engine.move_cursor_left();
    engine.move_cursor_left();
    let query = engine.query().unwrap();
    assert_eq!(query.rest_keys, "ve");
    // 窗口那侧连光标后的部分也是解出的全拼, 行内那侧全是敲的键
    assert_eq!(query.marked_text(), "kai'fa'zhe");
    assert_eq!(query.inline_text(), "kd'fa've");
    assert_eq!(query.inline_cursor(), 5);
}

/// 辅码态: 触发键与码段也按敲的键接在行内.
#[test]
fn inline_appends_the_aux_code() {
    let mut engine = xiaohe().with_aux_codes(vec![aux_codes()]);
    engine.set_aux_enabled(true);
    engine.set_input("kdfa");
    assert!(engine.aux_trigger(';'));
    engine.enter_aux();
    assert!(engine.push_aux_code('k'));
    let query = engine.query().unwrap();
    assert_eq!(query.marked_text(), "kai'fa;k");
    assert_eq!(query.inline_text(), "kd'fa;k");
    assert_eq!(query.inline_cursor(), 7);
}

/// 五笔与拼音混输时拼音侧照样是敲的键.
#[test]
fn mixed_input_inline_shows_the_keys() {
    let mut engine = xiaohe();
    engine.set_code_table(Some(CodeTable::parse("开\tga\t5000\n").unwrap()));
    engine.set_input("kdfa");
    let query = engine.query().unwrap();
    assert!(query.keys_display.is_some());
    assert_eq!(query.inline_text(), "kd'fa");
}

/// 残缺切分, 解不动的尾巴等分支都不会让行内丢掉敲过的键.
#[test]
fn inline_never_drops_a_typed_key() {
    let mut engine = xiaohe();
    for keys in ["kdfave", "mignt", "nihc", "jintiantianqihenhao", "kdbl"] {
        engine.set_input(keys);
        let query = engine.query().unwrap();
        assert_eq!(
            query.inline_text().replace('\'', ""),
            keys,
            "输入 {keys} 的行内丢了键"
        );
    }
}

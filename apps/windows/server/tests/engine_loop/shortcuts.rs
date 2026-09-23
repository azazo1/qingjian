//! 组句中的快捷键：修饰键 + 数字（译词上屏、删候选）与调频键 + J / K。

use qingjian_core::WordFrequency;

use crate::support::*;

#[test]
fn alt_digit_commits_first_translation() {
    let mut router = router();
    let (_, _, frame) = type_letters(&mut router, "nihao");
    let slot = slot_of(&frame, "你好");
    // 缺省译词键（mac ⌥ / Windows Ctrl）+ 数字：上屏那个候选的第一个译词，组句结束。
    let (outcome, commit, after) = press(&mut router, digit_with(slot, TRANSLATE));
    assert_eq!(
        (outcome, commit.as_deref()),
        (KeyOutcome::Consumed, Some("hello"))
    );
    assert!(after.is_empty());
}

#[test]
fn second_translation_key_without_second_sense_is_swallowed() {
    let mut router = router();
    let (_, _, frame) = type_letters(&mut router, "nihao");
    let slot = slot_of(&frame, "你好");
    // 样例释义表里「你好」只有一条译文：第二个译词键（Shift+译词键）+ 数字吞掉不动，组句还在。
    let (outcome, commit, after) = press(&mut router, digit_with(slot, TRANSLATE_SECOND));
    assert_eq!((outcome, commit), (KeyOutcome::Consumed, None));
    assert_eq!(preedit(&after), "ni'hao");
}

#[test]
fn shift_digit_forgets_candidate_and_requeries() {
    let mut router = router();
    let (_, _, frame) = type_letters(&mut router, "nihao");
    let slot = slot_of(&frame, "你好");
    // 缺省 Shift + 数字：删候选（词库词只清学习记录），重新查一遍，组句不变。
    let (outcome, commit, after) = press(&mut router, digit_with(slot, SHIFT));
    assert_eq!((outcome, commit), (KeyOutcome::Consumed, None));
    assert_eq!(preedit(&after), "ni'hao");
    assert!(!after.candidates.items.is_empty());
}

#[test]
fn unconfigured_modifier_digit_is_not_a_selection() {
    // 删候选改成 Ctrl+Shift：Shift+4 就是普通的 `$`；Win+1 没配到快捷键，归应用。
    let mut router = router_with(RouterConfig {
        delete_keys: Some(KeyModifiers {
            shift: true,
            ..CTRL
        }),
        ..RouterConfig::default()
    });
    type_letters(&mut router, "nihao");
    let (outcome, commit, frame) = press(&mut router, digit_with(4, SHIFT));
    assert_eq!((outcome, commit), (KeyOutcome::Consumed, None));
    assert!(preedit(&frame).contains('$'), "{}", preedit(&frame));
    let (outcome, commit, _) = press(&mut router, digit_with(1, WIN));
    assert_eq!((outcome, commit), (KeyOutcome::Passthrough, None));
}

#[test]
fn deleting_a_candidate_shows_a_notice_until_next_key() {
    let mut router = router();
    let (_, _, frame) = type_letters(&mut router, "nihao");
    let slot = slot_of(&frame, "你好");

    // 「你好」是词库词且没学习记录，删不掉，但提示照样给出。
    let (outcome, _, after) = press(&mut router, digit_with(slot, SHIFT));
    assert_eq!(outcome, KeyOutcome::Consumed);
    assert!(!after.is_empty(), "删候选后仍在组句");
    let notice = after.notice.as_deref().expect("删候选后应带屏幕提示");
    assert!(notice.contains("你好"), "提示应提到候选词，实际：{notice}");

    let (_, _, next) = press(&mut router, KeyEvent::new(0x28, None, Default::default())); // VK_DOWN
    assert_eq!(next.notice, None, "提示应只活到下一次按键");
}

#[test]
fn adjust_keys_preview_frequencies_and_move_the_highlighted_candidate() {
    let mut router = router();
    let (_, _, frame) = type_letters(&mut router, "nihao");
    assert!(frame.frequencies.is_empty(), "没按调频键时不带频次");

    // 按住缺省调频键（Ctrl）：帧里按候选逐项带上频次，而修饰键本身照旧归应用
    let (outcome, _, held) = press(&mut router, KeyEvent::new(0x11, None, CTRL)); // VK_CONTROL
    assert_eq!(outcome, KeyOutcome::Passthrough, "调频修饰键本身不拦");
    assert_eq!(held.frequencies.len(), held.candidates.items.len());
    let highlighted = held.highlight;
    let word = held.candidates.items[highlighted].text.clone();
    assert_eq!(
        held.frequencies[highlighted],
        Some(WordFrequency {
            total: 0,
            selected: 0
        })
    );

    // Ctrl + K：这个候选升一格（相当于又选一次），高亮跟着它走
    let (outcome, commit, up) = press(&mut router, letter_with('k', CTRL));
    assert_eq!((outcome, commit), (KeyOutcome::Consumed, None));
    assert_eq!(up.candidates.items[up.highlight].text, word);
    assert_eq!(
        up.frequencies[up.highlight],
        Some(WordFrequency {
            total: 1,
            selected: 1
        })
    );

    // Ctrl + J 降回来；已经到底之后再降只给提示，计数不动
    let (_, _, down) = press(&mut router, letter_with('j', CTRL));
    assert_eq!(
        down.frequencies[down.highlight],
        Some(WordFrequency {
            total: 0,
            selected: 0
        })
    );
    let (_, _, floor) = press(&mut router, letter_with('j', CTRL));
    let notice = floor.notice.as_deref().expect("降到底应给一句提示");
    assert!(notice.contains(&word), "提示应提到候选词，实际：{notice}");

    // 抬起调频键：频次收起
    let (outcome, _, released) = press(&mut router, KeyEvent::new(0x11, None, CTRL).released());
    assert_eq!(outcome, KeyOutcome::Passthrough);
    assert!(released.frequencies.is_empty(), "抬起后不再显示频次");
}

#[test]
fn adjust_keys_do_nothing_outside_composition() {
    let mut router = router();
    // 没在组句：Ctrl + K 原样归应用（终端里它是换行）
    let (outcome, _, frame) = press(&mut router, letter_with('k', CTRL));
    assert_eq!(outcome, KeyOutcome::Passthrough);
    assert!(frame.frequencies.is_empty());

    // 配成 none 关掉：组句里也不再是调频键
    let mut off = router_with(RouterConfig {
        adjust_keys: None,
        ..RouterConfig::default()
    });
    type_letters(&mut off, "nihao");
    let (outcome, _, frame) = press(&mut off, letter_with('k', CTRL));
    assert_eq!(
        outcome,
        KeyOutcome::Passthrough,
        "关掉调频键后 Ctrl+字母照旧归应用"
    );
    assert!(frame.frequencies.is_empty());
}

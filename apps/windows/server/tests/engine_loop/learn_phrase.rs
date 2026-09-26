//! 「录入词组」：反查拼音、校验手填、写入用户词（窗口在 UI 线程，这里测它请工人线程办的活）。

use qingjian_windows_server::dispatch::{LearnPhraseReply, LearnPhraseWork};

use crate::support::*;

/// 词库里的整词直接反查出拼音。
#[test]
fn suggest_pinyin_uses_the_dictionary() {
    let mut router = router();

    assert_eq!(
        router.learn_phrase(LearnPhraseWork::Suggest("你好".to_owned())),
        LearnPhraseReply::Pinyin(Some("ni hao".to_owned()))
    );
}

/// 词库里没有的字不给建议，留给用户手填。
#[test]
fn suggest_pinyin_leaves_unknown_words_to_the_user() {
    let mut router = router();

    assert_eq!(
        router.learn_phrase(LearnPhraseWork::Suggest("青简".to_owned())),
        LearnPhraseReply::Pinyin(None)
    );
}

/// 录入一个新词：写进用户词，之后反查得到，打字也能出。
#[test]
fn confirm_learns_a_phrase() {
    let mut router = router();

    assert_eq!(
        router.learn_phrase(LearnPhraseWork::Confirm {
            text: "青简".to_owned(),
            pinyin: "qing jian".to_owned(),
        }),
        LearnPhraseReply::Learned
    );
    assert_eq!(
        router.learn_phrase(LearnPhraseWork::Suggest("青简".to_owned())),
        LearnPhraseReply::Pinyin(Some("qing jian".to_owned())),
        "录入之后应当能从用户词反查回来"
    );

    let (_, _, frame) = type_letters(&mut router, "qingjian");
    assert!(
        candidate_texts(&frame).contains(&"青简"),
        "录入的词应当进候选: {:?}",
        candidate_texts(&frame)
    );
}

/// 拼音连写（不带空格）也能用。
#[test]
fn confirm_accepts_joined_pinyin() {
    let mut router = router();

    assert_eq!(
        router.learn_phrase(LearnPhraseWork::Confirm {
            text: "青简".to_owned(),
            pinyin: "qingjian".to_owned(),
        }),
        LearnPhraseReply::Learned
    );
}

/// 音节数对不上字数、词组为空：给出与 macOS 一致的说法。
#[test]
fn confirm_reports_bad_input() {
    let mut router = router();

    assert_eq!(
        router.learn_phrase(LearnPhraseWork::Confirm {
            text: "你好".to_owned(),
            pinyin: "ni".to_owned(),
        }),
        LearnPhraseReply::Error("拼音是 1 个音节, 词组是 2 个字".to_owned())
    );
    assert_eq!(
        router.learn_phrase(LearnPhraseWork::Confirm {
            text: "  ".to_owned(),
            pinyin: "ni".to_owned(),
        }),
        LearnPhraseReply::Error("请输入词组".to_owned())
    );
}

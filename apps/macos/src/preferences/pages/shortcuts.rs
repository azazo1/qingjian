//! 「快捷键」页：翻页键、模式键、译词上屏 / 删候选 / 翻译选中文字的组合键。

use objc2::MainThreadMarker;
use objc2::rc::Retained;
use objc2_app_kit::{NSButton, NSPopUpButton};
use qingjian_core::ModeKeys;
use qingjian_platform::{Config, KeyBinding, KeyCombo, Modifiers, PAGE_KEY_OPTIONS};

use crate::preferences::controls::{
    GROUP_GAP, button, checkbox, note, note_full, page_keys_label, row_checkbox, row_popup,
    row_recorder, select, set_checked,
};
use crate::preferences::key_recorder::{KeyRecorder, OFF_KEY, OFF_LABEL, RecorderKind};
use crate::preferences::layout::{Layout, PAGE_PADDING, ROW_HEIGHT};
use crate::preferences::setting::Setting;
use crate::preferences::target::PreferencesTarget;

pub struct ShortcutsPage {
    /// 翻页键对。
    page_keys: Retained<NSPopUpButton>,

    /// 表达式模式键。
    expression: Retained<NSPopUpButton>,

    /// 问字模式键。
    question: Retained<NSPopUpButton>,

    /// 没在组句时敲 `?` 也进问字。
    question_mark: Retained<NSButton>,

    /// 上屏第一个译词的修饰键。
    translation: Retained<KeyRecorder>,

    /// 上屏第二个译词的修饰键。
    translation_second: Retained<KeyRecorder>,

    /// 删除候选的修饰键。
    delete_candidate: Retained<KeyRecorder>,

    /// 翻译选中文字的组合键。
    translate_selection: Retained<KeyRecorder>,

    /// 记词组的组合键（缺省没配）。
    learn_phrase: Retained<KeyRecorder>,
}

impl ShortcutsPage {
    pub fn build(layout: &mut Layout, mtm: MainThreadMarker, target: &PreferencesTarget) -> Self {
        let page_key_titles: Vec<String> = PAGE_KEY_OPTIONS
            .iter()
            .map(|k| page_keys_label(k))
            .collect();
        let page_keys = row_popup(
            layout,
            mtm,
            "翻页键",
            &page_key_titles,
            Setting::PageKeys,
            target,
        );
        note(
            layout,
            mtm,
            "选「，  。」时组句中敲逗号句号是翻页，不再是上屏加标点。",
        );
        let key_titles: Vec<String> = ModeKeys::CANDIDATES.iter().map(char::to_string).collect();
        let expression = row_popup(
            layout,
            mtm,
            "表达式模式键",
            &key_titles,
            Setting::ExpressionKey,
            target,
        );
        let question = row_popup(
            layout,
            mtm,
            "问字模式键",
            &key_titles,
            Setting::QuestionKey,
            target,
        );
        note(
            layout,
            mtm,
            "这两个字母开头进模式：v1+2 出 3，usangemu 问「三个木」（需要云服务），u4e00 出对应的字符。两个键不能相同。",
        );
        let question_mark = checkbox(
            mtm,
            "没在输入拼音时敲 ? 也进入问字",
            Setting::QuestionMark,
            target,
        );
        row_checkbox(layout, &question_mark);
        note(
            layout,
            mtm,
            "勾上后 ? 先进问字（中英文模式都行），后面跟字母才是问题，跟空格、回车等其他键时还原成问号；不勾问号就是问号。",
        );
        layout.space(GROUP_GAP);
        let translation = row_recorder(
            layout,
            mtm,
            "上屏第一个译词",
            Setting::TranslationKeys,
            RecorderKind::ModifiersOnly,
            target,
        );
        let translation_second = row_recorder(
            layout,
            mtm,
            "上屏第二个译词",
            Setting::TranslationSecondKeys,
            RecorderKind::ModifiersOnly,
            target,
        );
        note(
            layout,
            mtm,
            "按住修饰键再按候选序号，上屏的是候选右侧的译词而不是中文；候选有两个译词时第二组键上屏后一个。两组不能相同。",
        );
        layout.space(GROUP_GAP);
        let delete_candidate = row_recorder(
            layout,
            mtm,
            "删除候选",
            Setting::DeleteCandidateKeys,
            RecorderKind::ModifiersOnly,
            target,
        );
        note(
            layout,
            mtm,
            "按住修饰键再按候选序号：自己造的词、云端选过的词整个删掉；词库里的词清掉对它的学习记录，回到原来的排序。组句中要打感叹号先把词上屏。",
        );
        layout.space(GROUP_GAP);
        let translate_selection = row_recorder(
            layout,
            mtm,
            "翻译选中的文字",
            Setting::TranslateSelectionKeys,
            RecorderKind::Combo,
            target,
        );
        note(
            layout,
            mtm,
            "在应用里选中一段文字再按这个键，译文（学习语言）出现在候选窗口：回车替换选中的文字，Esc 保留原文。需要开着云服务。",
        );
        layout.space(GROUP_GAP);
        let learn_phrase = row_recorder(
            layout,
            mtm,
            "记词组（默认未设置）",
            Setting::LearnPhraseKeys,
            RecorderKind::Combo,
            target,
        );
        note(
            layout,
            mtm,
            "在应用里选中一段文字再按这个键：候选窗口显示它的拼音（可退格改），回车把这段文字记成用户词，下次敲这段拼音就有它；Esc 取消。选中的文字不会被改动。",
        );
        layout.space(GROUP_GAP);
        note_full(
            layout,
            mtm,
            "改快捷键: 点一下右边的按钮, 再按下新的组合键 (要带修饰键 ⌃ ⌥ ⇧ ⌘), Esc 取消, ⌫ 清空 (清空后这项键不生效). 避开 ⌃+数字 (切换桌面) 和 ⌘+数字 / ⌘T (应用常用键).",
        );
        let reset = button(mtm, "恢复默认快捷键", Setting::ResetShortcuts, target);
        layout.place(&reset, PAGE_PADDING, 160.0, ROW_HEIGHT + 4.0);
        layout.next_row(ROW_HEIGHT + 4.0);
        layout.space(GROUP_GAP);
        note_full(
            layout,
            mtm,
            "组句中固定的键（不可改）：空格上屏首选，1–9 选词，回车原样上屏，Esc 清空；⌥⌫ 删一个音节，⌘⌫ 删到开头；\
             ⌥← / ⌥→ 按音节跳光标，⌘← / ⌘→ 到开头 / 末尾；上 / 下移动高亮，PageUp / PageDown 与 ⇧Tab 翻页；\
             Tab 接受云端整句补全（没有就翻页）；半角标点进入英文直输段。",
        );
        Self {
            page_keys,
            expression,
            question,
            question_mark,
            translation,
            translation_second,
            delete_candidate,
            translate_selection,
            learn_phrase,
        }
    }

    pub fn sync(&self, config: &Config) {
        let (previous, next) = config.general.page_keys();
        let pair = format!("{previous}{next}");
        select(
            &self.page_keys,
            PAGE_KEY_OPTIONS.iter().position(|k| *k == pair),
        );
        let keys = config.shortcut.mode.sanitized();
        select(
            &self.expression,
            ModeKeys::CANDIDATES
                .iter()
                .position(|k| *k == keys.expression),
        );
        select(
            &self.question,
            ModeKeys::CANDIDATES
                .iter()
                .position(|k| *k == keys.question),
        );
        set_checked(&self.question_mark, keys.question_mark);
        let (first, second) = config.shortcut.translation_keys();
        show_modifiers(&self.translation, first);
        show_modifiers(&self.translation_second, second);
        show_modifiers(&self.delete_candidate, config.shortcut.delete_keys());
        show_combo(
            &self.translate_selection,
            config.shortcut.translate_selection,
        );
        show_combo(&self.learn_phrase, config.shortcut.learn_phrase);
    }
}

/// 一行修饰键录制按钮按配置刷新; 配成 `none` 的显示「未设置」.
fn show_modifiers(recorder: &KeyRecorder, binding: KeyBinding<Modifiers>) {
    match binding.key() {
        Some(keys) => recorder.show(&keys.key(), &keys.label()),
        None => recorder.show(OFF_KEY, OFF_LABEL),
    }
}

/// 同上, 记组合键的那一行 (`control+option+t`).
fn show_combo(recorder: &KeyRecorder, binding: KeyBinding<KeyCombo>) {
    match binding.key() {
        Some(combo) => recorder.show(&combo.key_string(), &combo.label()),
        None => recorder.show(OFF_KEY, OFF_LABEL),
    }
}

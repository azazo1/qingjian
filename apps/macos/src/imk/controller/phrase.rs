//! 记词组：读出应用里的选区，反查读音预填拼音，回车把「拼音 → 文本」记成一条用户词。
//!
//! 评审期间应用里的选区一个字符都不动，也不放 marked text：拼音行与提示都画在候选窗口里，
//! 用户敲的字母只改壳里那份拼音字符串。与「翻译选中文字」是同一套流程（读选区 -> 评审态 -> 按键归这里）。

use super::*;
use qingjian_core::PhraseError;

impl QingjianInputController {
    /// 记词组快捷键：读应用里的选区 -> 反查读音 -> 进评审态。读不到选区就提示一句，键照旧吃掉。
    pub(super) fn learn_phrase(&self, client: TextClient<'_>) -> bool {
        if secure_input::enabled() {
            tracing::debug!("Secure Input 中，不记词组");
            return false;
        }
        let Some((text, _)) = client.selected_text(MAX_TRANSLATE_CHARS) else {
            // 分不清是没选还是应用不给读（不少 Electron 应用不支持），两种情况都提示一下
            tracing::debug!("没有选中的文字，或应用不支持读选区");
            let anchor = client.caret_rect();
            host::with(|h| {
                h.show_notice(
                    "先选中一段文字再按这个键；这个应用也可能不支持读取选区（最多 500 字）",
                    anchor,
                )
            });
            return true;
        };
        // 取光标位置要等应用回话，放在借用 Host 之外（见 `translate_selection`）
        let anchor = client.caret_rect();
        let started = std::time::Instant::now();
        let chars = text.chars().count();
        host::with(|h| {
            h.anchor = anchor;
            // 反查表第一次用时要建，几十毫秒级；建完留在引擎里，之后按快捷键不再付这份代价
            let pinyin = h.engine.pinyin_of(&text).unwrap_or_default();
            let missing = pinyin.is_empty();
            h.begin_phrase(text, pinyin);
            if missing {
                h.status = Some("词库读不出这段文字的读音，请自己敲拼音 · 回车记词组".to_owned());
                h.render();
            }
            tracing::debug!(chars, ms = started.elapsed().as_millis(), "记词组：进入评审态");
        });
        true
    }

    /// 评审态下的按键：字母与 `'` 追加、退格删末位、回车记下、Esc 放弃，其余键吞掉不动。
    /// 带 ⌘ / ⌃ 的组合（复制、切换输入法这些）不归评审：放弃这次记词组，键交回应用。
    pub(super) fn handle_phrase_review(&self, event: &NSEvent, client: TextClient<'_>) -> bool {
        let flags = event.modifierFlags();
        if flags.intersects(NSEventModifierFlags::Command | NSEventModifierFlags::Control) {
            tracing::debug!("记词组：带命令修饰键的键，放弃并交回应用");
            host::with(|h| h.end_phrase());
            return false;
        }
        match event.keyCode() {
            // 回车 / 小键盘回车：记下
            36 | 76 => self.commit_phrase(client),
            // Esc：放弃，应用里的选区本来就没人动过
            53 => {
                tracing::debug!("记词组：放弃");
                host::with(|h| h.end_phrase());
            }
            // 退格：删掉拼音的最后一位
            51 => {
                host::with(|h| {
                    if let Some(job) = h.phrase.as_mut() {
                        job.pinyin.pop();
                    }
                    h.refresh_phrase();
                });
            }
            _ => {
                let typed: String = event
                    .characters()
                    .map(|c| c.to_string())
                    .unwrap_or_default()
                    .chars()
                    .filter(|c| c.is_ascii_alphabetic() || *c == '\'')
                    .map(|c| c.to_ascii_lowercase())
                    .collect();
                if typed.is_empty() {
                    // 数字、空格、方向键这类：评审里没意义，吞掉当没按
                    return true;
                }
                host::with(|h| {
                    if let Some(job) = h.phrase.as_mut() {
                        job.pinyin.push_str(&typed);
                    }
                    h.refresh_phrase();
                });
            }
        }
        true
    }

    /// 回车：把当前的「拼音 → 选中文本」记成用户词。记下了就提示一句并收窗，
    /// 没记成（拼音还没打完、私密输入等）就留着评审态让用户接着改。
    fn commit_phrase(&self, _client: TextClient<'_>) {
        let outcome = host::with(|h| {
            let job = h.phrase.clone()?;
            let result = h.engine.remember_phrase(&job.pinyin, &job.text);
            Some((job, result))
        })
        .flatten();
        let Some((job, result)) = outcome else {
            return;
        };
        match result {
            Ok(remembered) => {
                let message = match remembered.replaced {
                    Some(_) => format!("已更新「{}」的拼音", remembered.text),
                    None => format!("已记住词组「{}」", remembered.text),
                };
                tracing::info!(text = %remembered.text, pinyin = %job.pinyin, "记下用户词组");
                host::with(|h| {
                    h.end_phrase();
                    h.engine.flush_learning();
                    h.show_notice(&message, h.anchor);
                });
            }
            Err(error) => {
                tracing::debug!(%error, "没记成词组");
                let message = phrase_error_message(&error);
                host::with(|h| {
                    h.status = Some(message);
                    h.render();
                });
            }
        }
    }
}

/// 记词组失败时给用户看的一句话。
fn phrase_error_message(error: &PhraseError) -> String {
    match error {
        PhraseError::EmptyPinyin => "拼音还是空的：敲出这段文字的拼音再回车".to_owned(),
        PhraseError::Incomplete => "拼音里有没打完的音节：补全再回车，或按 Esc 放弃".to_owned(),
        PhraseError::BadPinyin(_) => "拼音认不出来：只收小写字母，音节之间可以用 '".to_owned(),
        PhraseError::EmptyText => "没有要记的文字".to_owned(),
        PhraseError::Private => "私密输入中，不记词组".to_owned(),
        PhraseError::LearningOff => "学习关着（[general] learning），没记词组".to_owned(),
    }
}

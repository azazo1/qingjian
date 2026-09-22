//! 「记词组」：快捷键 → 请 DLL 读选区 → 反查读音预填拼音 → 候选窗显示拼音与提示，
//! 回车把「拼音 → 文本」记成用户词，Esc 放弃。与 macOS 壳对齐；引擎调用在 Core。
//!
//! 应用里的选区一个字符都不动：评审期间按键只改这里那份拼音字符串，回给 DLL 的 `commit` 恒空。
//! 进行态在 [`Phrase`]，提交成功后的那一句话在 [`Router::phrase_notice`]。

mod job;

use qingjian_core::{Candidate, CandidateKind, CandidateList, PhraseError};
use qingjian_platform::protocol::{
    Frame, KeyEvent, KeyModifiers, KeyOutcome, PreeditKind, PreeditSegment, ScreenRect,
    ServerMessage, SessionId,
};

pub(super) use self::job::Phrase;
use super::Router;
use super::key::{BACK, ESCAPE, RETURN};

impl Router {
    /// 修饰键比物理组合（去掉 Caps / 中英模式两个状态位）；配置里配成 `none` 时这个键不存在，恒不命中。
    pub(super) fn matches_learn_combo(&self, event: &KeyEvent) -> bool {
        let Some(combo) = self.config.learn_phrase else {
            return false;
        };
        event.character == Some(combo.key)
            && event.modifiers.chord() == KeyModifiers::from(combo.modifiers)
    }

    /// DLL 回来的选区（用途是记词组）：反查读音预填，进评审态。
    pub(super) fn begin_phrase_review(&mut self, text: &str, rect: ScreenRect) {
        self.last_rect = Some(rect);
        self.cancel_prediction();
        // 反查表第一次用时要建（遍历全部词库），建完留在引擎里，之后按快捷键不再付这份代价
        let pinyin = self.engine.pinyin_of(text).unwrap_or_default();
        tracing::debug!(chars = text.chars().count(), "记词组：进入评审态");
        self.phrase = Some(Phrase {
            text: text.to_owned(),
            pinyin,
        });
        let frame = self.self_drawn_frame();
        self.reconcile_candidates(&frame);
    }

    /// 评审态按键：字母与 `'` 追加、退格删末位、回车记下、Esc 放弃，其余键吞掉不动。
    /// 带 Ctrl / Alt / Win 的组合（复制、切换输入法这些）不归评审：放弃这次记词组，键交回应用。
    pub(super) fn handle_phrase_review(
        &mut self,
        session: SessionId,
        event: &KeyEvent,
    ) -> ServerMessage {
        if event.modifiers.has_command_key() {
            tracing::debug!("记词组：带命令修饰键的键，放弃并交回应用");
            self.end_phrase();
            return ServerMessage::KeyResult {
                session,
                outcome: KeyOutcome::Passthrough,
                commit: None,
                frame: Frame::default(),
            };
        }
        match event.virtual_key {
            RETURN => self.commit_phrase(),
            ESCAPE => {
                tracing::debug!("记词组：放弃");
                self.end_phrase();
            }
            BACK => {
                if let Some(phrase) = self.phrase.as_mut() {
                    phrase.pinyin.pop();
                }
            }
            _ => {
                let typed = event
                    .character
                    .filter(|c| c.is_ascii_alphabetic() || *c == '\'');
                if let Some(c) = typed
                    && let Some(phrase) = self.phrase.as_mut()
                {
                    phrase.pinyin.push(c.to_ascii_lowercase());
                }
            }
        }
        // 提交成功时评审态已经换成提示态，这一帧按当前状态生成
        let shown = self.self_drawn_frame();
        self.reconcile_candidates(&shown);
        ServerMessage::KeyResult {
            session,
            outcome: KeyOutcome::Consumed,
            commit: None,
            frame: self.current_frame(),
        }
    }

    /// 回车：把当前的「拼音 → 文本」记成用户词。记下了就收窗留一句话，
    /// 没记成（拼音还没打完、学习关着等）就留在评审态里，提示随这一帧显示。
    fn commit_phrase(&mut self) {
        // 先取出评审态那两份字符串：`remember_phrase` 要可变借引擎，与借 `self.phrase` 冲突
        let Some((pinyin, text)) = self
            .phrase
            .as_ref()
            .map(|phrase| (phrase.pinyin.clone(), phrase.text.clone()))
        else {
            return;
        };
        match self.engine.remember_phrase(&pinyin, &text) {
            Ok(remembered) => {
                let message = match remembered.replaced {
                    Some(_) => format!("已更新「{}」的拼音", remembered.text),
                    None => format!("已记住词组「{}」", remembered.text),
                };
                tracing::info!(text = %remembered.text, "记下用户词组");
                self.phrase = None;
                self.phrase_notice = Some(message);
                self.flush_learning();
            }
            Err(error) => {
                tracing::debug!(%error, "没记成词组");
                self.notice = Some(phrase_error_message(&error));
            }
        }
    }

    /// 结束记词组：丢掉进行态并收窗。没在评审时是空操作。
    pub(super) fn end_phrase(&mut self) {
        if self.phrase.take().is_some() {
            self.hide_candidate_window();
        }
    }

    /// 评审帧：拼音行 + 一句提示，没有候选、没有分页。拼音还空着时放一段空格，窗口才不至于收起。
    pub(super) fn phrase_frame(&self, phrase: &Phrase) -> Frame {
        let display = if phrase.pinyin.is_empty() {
            " ".to_owned()
        } else {
            phrase.pinyin.clone()
        };
        Frame {
            preedit: vec![PreeditSegment {
                text: display,
                kind: PreeditKind::Typed,
            }],
            preedit_mode: self.config.preedit,
            cursor: phrase.pinyin.chars().count(),
            candidates: CandidateList { items: Vec::new() },
            highlight: usize::MAX,
            page: 0,
            page_count: 1,
            layout: self.config.layout,
            theme: self.config.theme,
            aux_code_show: self.config.aux_code_show,
            sentence: None,
            // 上一次提交失败时的那句话优先（`notice` 随这一次帧下发，下一次按键清）
            notice: self
                .notice
                .clone()
                .or_else(|| Some("回车记词组 · Esc 取消".to_owned())),
            reviewing: true,
        }
    }

    /// 记下之后的那一帧：一条提示候选，候选窗显示一句，下一次按键就清。
    pub(super) fn phrase_notice_frame(&self, message: &str) -> Frame {
        Frame {
            preedit: Vec::new(),
            preedit_mode: self.config.preedit,
            cursor: 0,
            candidates: CandidateList {
                items: vec![Candidate {
                    text: message.to_owned(),
                    kind: CandidateKind::Cloud,
                    syllables: Vec::new(),
                    reading: None,
                    translation: None,
                    aux_code: None,
                }],
            },
            highlight: 0,
            page: 0,
            page_count: 1,
            layout: self.config.layout,
            theme: self.config.theme,
            aux_code_show: self.config.aux_code_show,
            sentence: None,
            notice: None,
            reviewing: false,
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

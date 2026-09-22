//! 「翻译选中文字」：快捷键 → 请 DLL 读选区 → 云端翻译 → 候选窗显示译文，回车 / 空格替换、Esc 保留。
//! 与 macOS 壳对齐；引擎调用在 Core。替换选区靠回给 DLL 的 `commit`：无组句时 `InsertTextAtSelection` 正好替换选区。
//! 进行态在 [`Translation`]。

mod job;

use qingjian_core::{Candidate, CandidateKind, CandidateList};
use qingjian_platform::protocol::{
    Frame, KeyEvent, KeyModifiers, KeyOutcome, ScreenRect, ServerMessage, SessionId,
};

pub(super) use self::job::Translation;
use super::Router;
use super::key::{ESCAPE, RETURN};
use super::selection::SelectionPurpose;

impl Router {
    /// 修饰键比物理组合（去掉 Caps / 中英模式两个状态位）; 配置里配成 `none` 时这个键不存在, 恒不命中.
    pub(super) fn matches_translate_combo(&self, event: &KeyEvent) -> bool {
        let Some(combo) = self.config.translate_selection else {
            return false;
        };
        event.character == Some(combo.key)
            && event.modifiers.chord() == KeyModifiers::from(combo.modifiers)
    }

    /// DLL 回来的选区：按请求的用途分流（翻译 / 记词组）。请求号对不上、选区为空的回空帧，
    /// 让 DLL 清掉本地的评审态。回给 DLL 的帧恒空（候选窗在 Server 自绘）。
    pub(super) fn handle_selection(
        &mut self,
        session: SessionId,
        request: u64,
        text: String,
        rect: ScreenRect,
    ) -> ServerMessage {
        let empty = ServerMessage::KeyResult {
            session,
            outcome: KeyOutcome::Consumed,
            commit: None,
            frame: Frame::default(),
        };
        if self.focused != Some(session) {
            return empty;
        }
        let Some(pending) = self.pending_selection.take() else {
            return empty;
        };
        if pending.request != request {
            // 不是这次请的选区：留着原来那个请求，等它自己的回包
            self.pending_selection = Some(pending);
            return empty;
        }
        let text = text.trim();
        if text.is_empty() {
            tracing::info!("读选区：没有可读的选区");
            return empty;
        }
        match pending.purpose {
            SelectionPurpose::Translate => self.begin_translation_review(text, rect),
            SelectionPurpose::LearnPhrase => self.begin_phrase_review(text, rect),
        }
        // 回给 DLL 的帧只带「评审中不在中」这个标志（`current_frame` 会把评审帧裁成空帧）：
        // 评审期间 DLL 得继续把键送来，而拼音与提示画在 Server 自绘的候选窗里。
        ServerMessage::KeyResult {
            session,
            outcome: KeyOutcome::Consumed,
            commit: None,
            frame: self.current_frame(),
        }
    }

    /// 选区回来（用途是翻译）：云服务开着才进评审，否则什么都不做，DLL 那边清掉翻译态。
    fn begin_translation_review(&mut self, text: &str, rect: ScreenRect) {
        if !self.engine.prediction_enabled() {
            tracing::info!("翻译选中文字：云服务已关");
            return;
        }
        self.last_rect = Some(rect);
        self.translation = Some(Translation { result: None });
        self.engine.request_translation(text);
        tracing::debug!(chars = text.chars().count(), "翻译选中文字：已发翻译请求");
        let frame = self.self_drawn_frame();
        self.reconcile_candidates(&frame);
    }

    /// 评审态收到按键：任何键都结束评审。回车 / 空格 / 1 接受（译文没到就只吃键），Esc 放弃，其余放弃并交回应用。
    pub(super) fn handle_translation_review(
        &mut self,
        session: SessionId,
        event: &KeyEvent,
    ) -> ServerMessage {
        let accept =
            event.virtual_key == RETURN || matches!(event.character, Some(' ') | Some('1'));
        let escape = event.virtual_key == ESCAPE;
        let result = self
            .translation
            .as_ref()
            .and_then(|translation| translation.result.clone());
        self.end_translation();
        let (commit, outcome) = if accept {
            (result, KeyOutcome::Consumed)
        } else if escape {
            (None, KeyOutcome::Consumed)
        } else {
            (None, KeyOutcome::Passthrough)
        };
        ServerMessage::KeyResult {
            session,
            outcome,
            commit,
            frame: Frame::default(),
        }
    }

    /// 结束评审：丢进行态、取消在飞的请求、收起候选窗。没在评审时是空操作。
    pub(super) fn end_translation(&mut self) {
        if self.translation.take().is_some() {
            self.cancel_prediction();
            self.hide_candidate_window();
        }
    }

    /// 评审帧：单条候选（译文或「翻译中…」），无 preedit、无分页。
    pub(super) fn translation_frame(&self, translation: &Translation) -> Frame {
        let text = translation
            .result
            .clone()
            .unwrap_or_else(|| "翻译中…".to_owned());
        Frame {
            preedit: Vec::new(),
            preedit_mode: self.config.preedit,
            cursor: 0,
            candidates: CandidateList {
                items: vec![translate_candidate(text)],
            },
            highlight: 0,
            page: 0,
            page_count: 1,
            layout: self.config.layout,
            theme: self.config.theme,
            aux_code_show: self.config.aux_code_show,
            sentence: None,
            notice: None,
            reviewing: true,
        }
    }
}

/// 译文包成一条云端样式的候选（带云朵标记）。
fn translate_candidate(text: String) -> Candidate {
    Candidate {
        text,
        kind: CandidateKind::Cloud,
        syllables: Vec::new(),
        reading: None,
        translation: None,
        aux_code: None,
    }
}

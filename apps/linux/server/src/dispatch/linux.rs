//! Linux 事件决策；Fcitx 插件只报告事实并应用返回值。
use super::{Router, key::Effect};
use crate::protocol::{DisplayIdentity, LinuxEvent, LinuxRequest};
use qingjian_platform::protocol::{KeyOutcome, ServerMessage, SessionId};

impl Router {
    pub(super) fn linux_event(&mut self, request: LinuxRequest) -> Option<ServerMessage> {
        let session = request.session;
        if !self.sessions.contains_key(&session) {
            return None;
        }
        let mut outcome = KeyOutcome::Passthrough;
        let mut commit = None;
        match request.event {
            LinuxEvent::Reset => self.discard_session(session),
            LinuxEvent::Capabilities(caps) => {
                let disabled = caps.password || caps.disabled;
                let private = disabled || caps.sensitive;
                if self.sessions[&session].capabilities != Some(caps) {
                    // 即使 Sensitive → Password 仍为 private，也必须丢弃全部旧输入。
                    self.discard_session(session);
                    self.set_privacy(session, private);
                    let info = self.sessions.get_mut(&session)?;
                    info.capabilities = Some(caps);
                    info.disabled = disabled;
                }
            }
            LinuxEvent::Focus { focused } => {
                let info = self.sessions.get_mut(&session)?;
                info.active = focused;
                info.shift_pending = false;
                info.display_frame = None;
                if !focused && self.focused == Some(session) {
                    self.engine.note_displayed(std::iter::empty());
                }
                if focused {
                    self.ensure_focus(session);
                }
            }
            LinuxEvent::Deactivate {
                focus_out,
                client_preedit,
                capability_changed,
            } => {
                if capability_changed || self.sessions[&session].disabled {
                    self.discard_session(session);
                } else {
                    self.ensure_focus(session);
                    // 缓冲区空了但还有没交给应用的已选词时也要交出去
                    let text = (self.engine.has_pending() || !self.engine.composition().is_empty())
                        .then(|| self.engine.take_raw())
                        .filter(|text| !text.is_empty());
                    if !(focus_out && client_preedit) {
                        commit = text;
                    }
                    self.reset_composition();
                }
                let info = self.sessions.get_mut(&session)?;
                info.active = false;
                info.shift_pending = false;
                info.display_frame = None;
                self.flush_learning();
            }
            LinuxEvent::Key { mut event, release } => {
                if self.sessions[&session].disabled
                    || self.sessions[&session].capabilities.is_none()
                {
                    return Some(ServerMessage::KeyResult {
                        session,
                        outcome,
                        commit,
                        frame: Default::default(),
                    });
                }
                self.ensure_focus(session);
                self.notice = None;
                let info = self.sessions.get_mut(&session)?;
                let shift = event.virtual_key == 0x10;
                if release {
                    if shift && info.shift_pending {
                        info.shift_pending = false;
                        info.english = !info.english;
                        commit = (self.engine.has_pending()
                            || !self.engine.composition().is_empty())
                        .then(|| self.engine.take_raw())
                        .filter(|text| !text.is_empty());
                        self.reset_composition();
                        outcome = KeyOutcome::Consumed;
                    }
                    // 调频修饰键抬起：收起频次预览（键本身照旧归应用，outcome 仍是 Passthrough）
                    event.release = true;
                    self.apply_adjust_modifier(&event);
                } else {
                    info.shift_pending = shift && !event.modifiers.has_command_key();
                    event.modifiers.english_mode = info.english;
                    let effect = if shift {
                        Effect::Passthrough
                    } else {
                        self.apply_key(&event)
                    };
                    match effect {
                        Effect::Changed(text) => {
                            self.recompose();
                            // 空串（这段拼音还没选完，选中的词留在 Engine 里）不当上屏文本发
                            commit = text.filter(|text| !text.is_empty());
                            outcome = KeyOutcome::Consumed;
                        }
                        Effect::Navigated => outcome = KeyOutcome::Consumed,
                        // 只更新了预览状态（调频键的按下 / 抬起）：键归应用，但不碰组句 —— 按一下 Ctrl 不该把已选词上屏
                        Effect::Noted => {}
                        // 这一键归应用：文本流越过了这段组句，还没交给应用的已选词先交出去
                        Effect::Passthrough => {
                            let pending = self.engine.take_pending();
                            if !pending.is_empty() {
                                commit = Some(pending);
                            }
                        }
                    }
                }
            }
            LinuxEvent::Candidate { identity, index } => {
                if self.valid_panel_event(session, &identity) && index < self.config.page_size {
                    let offset = self.highlight / self.config.page_size * self.config.page_size;
                    commit = self.commit_index(offset + index);
                    if commit.is_some() {
                        self.recompose();
                        outcome = KeyOutcome::Consumed;
                    }
                }
            }
            LinuxEvent::Page { identity, next } => {
                if self.valid_panel_event(session, &identity) {
                    self.page(if next { 1 } else { -1 });
                    outcome = KeyOutcome::Consumed;
                }
            }
        }
        // 组句已经结束（删空拼音、Esc 之外的清空路径）却还有没交给应用的已选词：补上屏
        let commit = self.settle_pending(commit);
        let frame = if self.focused == Some(session) && !self.sessions[&session].disabled {
            self.current_frame()
        } else {
            Default::default()
        };
        Some(ServerMessage::KeyResult {
            session,
            outcome,
            commit,
            frame,
        })
    }

    /// 组句已经结束却还有没交给应用的已选词时，把它们接在本次上屏文本前面一起交出去。
    /// 组句还在时不动（词留在 Engine 里等组句结束，退格能拆回）。
    pub(super) fn settle_pending(&mut self, commit: Option<String>) -> Option<String> {
        if !self.engine.composition().is_empty() {
            return commit;
        }
        let pending = self.engine.take_pending();
        match (pending.is_empty(), commit) {
            (true, commit) => commit,
            (false, None) => Some(pending),
            (false, Some(commit)) => Some(format!("{pending}{commit}")),
        }
    }

    pub(super) fn valid_panel_event(&self, session: SessionId, identity: &DisplayIdentity) -> bool {
        let Some(info) = self.sessions.get(&session) else {
            return false;
        };
        self.focused == Some(session)
            && info.active
            && !info.disabled
            && info.display_identity.as_ref() == Some(identity)
    }

    fn discard_session(&mut self, session: SessionId) {
        let info = self.sessions.get_mut(&session).expect("known session");
        info.shift_pending = false;
        info.display_frame = None;
        info.composed = None;
        info.highlight = 0;
        info.navigated = false;
        if let Some(identity) = &mut info.display_identity {
            self.display_revision += 1;
            identity.revision = self.display_revision;
        }
        if self.focused == Some(session) {
            self.engine.discard_input();
            self.composed = None;
            self.sentence = None;
            self.notice = None;
            self.highlight = 0;
            self.navigated = false;
        } else {
            info.engine.discard_input();
        }
    }
}

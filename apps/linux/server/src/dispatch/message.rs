//! Linux 首版协议消息与 Engine 按键分派。
use super::{Router, key::Effect, session::SessionInfo};
use qingjian_platform::protocol::{ClientMessage, KeyOutcome, PROTOCOL_VERSION, ServerMessage};

impl Router {
    pub(super) fn dispatch(&mut self, message: ClientMessage) -> Option<ServerMessage> {
        match message {
            ClientMessage::OpenSession {
                session,
                app,
                protocol,
            } => {
                if protocol != PROTOCOL_VERSION {
                    return None;
                }
                self.close_session(session);
                self.sessions.insert(session, SessionInfo::new(app));
                None
            }
            ClientMessage::Privacy { session, private } => {
                self.set_privacy(session, private);
                None
            }
            ClientMessage::Key { session, event } if self.sessions.contains_key(&session) => {
                self.ensure_focus(session);
                self.notice = None;
                let (commit, outcome) = match self.apply_key(&event) {
                    Effect::Changed(commit) => {
                        self.recompose();
                        // 空串（这段拼音还没选完，选中的词留在 Engine 里）不当上屏文本发
                        (commit.filter(|text| !text.is_empty()), KeyOutcome::Consumed)
                    }
                    Effect::Navigated => (None, KeyOutcome::Consumed),
                    // 这一键归应用：文本流越过了这段组句，还没交给应用的已选词先交出去
                    Effect::Passthrough => {
                        let pending = self.engine.take_pending();
                        (
                            (!pending.is_empty()).then_some(pending),
                            KeyOutcome::Passthrough,
                        )
                    }
                };
                // 组句已经结束（删空拼音、Esc 之外的清空路径）却还有没交给应用的已选词：补上屏
                let commit = self.settle_pending(commit);
                let frame = self.current_frame();
                Some(ServerMessage::KeyResult {
                    session,
                    outcome,
                    commit,
                    frame,
                })
            }
            ClientMessage::Poll { session } if self.sessions.contains_key(&session) => {
                self.ensure_focus(session);
                Some(ServerMessage::Update {
                    session,
                    frame: self.current_frame(),
                })
            }
            ClientMessage::Commit { session } if self.sessions.contains_key(&session) => {
                self.ensure_focus(session);
                // 缓冲区空了但还有没交给应用的已选词时也要交出去
                let text = (self.engine.has_pending() || !self.engine.composition().is_empty())
                    .then(|| self.engine.take_raw())
                    .filter(|text| !text.is_empty());
                self.reset_composition();
                self.flush_learning();
                Some(ServerMessage::Committed { session, text })
            }
            ClientMessage::CloseSession { session } => {
                self.close_session(session);
                None
            }
            _ => None,
        }
    }
}

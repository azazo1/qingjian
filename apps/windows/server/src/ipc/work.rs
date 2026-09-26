use std::sync::mpsc::Sender;

use qingjian_platform::protocol::{ClientMessage, ServerMessage};

use crate::dispatch::{LearnPhraseReply, LearnPhraseWork, StatusEvent};

/// 工人线程（独占 [`crate::dispatch::Router`]）的一件活：DLL 的一条消息，或状态条上的一次操作。
pub enum Work {
    /// 某条连接收到的消息 + 回结果的通道（`None` 表示不用回话）。
    Client(ClientMessage, Sender<Option<ServerMessage>>),

    /// UI 线程发来的状态条操作。
    Status(StatusEvent),

    /// 「录入词组」窗口发来的活：`reply` 写回结果，`wake` 是 UI 线程 id，答完唤醒它去读。
    LearnPhrase {
        work: LearnPhraseWork,
        reply: Sender<LearnPhraseReply>,
        wake: u32,
    },
}

use super::SelectionPurpose;

/// 已经请 DLL 读选区、还没等到回包的请求。
pub(super) struct PendingSelection {
    /// 请求标识，随 `ClientMessage::Selection` 回来；对不上的丢弃。
    pub(super) request: u64,

    /// 这次读选区是给谁用的。
    pub(super) purpose: SelectionPurpose,
}

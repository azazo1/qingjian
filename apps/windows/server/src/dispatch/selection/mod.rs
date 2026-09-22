//! 等 DLL 回选区的请求：请求号与用途。

mod pending;
mod purpose;

pub(super) use pending::PendingSelection;
pub(super) use purpose::SelectionPurpose;

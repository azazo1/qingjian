//! 一次按键对组句的影响。

/// 一次按键对组句的影响。
pub(crate) enum Effect {
    /// 缓冲变了，要重建 [`Composed`](super::super::Composed)；带本次要上屏的文本。
    Changed(Option<String>),

    /// 只挪了高亮 / 翻页。
    Navigated,

    /// 只更新了 Server 侧的状态（调频键按住时的频次预览），键本身不吃、也不碰组句：回 `Passthrough` 放行，
    /// 但不像 [`Self::Passthrough`] 那样把还没上屏的已选词交出去（按一下 Ctrl 不该上屏任何东西）。
    Noted,

    /// 不吃，交还应用。
    Passthrough,
}

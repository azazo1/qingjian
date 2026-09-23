//! 候选词频次的读与手动调整：调频键按住时的预览（[`WordFrequency`]）与 K / J 的升降结果（[`FrequencyChange`]）。

use serde::{Deserialize, Serialize};

use crate::candidate::CandidateKind;

/// 一个候选的学习计数，调频预览显示的就是这两个数。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct WordFrequency {
    /// 这个词被用户选过的总次数（进排序得分里的 [`weight_bonus`](crate::ranking::weight_bonus)，对数且封顶）。
    pub total: u32,

    /// 当前输入串下选过它的次数（排序键里比上下文得分更靠前的那一项）。
    pub selected: u32,
}

/// 一次手动调频之后的状态（调频键 + K / J）。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FrequencyChange {
    /// 调整之后该候选的两个计数。
    pub frequency: WordFrequency,

    /// 这次真的改了计数。降到底、以及没有词频可调的候选都是 `false`。
    pub changed: bool,
}

/// 这个候选有没有可以调的词频：中文词（含辅码、云端）与英文词有，整句 / 快捷 / emoji / 自定义规则没有
/// 「被选过几次」这回事。
pub(super) fn adjustable(kind: CandidateKind) -> bool {
    matches!(
        kind,
        CandidateKind::Chinese
            | CandidateKind::Code
            | CandidateKind::Cloud
            | CandidateKind::English
    )
}

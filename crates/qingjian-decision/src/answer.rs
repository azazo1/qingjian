//! 决策模型给的答案：选择题的选项概率。两个后端都按选项文本索引，取回来就能对上候选。

use std::collections::BTreeMap;

/// 一条选择题的答案。
#[derive(Debug, Clone, PartialEq)]
pub struct ChoiceAnswer {
    /// 模型挑中的那条（后端给的 argmax，青简不直接用，留着写日志）。
    pub picked: String,

    /// 选项文本 → 概率。两个后端都按选项文本索引，所以能直接按候选句子取回来。
    pub probabilities: BTreeMap<String, f64>,
}

impl ChoiceAnswer {
    /// 某个选项的概率；后端没给这一项就 `None`。
    pub fn probability(&self, option: &str) -> Option<f64> {
        self.probabilities.get(option).copied()
    }
}

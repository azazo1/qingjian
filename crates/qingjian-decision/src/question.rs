//! 要问决策模型的问题。现在只用得上选择题。

/// 一个选择题（choice）：从若干选项里挑一条。
///
/// 决策模型（Laya / Jev）的题型有 choice / score / noul 三种，青简现在只用得上 choice——
/// 整句重排问的就是「这几条候选里哪条最顺」。要接别的题型时，在这里多一个类型即可，后端按类型序列化。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Question {
    /// 问题编号；答案按它对应回来。
    pub id: String,

    /// 问句本身（模型看到的那句话）。
    pub instructions: String,

    /// 选项，顺序即下标；答案里的概率按选项文本对应。
    pub options: Vec<String>,
}

impl Question {
    /// 按选项文本造一个选择题。
    pub fn choice(id: impl Into<String>, instructions: impl Into<String>, options: Vec<String>) -> Self {
        Self {
            id: id.into(),
            instructions: instructions.into(),
            options,
        }
    }
}

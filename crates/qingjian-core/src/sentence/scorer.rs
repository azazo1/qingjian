use super::ScoreForm;

/// 整句路径的第二打分来源: 给 "前文 + 整句文本" 打分的模型. 字级 Transformer (`qingjian-neural`,
/// 给整句 log 概率) 与判断类决策模型 (`qingjian-decision`, 给同一批候选之间的相对优劣) 都接在这里.
/// Core 只认这个 trait; Viterbi 出的前几条路径用它重打分, 与路径本身的得分插值.
pub trait SentenceScorer: Send {
    /// 每条 `texts` 接在 `context` (光标前文, 可空) 后面的分, 与 `texts` 一一对应, 越大越好.
    /// 算不了 (模型出错) 返回空 Vec, 调用方就当没有这个打分.
    fn score(&self, context: &str, texts: &[&str]) -> Vec<f64>;

    /// 分的量纲 (见 [`ScoreForm`]): 缺省当整句 log 概率 (`log P(text | context)`).
    fn form(&self) -> ScoreForm {
        ScoreForm::Absolute
    }

    /// 打分会不会把内容发到本机之外 (云端接口). 缺省不会 (本地模型); 私密输入期间这类打分器
    /// 一律不参与重排, 前文一个字都不出去.
    fn is_remote(&self) -> bool {
        false
    }

    /// 相对分的满量程: `form()` 为 [`ScoreForm::Relative`] 时, 分 = 置信度 x 这个值, Core 据此还原出
    /// 每条候选 0 到 1 的置信度给壳显示 (候选旁的模型徽标). 缺省 `None`: 没有置信度这个概念 ——
    /// 字级模型的整句 log 概率只有相对大小, 不是概率.
    fn relative_scale(&self) -> Option<f64> {
        None
    }
}

//! 把决策模型接成整句重打分的第二来源（Core `sentence::SentenceScorer` 的实现）。

use qingjian_core::sentence::{ScoreForm, SentenceScorer};

use crate::backend::{self, DecisionBackend};
use crate::config::DecisionConfig;
use crate::error::DecisionError;
use crate::question::Question;

/// 一次重排只问一个问题，编号固定（响应里按它取答案）。
const QUESTION_ID: &str = "natural";

/// 问句：整句重排要的就是「这几条里哪条最顺」。
const INSTRUCTIONS: &str =
    "下面几个选项是接在同一段前文之后的候选句子，哪一句读起来最自然、最像人会写下来的话？";

/// 决策模型接成整句重打分的第二来源。
///
/// 一次 [`SentenceScorer::score`] 把所有候选放进同一个选择题：决策模型一次前向就给出各候选的概率分布，
/// 概率高的那条说明模型觉得它更顺。它给的是同一批候选之间的相对优劣，所以量纲是
/// [`ScoreForm::Relative`]——重排时只按批内均值居中后叠加，不去顶掉路径分里的静态语言模型。
pub struct DecisionScorer {
    /// 后端。
    backend: Box<dyn DecisionBackend>,

    /// 给模型看的前文最多几个字符。
    context_chars: usize,

    /// 概率到 nat 的换算跨度。
    span: f64,

    /// 内容会不会离开本机（云端后端）。Core 私密输入期间按它决定要不要让这个打分器干活。
    remote: bool,
}

impl DecisionScorer {
    /// 按配置打开后端；后端起不来（没密钥、地址不合法、起不了运行时）返回错误，壳据此退回不重排。
    pub fn new(config: &DecisionConfig) -> Result<Self, DecisionError> {
        let backend = backend::open(config)?;
        tracing::info!(
            backend = config.backend.key(),
            endpoint = config.endpoint(),
            timeout_ms = config.timeout_ms,
            span = config.span,
            "决策模型接上整句重排"
        );
        Ok(Self {
            backend,
            context_chars: config.context_chars,
            span: config.span.max(0.0),
            remote: config.backend.leaves_the_machine(),
        })
    }
}

impl SentenceScorer for DecisionScorer {
    fn score(&self, context: &str, texts: &[&str]) -> Vec<f64> {
        if texts.is_empty() {
            return Vec::new();
        }
        let question = Question::choice(
            QUESTION_ID,
            INSTRUCTIONS,
            texts.iter().map(|text| (*text).to_owned()).collect(),
        );
        let state = take_last_chars(context, self.context_chars);
        let answer = match self.backend.decide(&state, std::slice::from_ref(&question)) {
            Ok(answers) => answers.into_iter().next(),
            Err(error) => {
                tracing::warn!(%error, "决策模型没给答案，本轮不重排");
                None
            }
        };
        let Some(answer) = answer else {
            return Vec::new();
        };
        tracing::debug!(
            candidates = texts.len(),
            picked = %answer.picked,
            "决策模型重排"
        );
        // 概率缺失的候选按均分算：后端没给这一项，说明它没被算进来，不该因此被抬高
        let uniform = 1.0 / texts.len() as f64;
        texts
            .iter()
            .map(|text| answer.probability(text).unwrap_or(uniform) * self.span)
            .collect()
    }

    fn form(&self) -> ScoreForm {
        ScoreForm::Relative
    }

    fn is_remote(&self) -> bool {
        self.remote
    }
}

/// 末尾 `count` 个字符；`count` 为 0 时给空串（后端按「没有前文」处理）。
fn take_last_chars(text: &str, count: usize) -> String {
    if count == 0 {
        return String::new();
    }
    let total = text.chars().count();
    text.chars().skip(total.saturating_sub(count)).collect()
}

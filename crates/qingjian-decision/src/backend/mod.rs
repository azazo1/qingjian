//! 决策模型的两个后端：本地服务（laya）与云端接口（jev）。两者都只走 HTTP，形状差异在这一层抹平。

mod http;
mod jev;
mod kind;
mod laya;
mod response;

pub use jev::JevBackend;
pub use kind::BackendKind;
pub use laya::LayaBackend;

use crate::answer::ChoiceAnswer;
use crate::config::DecisionConfig;
use crate::error::DecisionError;
use crate::question::Question;

/// 一个决策模型后端：把 state 与一批选择题发出去，拿回答案。
///
/// 接口是同步的：它只被整句重排的后台线程调用，那里没有异步上下文，运行时在 `http` 里自己管。
pub trait DecisionBackend: Send {
    /// 与 `questions` 一一对应的答案，顺序一致。失败返回错误，调用方这一轮不重排。
    fn decide(&self, state: &str, questions: &[Question])
    -> Result<Vec<ChoiceAnswer>, DecisionError>;
}

/// 按配置打开后端。
pub fn open(config: &DecisionConfig) -> Result<Box<dyn DecisionBackend>, DecisionError> {
    match config.backend {
        BackendKind::Laya => Ok(Box::new(LayaBackend::new(config)?)),
        BackendKind::Jev => Ok(Box::new(JevBackend::new(config)?)),
    }
}

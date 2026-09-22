//! 决策模型接入：把 jev（TypeSafe System One 云端接口）与 laya（本地 HTTP 服务）这类 typed decision 模型
//! 接成整句重排的第二打分来源。
//!
//! 这类模型不做生成：给一段 state 与若干「选择题」，一次前向直接返回各选项上的概率分布，正好对上
//! 「这几条整句候选，哪条接在前文之后最顺」。它占的是 Core 里 `SentenceScorer` 那个位置（与字级语言模型
//! `qingjian-neural` 同一个），区别只有一点：它给的是同一批候选之间的相对优劣
//! （`qingjian_core::sentence::ScoreForm::Relative`），不是整句的 log 概率。
//!
//! 两个后端都只走 HTTP：laya 打本地服务（`laya-serve`，或自己包的 `laya-mlx`），jev 打 TypeSafe 的
//! System One 接口。青简不内嵌它们的推理栈——模型怎么跑、跑在哪台机器上、用哪个 checkpoint，都由用户自己定。

mod answer;
mod backend;
mod config;
mod error;
mod question;
mod scorer;

pub use answer::ChoiceAnswer;
pub use backend::{BackendKind, DecisionBackend};
pub use config::DecisionConfig;
pub use error::DecisionError;
pub use question::Question;
pub use scorer::DecisionScorer;

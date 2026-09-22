//! 本地 Laya 服务后端 (`laya-serve` 的 `POST /api/predict`).

use serde_json::json;

use super::http::Http;
use super::response::parse_answers;
use super::DecisionBackend;
use crate::answer::ChoiceAnswer;
use crate::config::DecisionConfig;
use crate::error::DecisionError;
use crate::question::Question;

/// 本地 Laya 后端: `laya-serve` 的 `POST /api/predict`.
///
/// 请求体是 `{"state": …, "questions": [{"id", "type", "instructions", "criteria"}]}`, `criteria` 是选项数组,
/// 响应里的 `probabilities` 按选项文本索引. 自己用 `laya-mlx` 包服务时照这个形状实现即可.
pub struct LayaBackend {
    http: Http,
    endpoint: String,
}

impl LayaBackend {
    pub fn new(config: &DecisionConfig) -> Result<Self, DecisionError> {
        Ok(Self {
            http: Http::new(config.timeout_ms)?,
            endpoint: config.endpoint().to_owned(),
        })
    }
}

impl DecisionBackend for LayaBackend {
    fn decide(
        &self,
        state: &str,
        questions: &[Question],
    ) -> Result<Vec<ChoiceAnswer>, DecisionError> {
        let wire: Vec<serde_json::Value> = questions
            .iter()
            .map(|question| {
                json!({
                    "id": question.id,
                    "type": "choice",
                    "instructions": question.instructions,
                    "criteria": question.options,
                })
            })
            .collect();
        let body = json!({ "state": state, "questions": wire });
        let raw = self.http.post_json(&self.endpoint, &body, None)?;
        parse_answers(&raw, questions)
    }
}

//! TypeSafe System One（Jev）云端接口后端。

use serde_json::{Map, Value, json};

use super::http::Http;
use super::response::parse_answers;
use super::DecisionBackend;
use crate::answer::ChoiceAnswer;
use crate::config::DecisionConfig;
use crate::error::DecisionError;
use crate::question::Question;

/// TypeSafe System One（Jev）后端：`POST https://api.typesafe.ai/v1/systemone`。
///
/// 请求体是 `{"state": …, "model": …, "questions": {"<id>": {"type", "instructions", "criteria"}}}`：
/// 问题按 id 索引（不是数组），`criteria` 是「选项名 → 选项说明」的对象，只要选项文本时说明写 `null`。
/// 鉴权走 `Authorization: Bearer <key>`。
pub struct JevBackend {
    http: Http,
    endpoint: String,
    model: String,
    key: String,
}

impl JevBackend {
    /// 没密钥直接报错，让壳退回不重排并记一条日志。
    pub fn new(config: &DecisionConfig) -> Result<Self, DecisionError> {
        let key = config
            .resolve_api_key()
            .ok_or_else(|| DecisionError::MissingApiKey(config.api_key_env.clone()))?;
        Ok(Self {
            http: Http::new(config.timeout_ms)?,
            endpoint: config.endpoint().to_owned(),
            model: config.model.clone(),
            key,
        })
    }
}

impl DecisionBackend for JevBackend {
    fn decide(
        &self,
        state: &str,
        questions: &[Question],
    ) -> Result<Vec<ChoiceAnswer>, DecisionError> {
        let mut wire = Map::new();
        for question in questions {
            let mut criteria = Map::new();
            for option in &question.options {
                criteria.insert(option.clone(), Value::Null);
            }
            wire.insert(
                question.id.clone(),
                json!({
                    "type": "choice",
                    "instructions": question.instructions,
                    "criteria": criteria,
                }),
            );
        }
        let body = json!({ "state": state, "model": self.model, "questions": wire });
        let raw = self.http.post_json(&self.endpoint, &body, Some(&self.key))?;
        parse_answers(&raw, questions)
    }
}

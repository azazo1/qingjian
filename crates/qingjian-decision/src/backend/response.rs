//! 两个后端共用的响应解析：Laya 与 Jev 的响应形状一致，形状变了只改这里。

use std::collections::BTreeMap;

use serde_json::Value;

use crate::answer::ChoiceAnswer;
use crate::error::DecisionError;
use crate::question::Question;

/// 从后端响应体里取出各问题的选择题答案。
///
/// Laya 与 Jev 的响应形状一致：`answers` 是按问题 id 索引的对象，每条答案带 `choice` 与 `probabilities`，
/// 概率按选项文本索引。两个后端共用这一份解析，形状变了只改这里。
pub(super) fn parse_answers(
    raw: &Value,
    questions: &[Question],
) -> Result<Vec<ChoiceAnswer>, DecisionError> {
    let answers = raw
        .get("answers")
        .and_then(Value::as_object)
        .ok_or_else(|| DecisionError::Response("response has no `answers` object".to_owned()))?;
    let mut out = Vec::with_capacity(questions.len());
    for question in questions {
        let entry = answers
            .get(&question.id)
            .ok_or_else(|| DecisionError::MissingAnswer(question.id.clone()))?;
        let picked = entry
            .get("choice")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned();
        let mut probabilities = BTreeMap::new();
        if let Some(map) = entry.get("probabilities").and_then(Value::as_object) {
            for (option, value) in map {
                if let Some(probability) = value.as_f64() {
                    probabilities.insert(option.clone(), probability);
                }
            }
        }
        out.push(ChoiceAnswer {
            picked,
            probabilities,
        });
    }
    Ok(out)
}

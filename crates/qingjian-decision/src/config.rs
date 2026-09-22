//! 决策模型配置 (`[decision]` 分节).

use serde::{Deserialize, Serialize};

use crate::backend::BackendKind;

/// 决策模型配置 (`[decision]` 分节): 整句重排的第二个来源.
///
/// 开着时它取代本地字级模型 (同一个位置, 两者不会同时生效): 字级模型算整句的 log 概率, 决策模型回答
/// "这几条候选里哪条最顺", 后者不做生成, 一次前向出答案. 默认**关闭**, 因为 jev 会把前文发往云端,
/// laya 也要用户先把本地服务跑起来.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct DecisionConfig {
    /// 是否启用.
    pub enabled: bool,

    /// 后端.
    pub backend: BackendKind,

    /// 接口地址; 留空按后端取缺省 (见 [`BackendKind::default_endpoint`]).
    pub endpoint: String,

    /// 模型名, 只有 jev 用 (laya 的服务端自己知道加载了哪个 checkpoint).
    pub model: String,

    /// 密钥, 只有 jev 用; 留空读 `api_key_env` 指定的环境变量.
    pub api_key: Option<String>,

    /// 存放密钥的环境变量名.
    pub api_key_env: String,

    /// 单次请求超时 (毫秒): 超时这一轮就不重排, 按键那边本来也不等它.
    pub timeout_ms: u64,

    /// 给模型看的光标前文最多几个字符 (0 不给). 决策模型的上下文比输入法需要的长, 给多了只是白花算力.
    pub context_chars: usize,

    /// 决策分到路径分的换算跨度 (nat): 同一批候选里, 模型最偏好的那条相对批内均值最多加这么多分.
    /// 越大越敢翻盘, 太大就压过词库统计了.
    pub span: f64,
}

impl Default for DecisionConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            backend: BackendKind::Laya,
            endpoint: String::new(),
            model: "jev-latest".to_owned(),
            api_key: None,
            api_key_env: "TYPESAFE_API_KEY".to_owned(),
            timeout_ms: 1500,
            context_chars: 48,
            span: 4.0,
        }
    }
}

impl DecisionConfig {
    /// 真正要用的接口地址: 配置里填了就用它, 留空按后端取缺省.
    pub fn endpoint(&self) -> &str {
        let trimmed = self.endpoint.trim();
        if trimmed.is_empty() {
            self.backend.default_endpoint()
        } else {
            trimmed
        }
    }

    /// 配置里的密钥优先, 其次环境变量; 两边都没有返回 `None`.
    pub fn resolve_api_key(&self) -> Option<String> {
        self.api_key
            .as_deref()
            .map(str::trim)
            .filter(|key| !key.is_empty())
            .map(str::to_owned)
            .or_else(|| std::env::var(&self.api_key_env).ok())
            .filter(|key| !key.trim().is_empty())
    }
}

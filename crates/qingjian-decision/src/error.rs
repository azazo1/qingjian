//! 决策模型接入的失败原因：起不来（没密钥、地址不合法、起不了运行时）与问不成（超时、后端拒绝、响应形状不对）。

use thiserror::Error;

/// 决策模型接入的失败原因。
#[derive(Debug, Error)]
pub enum DecisionError {
    /// 配置与环境变量里都没有密钥。
    #[error("no api key: neither `api_key` nor the environment variable {0} is set")]
    MissingApiKey(String),

    /// 接口地址不是合法 URL。
    #[error("bad endpoint {endpoint}: {detail}")]
    Endpoint {
        /// 配置里的地址。
        endpoint: String,
        /// 说不合法在哪。
        detail: String,
    },

    /// 网络层失败：连不上、读不出响应体。
    #[error("request failed: {0}")]
    Transport(#[from] reqwest::Error),

    /// 超时；这一轮不重排。
    #[error("request timed out after {0} ms")]
    Timeout(u64),

    /// 后端拒绝了请求。
    #[error("backend refused the request with {status}: {message}")]
    Status {
        /// HTTP 状态码。
        status: u16,
        /// 响应体里的说明（截断过）。
        message: String,
    },

    /// 响应体不是期望的形状。
    #[error("unexpected response body: {0}")]
    Response(String),

    /// 响应里少了某个问题的答案。
    #[error("backend returned no answer for question {0}")]
    MissingAnswer(String),

    /// 起不了内部运行时。
    #[error("cannot start the request runtime: {0}")]
    Runtime(#[from] std::io::Error),
}

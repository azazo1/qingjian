//! 两个后端共用的 HTTP 通道: 一个只跑请求的运行时, 加一个带超时的客户端.

use std::time::Duration;

use serde_json::Value;

use crate::error::DecisionError;

/// 后端响应体里留下多少字符写进错误信息: 够看清后端说了什么, 又不会把整段正文塞进日志.
const MESSAGE_CHARS: usize = 400;

/// 后端共用的 HTTP 通道: 一个只跑请求的 current_thread 运行时, 加一个带超时的客户端.
///
/// 决策模型只在整句重排的后台线程里被调用, 那里没有异步上下文, 所以运行时放在这里自己 `block_on`.
pub(super) struct Http {
    client: reqwest::Client,
    runtime: tokio::runtime::Runtime,
    timeout: Duration,
}

impl Http {
    pub fn new(timeout_ms: u64) -> Result<Self, DecisionError> {
        let timeout = Duration::from_millis(timeout_ms.max(1));
        let client = reqwest::Client::builder().timeout(timeout).build()?;
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?;
        Ok(Self {
            client,
            runtime,
            timeout,
        })
    }

    /// POST 一个 JSON 体, 返回解析好的 JSON. `bearer` 是要带的密钥 (本地服务不需要).
    pub fn post_json(
        &self,
        endpoint: &str,
        body: &Value,
        bearer: Option<&str>,
    ) -> Result<Value, DecisionError> {
        let url = reqwest::Url::parse(endpoint).map_err(|error| DecisionError::Endpoint {
            endpoint: endpoint.to_owned(),
            detail: error.to_string(),
        })?;
        self.runtime.block_on(async {
            let mut request = self.client.post(url).json(body);
            if let Some(key) = bearer {
                request = request.bearer_auth(key);
            }
            let response = request.send().await.map_err(|error| self.classify(error))?;
            let status = response.status();
            let text = response
                .text()
                .await
                .map_err(|error| self.classify(error))?;
            if !status.is_success() {
                return Err(DecisionError::Status {
                    status: status.as_u16(),
                    message: truncated(&text),
                });
            }
            serde_json::from_str(&text).map_err(|error| {
                DecisionError::Response(format!("{error}; body: {}", truncated(&text)))
            })
        })
    }

    /// 超时单独报, 别的都算传输失败: 日志里两者要能分开看.
    fn classify(&self, error: reqwest::Error) -> DecisionError {
        if error.is_timeout() {
            DecisionError::Timeout(self.timeout.as_millis() as u64)
        } else {
            DecisionError::Transport(error)
        }
    }
}

/// 截断响应体, 避免把整段正文写进日志与错误信息.
fn truncated(text: &str) -> String {
    match text.char_indices().nth(MESSAGE_CHARS) {
        Some((end, _)) => format!("{}…", &text[..end]),
        None => text.to_owned(),
    }
}

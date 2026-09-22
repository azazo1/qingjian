//! 后端的选择: 本地服务还是云端.

use serde::{Deserialize, Serialize};

/// 决策模型后端.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum BackendKind {
    /// 本地 HTTP 服务: `laya-serve` (Rust + candle), 或自己用 `laya-mlx` 包的同类服务. 全程离线.
    #[default]
    Laya,

    /// TypeSafe System One 云端接口 (Jev): 需要密钥, 前文会离开本机.
    Jev,
}

impl BackendKind {
    /// 全部后端, 按设置界面弹出菜单的顺序.
    pub const ALL: [Self; 2] = [Self::Laya, Self::Jev];

    /// 配置文件里的写法, 也是日志里的名字.
    pub fn key(self) -> &'static str {
        match self {
            Self::Laya => "laya",
            Self::Jev => "jev",
        }
    }

    /// 设置界面里的说明.
    pub fn label(self) -> &'static str {
        match self {
            Self::Laya => "本地服务 (laya)",
            Self::Jev => "云端 (jev)",
        }
    }

    /// 配置里留空时用的接口地址.
    pub fn default_endpoint(self) -> &'static str {
        match self {
            Self::Laya => "http://127.0.0.1:8080/api/predict",
            Self::Jev => "https://api.typesafe.ai/v1/systemone",
        }
    }

    /// 是否要密钥: 决定设置界面里密钥框的灰显, 以及拼配置时的报错提示.
    pub fn needs_key(self) -> bool {
        matches!(self, Self::Jev)
    }

    /// 内容会不会离开本机: 云端后端会 (私密输入期间 Core 不让它参与重排), 本地服务不会.
    pub fn leaves_the_machine(self) -> bool {
        matches!(self, Self::Jev)
    }

    /// 配置里的字符串转后端; 认不得的返回 `None` (调用方按缺省处理并记一条日志).
    pub fn from_key(key: &str) -> Option<Self> {
        match key.trim().to_ascii_lowercase().as_str() {
            "laya" => Some(Self::Laya),
            "jev" => Some(Self::Jev),
            _ => None,
        }
    }
}

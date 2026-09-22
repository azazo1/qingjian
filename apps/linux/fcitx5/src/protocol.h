//! 插件与 Server 的线上协议号, 与 Rust 侧的常量成对, 升版本时只改这里.
#pragma once
namespace qingjian {
/// 主协议号 (`OpenSession.protocol`), 对应 `qingjian_platform::protocol::PROTOCOL_VERSION`.
inline constexpr int kProtocolVersion = 7;

/// 展示协议号 (`Update.linux_ui.version` 与 `LinuxHello.version`),
/// 对应 `qingjian-linux-server` 的 `LINUX_UI_PROTOCOL`.
inline constexpr int kUiVersion = 3;
}

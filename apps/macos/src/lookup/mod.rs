//! zi.tools 查字窗口: 输入一个汉字查详情, 输入几个部件查字.
//!
//! 只有 macOS 接了这条线 (配置 `[shortcut] look_up`, 缺省 ⌃⌥L, 与输入法菜单里的「查字…」):
//! 独立窗口, 不跟光标跑, 也不打断正在打的拼音. 请求只在按下「查询」(或输入框里回车) 时发,
//! 平时零网络; Core 不依赖这里, 也永远不联网.

mod fetch;
mod list;
mod panel;
mod target;
mod window;

pub use window::LookupWindow;

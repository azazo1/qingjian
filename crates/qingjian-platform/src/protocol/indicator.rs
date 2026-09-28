use serde::{Deserialize, Serialize};

/// 任务栏「中 / 英」图标右键菜单里要交给 Server 办的项（中 / 英切换在 DLL 侧自己做）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum IndicatorCommand {
    /// 翻转当前模式的全角标点，与悬浮条上点「，。」一样。
    TogglePunctuation,

    /// 显示 / 隐藏悬浮状态条（`[status_bar] enabled`）。
    ToggleStatusBar,

    /// 打开设置程序。DLL 可能在 UWP 沙箱里起不了进程，交给 Server 起。
    OpenSettings,

    /// 查到新版本时菜单里的「有新版本」：打开下载页，同样交给 Server。
    OpenDownload,

    /// 菜单里的「录入词组…」：Server 弹出录入窗口（Engine 只在 Server 进程里，
    /// 反查拼音与写入用户词都得在那做）。DLL 与 Server 同包升级，故这个变体不动
    /// [`super::PROTOCOL_VERSION`]：老 DLL 不发它，新 DLL 配老 Server 才会读不出来。
    LearnPhrase,

    /// 菜单里的「重启输入法」：Server 落盘学习数据后退出，由前台应用的 DLL 按需把它拉回来
    /// （见 `tsf/src/com/service/launch.rs`）。与 [`Self::LearnPhrase`] 同理不动 [`super::PROTOCOL_VERSION`]：
    /// 老 DLL 不认识它，也就不会发；新 DLL 配老 Server 时这一条会解析失败、那条连接断掉，
    /// 而升级安装本身会 `taskkill qingjian-server.exe`（`qingjian.iss` 的 `PrepareToInstall`），
    /// 所以这个窗口很窄。
    Restart,
}

/// 右键菜单打勾用的开关状态。DLL 不读配置文件（UWP 沙箱里读不到），由 Server 随
/// [`super::ServerMessage::ModeSync`] 每一拍带下来。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct IndicatorState {
    /// 中文模式下标点转全角（`[general] full_width_punctuation`）。
    pub full_width_punctuation: bool,

    /// 英文模式下标点转全角（`[general] english_full_width_punctuation`）。
    pub english_full_width_punctuation: bool,

    /// 悬浮状态条开着（`[status_bar] enabled`）。
    pub status_bar: bool,

    /// 检查更新查到了新版本，菜单里露出「有新版本」。
    pub update_available: bool,
}

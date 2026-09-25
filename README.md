# 青简 Qingjian

> 输入的不只是文字。

## 本仓库的改动

这是自用 fork, `upstream` 指向 [qingjian-team/qingjian](https://github.com/qingjian-team/qingjian). 相对上游的改动都记在这里, 便于日后 merge upstream 时对照.

- macOS 输入法菜单「录入词组…」: 填一个词, 拼音按词库自动生成且可改, 确认后效果等同于打这段全拼并选一次该词 (词库没有则记成用户词). 见 `docs/user/settings/preferences.md`.
- CI 在 push main 与手动触发时, 除原有检查外还各打一份未签名的测试包传成 Actions artifact (macOS 的 pkg 两个架构、Windows 的 Inno 安装包), PR 只跑检查不打包. 见 `docs/notes/release.md`.
- 决策模型接入: 新增 `crates/qingjian-decision` 与配置 `[decision]`, 把 jev (云端接口) 与 laya (本地服务) 这类 typed decision 模型接成整句重排的第二个来源 (与 `[model]` 的本地字级模型互斥, 只走 HTTP 不内嵌推理栈); macOS 壳与偏好设置 "云服务" 页已接上, 见 `docs/design/decision-models.md` 与 `docs/user/input/decision-model.md`.
- macOS 的中 / 英切换可配: `[shortcut] mac_switch_single` (单键切换, 键在 `mac_switch_toggle`) 与 `mac_switch_dual` (双键切换, 键在 `mac_switch_english` / `mac_switch_chinese`) 两个开关可同时开, 键可以是带左右的修饰键 (`left-command` / `right-command` 等) 或组合键 (`control+option+z`), 例如左 ⌘ 切英文、右 ⌘ 切中文; `[shortcut] mac_caps_lock_switch` 决定 Caps Lock 是否也切 (`false` 时它只当大小写锁), `[general] english_mode` 关掉后 macOS 也固定中文模式. 实现照 Rime 的 Squirrel: 在 `recognizedEvents:` 里多要一个 flagsChanged, 单击判定与 Windows 的 `KeyTap` 同一套. 见 `docs/user/input/english-mode.md`.
- macOS 切换中 / 英时光标旁闪一下当前模式: 新增 `menubar/badge.rs` (一块浮动小面板, 一秒后自己收, 配置 `[general] mode_badge` 缺省开), 与菜单栏状态项是同一件事的两种显示; 建面板与摆放逻辑从候选窗口抽成 `candidates::window` 的 `build_float_panel` / `place_at_caret` 共用.
- 快捷键可以设成不用: `[shortcut]` 的 `translation` / `translation_second` / `delete_candidate` / `translate_selection` 四项都改走新的 `KeyBinding` 类型 (`config/key_binding.rs`), 值写 `none` 就是这项键不用 (不再占着那个组合, 事件照常交给应用); macOS 偏好设置的快捷键页录制时按 ⌫ 即清空 (按钮显示「未设置」), Windows 设置的快捷键页多一项「不使用」, 两个 Server 的匹配与 TSF 的保留键登记对关掉的项一律不认.
- 偏好设置的「云服务」页多一项「推理强度」文本框 (配置 `[predict] reasoning_effort`), 直接对应请求里的同名字段, 换服务商时不必再手改 TOML: 接口回 400 说这个参数只认哪几个值 (例如只认 `low` / `medium` / `high` / `xhigh` / `max`, 不认缺省的 `none`) 时, 在界面上照它填或留空 (留空即请求里不带这个参数) 即可; macOS 与 Windows 两端同形.
- 同一页再增一项「输出额度」文本框 (配置 `[predict] max_tokens`, 缺省 `200`): 填 0 或留空则请求里不带 `max_tokens`, 由服务商用自己的缺省值 —— 给不认这个参数 (新式推理模型要求 `max_completion_tokens`) 或要按服务商缺省跑的服务商. 释义兜底一次要写 8 个词的译词, 额度取配置值与自己的 600 里大的那个, 配置写 0 时它也不发.
- 组句里的上屏改成延迟: 一段拼音还没选完时 (例如双拼 `bilw` 先选了 `避`, 还剩 `lw`), 选中的词先留在 preedit 里 (`避lw`), 等这段拼音选完、回车原样上屏、取消组句或失焦时才真正交给应用. 退格按后进先出先把这个词拆回候选 (键还回缓冲区, 候选重新按整段拼音算), 拆完再删拼音字符, 与 Rime 的退格手感一致; 见 `docs/notes/crate-notes.md` 的 Engine 一节.
- 候选旁的模型标注: 被整句打分器重排过的候选, macOS 壳在其后标一个小字, 决策模型是 `AI 76% ↑2` (模型给这条的概率, 以及名次被抬了几位), 本地字级模型只标 `AI ↑2` (它的整句 log 概率不是概率, 折成百分比是假精度). Core 侧由 `Engine::model_hint` 给出 `ModelHint` (位移 + 置信度), 见 `docs/design/decision-models.md` 的模型标注一节.
- macOS 候选窗 / 模式徽标的 NSPanel 层级从 `kCGPopUpMenuWindowLevel` (101) 抬到 `CGShieldingWindowLevel()`, 截屏软件标注界面里也能看到拼音行和候选 (popup menu 会被截屏覆盖层压住).
- 候选词频次可调: 组句时按住调频键 (配置 `[shortcut] adjust_frequency`, 缺省 Ctrl) 时候选右侧显示每个词被选过的次数 (`频 全局/本串`), 同时按 K 升, 按 J 降当前高亮的候选, 每次升降等价于又选一次 / 撤销一次选择 (`Learner::adjust_frequency`), 降到底就停. 不在组句时这几个键一个都不拦 (终端里 Ctrl+J 仍是换行). macOS 壳在自己渲染时现算, Windows Server 的自绘候选窗与 Linux 的 fcitx5 面板走帧里的 `Frame::frequencies`. 见 `docs/user/getting-started/keys.md`.
- 组句中标点可先上屏候选: 新增配置 `[general] punctuation_first` (缺省 false, 保持 upstream 的「标点进英文直输段」行为), 打开后拼音打完直接敲 `,` `.` `?` 等先把高亮候选上屏、再按全角设置补上这个标点 (`ni'hao,` 出「你好，」), 不必先按空格; 表达式 / 问字模式、英文直输段、英文模式与配成翻页键的标点不受影响. Core 侧判据是 `Engine::punctuation_commits_candidate`, macOS 与 Windows / Linux 三个壳都接上, 偏好设置 / 设置的通用页各有一项.
- 候选高亮可用 `⌃P` / `⌃N` 上下挪: 新增 `[shortcut] highlight_down` / `highlight_up` (缺省 `control+n` / `control+p`, 写 `none` 关掉, 两边配成同一个键时整对退回缺省), 组句里与 `↓` / `↑` 完全同义, 到页边自动翻页, 不在组句时这两个键照旧归应用; 与调频的 `⌃J` / `⌃K` 各认各的字母 (配成同一个字母时归调频). 三端都接上: macOS 壳在按键分发里按键, 两个 Server 各加一条快捷键分支, Windows 的 DLL 经 `InputSettings` 的 `highlight_down` / `highlight_up` 拿到这两个组合键, 组句里先吃键再送 Server (协议加的是带缺省的两个字段, `PROTOCOL_VERSION` 不动). 见 `docs/user/getting-started/keys.md`.
- 双拼下调频不再失灵: 调频键 (缺省 Ctrl) + K / J 原来把次数记在敲的原始按键上 (双拼的 `mokk`), 词级排序查的却是解出的全拼 (`mokuai`), 两个键对不上, 所以怎么调候选都不挪位 (双拼下每个输入串都如此, 全拼下输入比候选长时也一样). 现在调频与上屏记账和排序共用同一把键 `Engine::choice_key_of` (候选覆盖的那段拼音, 双拼按解出的全拼算), 按几下就挪几下; CLI 交互模式加了 `:up N` / `:down N` 直接试调频, 见 `docs/notes/crate-notes.md`.
- 整句候选不再抢整词的位置: 整段拼音正好是词库里一个原样读音的整词时 (`mokk` 的 `模块`), 不再出整段拼音的整句候选 (`没会`), 判据 (`hit.exact` + 覆盖整段字母 + 末尾音节完整 + 不是模糊音 / 敲错命中 + 整段没有纠错生效) 取自 librime 的 `script_translator` (那边同样是 `make sentences when there is no exact-matching phrase candidate`, 并把 correction match 排除在 reliable phrase 之外); 整句路径上的用户加分也从 "每个词各加一次, 各自封顶 1.52" (拆成 `没` + `会` 能拿 3.04, 压过整词 `模块` 的 1.52, 20 个音节的句子能堆到 30 分盖过语言模型) 改成按 "这个词覆盖的音节数 ÷ 整段音节数" 折算, 一条路径的加分总和因此不超过 1.52. 见 `docs/notes/crate-notes.md`.

青简（Qingjian）是一个使用 **Rust** 开发的跨平台输入法。

https://github.com/user-attachments/assets/d145fde9-a641-4543-8b15-dd7a2685de3d

上面这段话全部由青简在 macOS 上输入：整句拼音一口气敲完，停顿一下由本地小模型重排候选，候选旁附英文译文与词性。

它的目标不只是「把拼音转换成中文」，而是让输入本身成为一种轻量、持续、几乎没有额外负担的语言学习方式。

当你输入文字时，青简会在候选词旁边显示一条简洁的目标语言译文。

例如，当你的主要语言是中文、正在学习英语时：

```text
1  开发            development
2  编程            programming
3  架构            architecture
4  编译            compile
5  语言            language
```

候选词仍然是输入的主体，翻译只作为较小、较浅的辅助信息存在。

**一次只学习一种语言。** 青简不会在一个候选项旁边同时塞入英语、日语、韩语、德语。保持输入体验干净，比堆砌信息更重要。

- 官网：[qingjian.app](https://qingjian.app)
- 下载：[qingjian.app/download](https://qingjian.app/download)（macOS、Windows）
- 文档：[qingjian.app/docs](https://qingjian.app/docs)（安装、按键、设置、数据与隐私）
- 反馈：[GitHub Issues](https://github.com/qingjian-team/qingjian/issues/new/choose)
- QQ 群：[902314603](https://qm.qq.com/q/jBvn2gGTxm)（青简输入法用户内测体验交流群）

---

## 为什么叫「青简」

「简」是古代记录文字的载体。竹木成简，文字成书。

「青简」也常被用来指代书籍、典籍与文字记录。

我们希望这个名字既保留中文书写文化的意味，又不过度限制输入法未来所支持的语言。青简首先面向中文使用者，但它并不准备永远只做中文输入法。

---

## 核心理念

> 学习语言为什么一定要专门打开一个学习软件？

聊天、写代码、搜索、记笔记、写文档、发邮件……大量时间其实都花在输入文字上。

如果这些每天发生数百次的输入行为，本身就能顺便提供一点语言反馈，语言学习就可以从「专门腾时间学习」变成日常行为的一部分。

**输入的时候，顺便多认识一个词。**

不打断。不弹题。不强迫记忆。只是悄悄地把翻译放在那里。

---

## 语言模式

用户可以设置：

```text
Primary Language: 中文
Learning Language: English
```

切换学习语言为日语后，同样的候选会显示：

```text
开发                開発
学习                学習
语言                言語
```

未来支持其他输入模式后，也可以反过来：

```text
こんにちは          你好
日本語              日语
```

核心原则不变：**一个候选词，只显示一种辅助语言。**

---

## 平台

青简从一开始就按跨平台架构设计：核心输入引擎平台无关，各平台只负责接入系统输入接口与候选窗口。

```text
macOS    → Input Method Kit (IMK)
Windows  → Text Services Framework (TSF)
Linux    → IBus / Fcitx
```

开发顺序是 macOS 优先；Windows 版已进入内测（TSF 文本服务 + 独立的输入引擎进程）。

---

## 不打算做什么

青简暂时不准备成为一个「大而全」的语言学习软件。它不会：

- 在候选框塞入五六种语言
- 每输入几个词就弹出测试
- 强制用户背单词
- 用复杂 UI 干扰正常输入
- 为了学习功能牺牲输入效率

输入法首先必须是一个好用的输入法。语言学习建立在这个前提之上。

如果用户需要思考「我现在到底是在打字还是在背单词」，那青简大概就设计错了。

---

## Philosophy

**输入优先。**

**学习自然发生。**

**平台只是壳，Core 才是青简。**

---

## 隐私

**青简不上传任何数据。** 拼音转换、词库、学习、释义全部在本机完成，没有账号，没有统计上报。
检查更新每天向官网读一次版本列表，请求不带任何标识，可在设置的「关于」页关掉。
云联想（缺省关闭）打开后，请求直接从你的电脑发到你自己填写的 AI 服务商，不经过作者；输入日志只写在本机，可以随时关闭和清空。
细节见文档 [数据与日志](https://qingjian.app/docs/help/data-and-logs)。

---

## 许可

代码以 **GPL-3.0-or-later** 发布（见 [LICENSE](LICENSE)）：可以自由使用、修改与再分发，修改后分发须同样开源。
「青简」名字与 logo 不在授权范围内。青简在官方渠道免费；若你为获得它向他人付费，你被骗了。

随包数据（词库、语言模型、释义表、emoji、英文词表、词汇等级、五笔码表）各自遵循来源的许可证，清单见 [docs/design/landscape.md](docs/design/landscape.md)，偏好设置「关于」页也列了一份。

---

## 参与开发

技术架构、设计决定、路线图与工程记录见 [`docs/`](docs/)；改代码前先看 [开发约定](docs/contributing.md)。
欢迎提 issue 与 PR，PR 模板里有合并前清单。

---

## Status

测试版，自用中，正在给少数测试者打包。API、项目结构和功能设计都可能发生较大变化。

---

<p align="center">
  <strong>青简 Qingjian</strong><br/>
  输入的不只是文字。
</p>

# 决策模型接入 (jev / laya)

2026-09-22. 整句重排的第二打分来源从 "只能接字级语言模型" 扩成 "也能接判断类模型".

## 是什么

jev 与 laya 是同一类模型: **typed decision**. 给一段 state (文本或 JSON) 和若干道题, 它们不生成任何文本,
一次前向直接给出答案. 三种题型:

- `choice`: 从若干选项里挑一条, 返回每个选项的概率;
- `score`: 给一个有序档位打期望值;
- `noul`: 判断一句命题成立的概率.

两个来源:

- **laya**: 开源权重 (`convaiinnovations/laya`, Apache-2.0), ModernBERT-large 421M 或 mmBERT 322M.
  Rust 侧有 candle 实现 (`laya` crate, 带 `laya-serve` 的 `POST /api/predict`), Apple 芯片上有 MLX 实现
  (`laya-mlx`, 一题 7-13 ms).
- **jev**: TypeSafe 的 System One 托管接口 (`https://api.typesafe.ai/v1/systemone`), 需要密钥.

## 接在哪

Core 里 `sentence::SentenceScorer` 那一个位置, 也就是整句重排的第二打分来源, 和 `[model]` 的本地字级模型互斥
(同一个位置只放得下一个). 选它是因为整句重排问的本来就是 "这几条候选里哪条最顺", 与 `choice` 题型同构;
现有的那条链路 (停顿后防抖, 缺分的先攒着, 后台线程打分, "前文 + 文本 -> 分" 缓存, 结果到了只换整句候选,
用户翻过页就不打扰) 一个字都不用改, 直接复用.

## 相对分

字级模型给的是整句的 `log P(文本 | 前文)`, 与路径分里的静态二元模型同量纲, 重排时直接把它顶掉.
决策模型给的是同一批候选上的概率分布, 数值本身不是概率意义上的绝对量, 只有同批之间的大小关系有意义.

所以 `SentenceScorer::form()` 分两种 (`ScoreForm`):

- `Absolute` (缺省, 字级模型): `路径分 += λ·(神经分 - 静态二元分)`;
- `Relative` (决策模型): 先按这一批候选的均值居中, 再 `路径分 += λ·(决策分 - 批内均值)`.

决策分由概率乘 `[decision] span` (nat) 得来, λ 仍是 Core 的 `NEURAL_WEIGHT` 0.5. span 是 "模型最偏好的那条
相对批内均值最多加多少分": 太小翻不动, 太大压过词库统计与个人 n-gram.

## 为什么只走 HTTP

不把推理栈内嵌进输入法: laya 的 Rust 实现目前依赖 candle 0.9 且只支持 f32 (权重常驻约 2.4 GB),
MLX 那条只在 Apple 芯片上. 青简只说协议: laya 打本地服务的 `/api/predict`, jev 打云端接口;
模型跑在哪台机器, 跑哪个 checkpoint, 要不要常驻, 都由用户自己定. 青简这一侧因此没有任何新推理依赖,
只多一个 `reqwest`.

## 协议

laya (本地服务, `questions` 是数组):

```json
{
  "state": "光标前的文字",
  "questions": [
    {
      "id": "natural",
      "type": "choice",
      "instructions": "接在前文之后, 哪一句读起来最自然?",
      "criteria": ["候选一", "候选二"]
    }
  ]
}
```

jev (`questions` 按 id 索引, `criteria` 是选项名到说明的对象, 鉴权用 `Authorization: Bearer <key>`):

```json
{
  "state": "光标前的文字",
  "model": "jev-latest",
  "questions": {
    "natural": {
      "type": "choice",
      "instructions": "接在前文之后, 哪一句读起来最自然?",
      "criteria": { "候选一": null, "候选二": null }
    }
  }
}
```

两者的响应形状一致 (`answers.<id>.choice` 与 `answers.<id>.probabilities`, 概率按选项文本索引),
解析共用一份实现.

## 隐私

- 云端后端会把光标前文发出去, 所以 `[decision] enabled` 默认关, 模型名与密钥都要用户自己填.
- Core 侧按 `SentenceScorer::is_remote` 判定: 为真的打分器在私密输入 (`Engine::set_private`) 期间不参与重排,
  既不查分也不把文本交给后台线程, 切进私密时已经攒下的待打分文本一并丢掉. 本地服务标为非远程.
- macOS 的 Secure Input 更靠前: 那里直接不组句, 也就没有整句候选可重排.
- 本地服务缺省地址是 `127.0.0.1`, 前文不出本机.

## 失败与降级

- 超时, 连不上, 响应形状不对: 这一轮不重排 (打分器返回空 Vec, Core 按 "没有这个打分" 处理),
  按键那头本来就不等它, 日志留一条 warn.
- 装配时校验地址与密钥: 起不来就退回本地整句模型 (`[model]` 开着的话), 两个都没有就不重排.
- 等待上限: 壳从把候选送出去到停止收结果等多久, 跟着 `[decision] timeout_ms` 走 (多留一秒), 本地整句模型
  仍用两秒的兜底值. 云端一次判断常要一两秒, 这个上限不跟着放宽的话, 结果回来时已经没人收.

## 配置

`[decision]` 分节 (`DecisionConfig`):

| 键 | 说明 |
|---|---|
| `enabled` | 开关, 缺省关 |
| `backend` | `laya` 本地服务 / `jev` 云端 |
| `endpoint` | 接口地址, 留空按后端取缺省 |
| `model` | 模型名, 只有 jev 用 |
| `api_key` / `api_key_env` | 密钥, 只有 jev 用; 缺省读 `TYPESAFE_API_KEY` |
| `timeout_ms` | 单次请求超时, 缺省 1500 |
| `context_chars` | 给模型看的前文长度, 缺省 48 |
| `span` | 决策分到路径分的换算跨度, 缺省 4.0 |

## 没做的

- 只用了 `choice`: `score` / `noul` 有明确用途时再接 (比如 "这段字母更像英文还是拼音").
- Windows / Linux 壳没接: Core 与配置两边共用, 接法与 macOS 相同.
- 没有 "测试连接" 按钮 (云联想有), 排查靠日志.

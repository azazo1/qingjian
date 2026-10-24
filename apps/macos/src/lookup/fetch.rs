//! zi.tools 查字: 单字详情与按部件查字.
//!
//! 接口是站点前端自用的 (没有文档, 没有版本号, 也没有速率承诺), 所以这里逐字段从
//! `serde_json::Value` 里取: 缺哪一段就少显示一段, 对方改了结构也只是少显示, 不会整个查不了.
//! 网络只在这个文件里, 窗口拿到的是 [`Entry`] 或一组字.

use std::fmt;
use std::time::Duration;

use serde_json::Value;

/// 请求超时: 站点在国内, 单字通常一秒内回; 六秒还不回就当失败.
const TIMEOUT: Duration = Duration::from_secs(6);

/// 响应上限: 实测常用字 460 KB (其中大半是笔画路径), 扩展区字 20 KB 上下.
const MAX_BYTES: usize = 2 * 1024 * 1024;

/// 释义最多留多少字: 康熙字典原文一段动辄几百字, 窗口里够读一段就行.
const MAX_SENSE_CHARS: usize = 300;

/// 字源的一条最多留多少字.
const MAX_ORIGIN_CHARS: usize = 80;

/// 部件查字最多列多少个字: 木木木 的响应里能翻出上百条, 窗口里列不完.
const MAX_COMPONENT_RESULTS: usize = 48;

/// 请求带的标识, 与检查更新一样只有产品名与用途, 不带任何标识用户的东西.
const USER_AGENT: &str = "qingjian-lookup/0.1";

/// 一次单字查询的结果, 只留显示得下的部分.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    /// 查的字.
    pub zi: char,

    /// 结构: 站点给的 IDS 写法, 例如 `⿱木林`.
    pub ids: String,

    /// 释义: (字典名, 正文). 只收《康熙字典》这类公有领域的.
    pub senses: Vec<(String, String)>,

    /// 字源: (出处, 正文), 例如 `《說文》`; 出处本来就带着书名号.
    pub origins: Vec<(String, String)>,
}

impl Entry {
    /// 结构与释义字源都没取到: 窗口显示「没有可显示的内容」而不是空一块.
    pub fn is_empty(&self) -> bool {
        self.ids.is_empty() && self.senses.is_empty() && self.origins.is_empty()
    }

    /// 窗口里显示的纯文本.
    pub fn render(&self) -> String {
        let mut out = self.zi.to_string();
        if !self.ids.is_empty() {
            out.push_str(&format!("\n结构  {}", self.ids));
        }
        for (name, body) in &self.senses {
            out.push_str(&format!("\n\n【{name}】\n{body}"));
        }
        for (source, body) in &self.origins {
            out.push_str(&format!("\n\n{source}\n{body}"));
        }
        out.push_str("\n\n来源  zi.tools 字統网");
        out
    }
}

/// 查字失败的原因; 窗口直接显示它的文字.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LookupError(pub String);

impl fmt::Display for LookupError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// 查一个字的详情. 阻塞到响应回来或者超时, 调用方必须放在自己的线程里.
pub fn detail(zi: char) -> Result<Entry, LookupError> {
    let url = format!("https://zi.tools/api/zi/{}", encode_path(&zi.to_string()));
    let value = get(&url)?;
    Ok(trim(zi, &value))
}

/// 按部件组合查字: 例如 `木 木 木` 出 森. 返回站点认为最像的那几十个字, 同位置命中的排在前面.
pub fn components(parts: &[char]) -> Result<Vec<char>, LookupError> {
    let joined = parts
        .iter()
        .map(|c| encode_path(&c.to_string()))
        .collect::<Vec<_>>()
        .join("%20");
    let url = format!("https://zi.tools/api/lookup/lookup/{joined}");
    let value = get(&url)?;
    Ok(component_characters(&value))
}

/// 发一次 GET 并解析 JSON.
fn get(url: &str) -> Result<Value, LookupError> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|error| LookupError(format!("建运行时失败: {error}")))?;
    runtime.block_on(async move {
        let client = reqwest::Client::builder()
            .timeout(TIMEOUT)
            .user_agent(USER_AGENT)
            .build()
            .map_err(|error| LookupError(format!("建客户端失败: {error}")))?;
        let response = client
            .get(url)
            .send()
            .await
            .map_err(|error| LookupError(format!("请求失败: {error}")))?;
        let status = response.status();
        if !status.is_success() {
            return Err(LookupError(format!("站点回 {status}")));
        }
        let bytes = response
            .bytes()
            .await
            .map_err(|error| LookupError(format!("读响应失败: {error}")))?;
        if bytes.len() > MAX_BYTES {
            return Err(LookupError(format!("响应过大: {} 字节", bytes.len())));
        }
        serde_json::from_slice(&bytes)
            .map_err(|error| LookupError(format!("响应不是 JSON: {error}")))
    })
}

/// 从整份单字响应里挑出要显示的部分.
fn trim(zi: char, value: &Value) -> Entry {
    let kv = value.get("yi").and_then(|yi| yi.get("kv"));
    let ids = kv
        .and_then(|kv| kv.get("ids_zi_tools"))
        .and_then(Value::as_str)
        .map(principal_ids)
        .unwrap_or_default();
    let senses = kv
        .and_then(|kv| kv.get("dict"))
        .and_then(Value::as_object)
        .map(public_senses)
        .unwrap_or_default();
    let origins = kv
        .and_then(|kv| kv.get("origin"))
        .and_then(Value::as_array)
        .map(|rows| origins(rows))
        .unwrap_or_default();
    Entry {
        zi,
        ids,
        senses,
        origins,
    }
}

/// 站点的 IDS 字段带变体标记 (`⿻十人(.);⿻十八(y)`), 取第一个变体、去掉标记.
fn principal_ids(raw: &str) -> String {
    let first = raw.split(';').next().unwrap_or_default().trim();
    match first.find('(') {
        Some(end) => first[..end].trim().to_owned(),
        None => first.to_owned(),
    }
}

/// 释义只收公有领域的字典: 《漢語多功能字庫》等是第三方有版权的内容, 不在别人的输入法里传播.
fn public_senses(dict: &serde_json::Map<String, Value>) -> Vec<(String, String)> {
    // 只收《康熙字典》: 公有领域的古籍原文.
    const ALLOWED: [&str; 1] = ["《康熙字典》"];
    ALLOWED
        .iter()
        .filter_map(|name| {
            let body = dict.get(*name)?.as_str()?;
            let text = clamp(body, MAX_SENSE_CHARS);
            (!text.is_empty()).then(|| (name.trim_matches(&['《', '》'][..]).to_owned(), text))
        })
        .collect()
}

/// 字源条目: 每行是 `[出处, 次要字段, 正文]`, 出处为空的行 (站点给的外文释义) 跳过.
fn origins(rows: &[Value]) -> Vec<(String, String)> {
    rows.iter()
        .filter_map(|row| {
            let row = row.as_array()?;
            let source = row.first()?.as_str()?.trim();
            let body = row.get(2)?.as_str()?;
            if source.is_empty() || source.chars().count() > 16 {
                return None;
            }
            let text = clamp(body, MAX_ORIGIN_CHARS);
            (!text.is_empty()).then(|| (source.to_owned(), text))
        })
        .take(3)
        .collect()
}

/// 部件查字的响应: 数组套数组, 组 0 是同位置命中的, 后面几组的置信度依次下降.
/// 条目里有真字也有 `⿰木林` 这种没编码的构形描述, 只留单字.
fn component_characters(value: &Value) -> Vec<char> {
    let mut out: Vec<char> = Vec::new();
    let Some(groups) = value.get("res").and_then(Value::as_array) else {
        return out;
    };
    for group in groups {
        let Some(items) = group.as_array() else {
            continue;
        };
        for item in items {
            let Some(text) = item
                .as_array()
                .and_then(|row| row.first())
                .and_then(Value::as_str)
            else {
                continue;
            };
            let mut han = text.chars().filter(|c| is_han(*c));
            let Some(single) = han.next() else {
                continue;
            };
            // 构形描述 (`⿰木林`) 里有两个以上汉字, 不是单字, 跳过
            if han.next().is_some() {
                continue;
            }
            if !out.contains(&single) {
                out.push(single);
            }
            if out.len() >= MAX_COMPONENT_RESULTS {
                return out;
            }
        }
    }
    out
}

/// 压成一行并截断: 字典原文里的全角空格与换行在窗口里没有意义.
fn clamp(text: &str, limit: usize) -> String {
    let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if text.chars().count() <= limit {
        return text;
    }
    let mut short = text.chars().take(limit).collect::<String>();
    short.push('…');
    short
}

/// 路径里的字按 UTF-8 逐字节转义, 只留 URL 里不用转义的字符.
fn encode_path(text: &str) -> String {
    let mut out = String::with_capacity(text.len() * 3);
    for &byte in text.as_bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                out.push(byte as char)
            }
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

/// 取一段文字里的所有汉字: 窗口按个数决定查详情还是查部件.
pub fn han_chars(text: &str) -> Vec<char> {
    text.chars().filter(|c| is_han(*c)).collect()
}

/// 汉字 (含扩展区与兼容区) 的判断. 标点与假名不算.
pub fn is_han(c: char) -> bool {
    let code = c as u32;
    matches!(code, 0x3400..=0x4DBF | 0x4E00..=0x9FFF | 0xF900..=0xFAFF | 0x20000..=0x3FFFF)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn principal_ids_drops_the_variant_marks() {
        assert_eq!(principal_ids("⿻十人(.);⿻十八(y)"), "⿻十人");
        assert_eq!(principal_ids("⿱木林"), "⿱木林");
        assert_eq!(principal_ids(""), "");
    }

    #[test]
    fn only_public_domain_senses_survive() {
        let raw = r#"{
            "__zi": "木",
            "yi": { "kv": {
                "dict": {
                    "《康熙字典》": "【辰集中】【木字部】 冒也。",
                    "漢語多功能字庫": "略說: 甲金文象樹木枝葉上竦。"
                },
                "ids_zi_tools": "⿻十人(.);⿻十八(y)",
                "origin": [["", "", "Tree."], ["《說文》", "", "冒也。冒地而生。"]]
            } }
        }"#;
        let entry = trim('木', &serde_json::from_str::<Value>(raw).unwrap());
        assert_eq!(entry.zi, '木');
        assert_eq!(entry.ids, "⿻十人");
        assert_eq!(entry.senses.len(), 1);
        assert_eq!(entry.senses[0].0, "康熙字典");
        assert_eq!(entry.origins.len(), 1);
        assert_eq!(entry.origins[0].0, "《說文》");
        assert!(!entry.is_empty());
        let rendered = entry.render();
        assert!(rendered.starts_with("木\n结构  ⿻十人"));
        assert!(rendered.contains("来源  zi.tools"));
    }

    #[test]
    fn component_results_keep_single_ideographs_only() {
        // 站点会把 `⿰木林` 这种构形描述一起返回; 只留单字, 组 0 在前, 去重
        let raw = r#"{"res":[
            [["森", [0, 0]], ["⿰木林", [0, 0]], ["⿱林朩", [0, 0]]],
            [["森", [45]], ["𣓏", [9]]]
        ]}"#;
        let found = component_characters(&serde_json::from_str::<Value>(raw).unwrap());
        assert_eq!(found, vec!['森', '𣓏']);
    }

    #[test]
    fn a_thin_response_still_makes_an_entry() {
        let entry = trim(
            '𠀀',
            &serde_json::from_str::<Value>(r#"{"__zi":"𠀀"}"#).unwrap(),
        );
        assert!(entry.is_empty());
    }

    #[test]
    fn han_chars_lists_ideographs_in_order() {
        assert_eq!(han_chars("木木木"), vec!['木', '木', '木']);
        assert_eq!(han_chars(" hello 森 "), vec!['森']);
        assert!(han_chars("abc").is_empty());
    }

    #[test]
    fn encode_path_escapes_utf8_bytes() {
        assert_eq!(encode_path("木"), "%E6%9C%A8");
        assert_eq!(encode_path("a-b"), "a-b");
    }
}

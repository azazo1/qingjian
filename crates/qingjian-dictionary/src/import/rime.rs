//! Rime `.dict.yaml`: YAML 头之后每行 `词\t拼音[\t权重]`, `#` 为注释.
//!
//! [`to_tsv`] 只解析这一份正文, 不去声调, 也不跟 `import_tables` (形码码表第二列是编码, 共用这个解析器).
//! [`from_path`] 跟 `import_tables` (相对主文件目录合表), 拼音去声调, 同词同音取最大权重;
//! 英文 / 反查表在合表时跳过.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::error::DictionaryError;
use crate::pattern::strip_tone;

/// `import_tables` 最多跟几层: 互相引用时绕不出来, 到这就停.
const MAX_IMPORT_DEPTH: usize = 8;

/// 解析结果: 青简 TSV 文本与 YAML 头里的名字.
pub struct Parsed {
    pub tsv: String,

    pub name: Option<String>,

    pub version: Option<String>,
}

struct Header {
    name: Option<String>,
    version: Option<String>,
    import_tables: Vec<String>,
    body: Vec<String>,
}

/// 有 YAML 头, 或第一条非注释行是 `name:` / `---` 就当 Rime 词库.
pub fn looks_like_rime(text: &str) -> bool {
    text.lines()
        .map(str::trim)
        .find(|l| !l.is_empty() && !l.starts_with('#'))
        .is_some_and(|l| l == "---" || l.starts_with("name:"))
}

/// 只解析这一份正文, 拼音原样留下. 形码码表走这条.
pub fn to_tsv(text: &str) -> Parsed {
    let header = parse_header(text);
    Parsed {
        tsv: emit_tsv(&header.body),
        name: header.name,
        version: header.version,
    }
}

/// 读主文件与它 `import_tables` 指到的其他词库, 拼音去声调后合成一份 TSV.
pub fn from_path(path: &Path) -> Result<Parsed, DictionaryError> {
    let mut merged: HashMap<(String, String), u32> = HashMap::new();
    let mut name = None;
    let mut version = None;
    let mut seen: Vec<PathBuf> = Vec::new();
    let mut queue: Vec<(PathBuf, usize)> = vec![(path.to_owned(), 0)];
    let mut main = true;
    while let Some((current, depth)) = queue.pop() {
        if depth > MAX_IMPORT_DEPTH {
            tracing::warn!(file = %current.display(), "import_tables 嵌套太深, 跳过");
            continue;
        }
        let identity = std::fs::canonicalize(&current).unwrap_or_else(|_| current.clone());
        if seen.contains(&identity) {
            continue;
        }
        seen.push(identity);
        let text = std::fs::read_to_string(&current)?;
        let header = parse_header(&text);
        if main {
            name = header.name.clone();
            version = header.version.clone();
            main = false;
        }
        absorb_body(&mut merged, &header.body);
        let dir = current.parent().unwrap_or(Path::new(".")).to_owned();
        for table in header.import_tables.iter().rev() {
            if skip_imported_table(table) {
                tracing::info!(table, "跳过英文或反查表");
                continue;
            }
            queue.push((resolve_import(&dir, table), depth + 1));
        }
    }
    Ok(Parsed {
        tsv: dump_merged(merged),
        name,
        version,
    })
}

fn parse_header(text: &str) -> Header {
    let mut header = Header {
        name: None,
        version: None,
        import_tables: Vec::new(),
        body: Vec::new(),
    };
    let mut in_header = false;
    let mut header_done = false;
    let mut pending_imports = false;
    for raw in text.lines() {
        let line = raw.trim_end();
        let trimmed = line.trim();
        if !header_done {
            if trimmed == "---" {
                in_header = true;
                continue;
            }
            if trimmed == "..." {
                header_done = true;
                pending_imports = false;
                continue;
            }
            if !in_header && (trimmed.is_empty() || trimmed.starts_with('#')) {
                continue;
            }
            if in_header || !line.contains('\t') {
                in_header = true;
                if pending_imports {
                    match trimmed.strip_prefix("- ") {
                        Some(item) => {
                            push_import(&mut header.import_tables, item);
                            continue;
                        }
                        None => pending_imports = false,
                    }
                }
                if let Some((key, value)) = split_key(trimmed) {
                    match key {
                        "name" => header.name = Some(unquote(value)),
                        "version" => header.version = Some(unquote(value)),
                        "import_tables" => match inline_list(strip_inline_comment(value)) {
                            Some(items) => header.import_tables.extend(items),
                            None => pending_imports = true,
                        },
                        _ => {}
                    }
                }
                continue;
            }
            header_done = true;
        }
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        header.body.push(line.to_owned());
    }
    header
}

fn absorb_body(merged: &mut HashMap<(String, String), u32>, body: &[String]) {
    for line in body {
        let Some((word, pinyin, weight)) = parse_entry(line, true) else {
            continue;
        };
        let slot = merged.entry((word, pinyin)).or_insert(weight);
        *slot = (*slot).max(weight);
    }
}

fn emit_tsv(body: &[String]) -> String {
    let mut tsv = String::new();
    for line in body {
        let Some((word, pinyin, weight)) = parse_entry(line, false) else {
            continue;
        };
        push_tsv(&mut tsv, &word, &pinyin, weight);
    }
    tsv
}

fn dump_merged(merged: HashMap<(String, String), u32>) -> String {
    let mut rows: Vec<_> = merged.into_iter().collect();
    rows.sort_unstable_by(|a, b| a.0.cmp(&b.0));
    let mut tsv = String::new();
    for ((word, pinyin), weight) in rows {
        push_tsv(&mut tsv, &word, &pinyin, weight);
    }
    tsv
}

fn parse_entry(line: &str, strip: bool) -> Option<(String, String, u32)> {
    let mut fields = line.split('\t');
    let word = fields.next()?.trim();
    let pinyin = fields.next()?.trim();
    if word.is_empty() || pinyin.is_empty() {
        return None;
    }
    let pinyin = if strip {
        normalize_pinyin(pinyin)
    } else {
        pinyin.to_owned()
    };
    if pinyin.is_empty() {
        return None;
    }
    let weight = fields
        .next()
        .map(str::trim)
        .filter(|w| !w.is_empty())
        .and_then(|w| w.parse::<u32>().ok())
        .unwrap_or(1);
    Some((word.to_owned(), pinyin, weight))
}

fn normalize_pinyin(pinyin: &str) -> String {
    pinyin
        .split_whitespace()
        .map(strip_tone)
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

fn push_tsv(tsv: &mut String, word: &str, pinyin: &str, weight: u32) {
    tsv.push_str(word);
    tsv.push('\t');
    tsv.push_str(pinyin);
    tsv.push('\t');
    tsv.push_str(&weight.to_string());
    tsv.push('\n');
}

/// 合表时跳过英文混输和反查表, 避免把 `He` 这类键写进拼音词库.
fn skip_imported_table(name: &str) -> bool {
    let stem = name.rsplit('/').next().unwrap_or(name).to_ascii_lowercase();
    stem == "en"
        || stem.contains("english")
        || stem.contains("reverse")
        || stem.contains("&en")
}

fn resolve_import(dir: &Path, name: &str) -> PathBuf {
    let candidate = dir.join(name);
    if candidate.exists() {
        return candidate;
    }
    for suffix in [".dict.yaml", ".yaml", ".yml"] {
        let candidate = dir.join(format!("{name}{suffix}"));
        if candidate.exists() {
            return candidate;
        }
    }
    candidate
}

fn push_import(tables: &mut Vec<String>, item: &str) {
    let item = unquote(strip_inline_comment(item));
    if !item.is_empty() {
        tables.push(item);
    }
}

fn strip_inline_comment(value: &str) -> &str {
    value.split('#').next().unwrap_or(value).trim()
}

fn split_key(line: &str) -> Option<(&str, &str)> {
    let (key, value) = line.split_once(':')?;
    let key = key.trim();
    (!key.is_empty() && !key.contains(char::is_whitespace)).then_some((key, value.trim()))
}

fn inline_list(value: &str) -> Option<Vec<String>> {
    let body = value.strip_prefix('[')?.strip_suffix(']')?;
    Some(
        body.split(',')
            .map(|item| unquote(item.trim()))
            .filter(|item| !item.is_empty())
            .collect(),
    )
}

fn unquote(value: &str) -> String {
    value.trim().trim_matches(['\'', '"']).to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn to_tsv_keeps_codes_and_untouched_pinyin() {
        let parsed = to_tsv("---\nname: law\n...\n合同法\the tong fa\t120\n民法典\tmin fa dian\n");
        assert_eq!(parsed.name.as_deref(), Some("law"));
        assert!(parsed.tsv.contains("合同法\the tong fa\t120"));
        assert!(parsed.tsv.contains("民法典\tmin fa dian\t1"));
    }

    #[test]
    fn from_path_strips_tones_follows_import_tables_and_skips_english() {
        let dir = std::env::temp_dir().join(format!(
            "qingjian-rime-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(dir.join("dicts")).unwrap();
        std::fs::write(
            dir.join("wanxiang.dict.yaml"),
            "---\nname: wanxiang\nversion: \"LTS\"\nimport_tables:\n  - dicts/zi            # 字表\n  - dicts/en\n  - dicts/cn&en\n...\n",
        )
        .unwrap();
        std::fs::write(
            dir.join("dicts/zi.dict.yaml"),
            "---\nname: zi\n...\n阿爸\tā bà\t275\n阿爸\tā bà\t10\n女儿\tnǚ ér\t80\n株木琅玛\tzhū mù láng mǎ\t1\t有声\n",
        )
        .unwrap();
        std::fs::write(
            dir.join("dicts/en.dict.yaml"),
            "---\nname: wanxiang_english\n...\nHe\tHe\n",
        )
        .unwrap();
        std::fs::write(
            dir.join("dicts/cn&en.dict.yaml"),
            "---\nname: cn&en\n...\n3D\t3D\n",
        )
        .unwrap();
        let parsed = from_path(&dir.join("wanxiang.dict.yaml")).unwrap();
        assert_eq!(parsed.name.as_deref(), Some("wanxiang"));
        assert_eq!(parsed.version.as_deref(), Some("LTS"));
        assert!(parsed.tsv.contains("阿爸\ta ba\t275"));
        assert!(parsed.tsv.contains("女儿\tnv er\t80"));
        assert!(parsed.tsv.contains("株木琅玛\tzhu mu lang ma\t1"));
        assert!(!parsed.tsv.contains("He"));
        assert!(!parsed.tsv.contains("3D"));
        std::fs::remove_dir_all(&dir).unwrap();
    }
}

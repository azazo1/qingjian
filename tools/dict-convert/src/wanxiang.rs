//! 万象拼音词库: 读 Rime 用户目录的总表 (`wanxiang.dict.yaml` 的 `import_tables`),
//! 去掉声调后写成青简基础词库 `dict.tsv` / `dict.qj`.
//!
//! 不把上游 yaml 打进 git. 英文混输表和反查表在合表时跳过.
//! 许可是 CC BY 4.0, 打包时写入元数据.

use std::path::{Path, PathBuf};
use std::time::Instant;

use qingjian_dictionary::Dictionary;
use qingjian_dictionary::import::{from_path, looks_like_rime};
use qingjian_format::Metadata;

use crate::error::ConvertError;

/// 万象拼音仓库.
const SOURCE: &str = "https://github.com/amzxyz/rime_wanxiang";

/// `$RIME_DIR`, 否则 `~/Library/Rime`.
pub fn default_rime_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("RIME_DIR") {
        let dir = dir.trim();
        if !dir.is_empty() {
            return PathBuf::from(dir);
        }
    }
    let home = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE"));
    PathBuf::from(home.unwrap_or_else(|| ".".into())).join("Library/Rime")
}

/// 读总表, 写出 `dict.tsv` 与 `dict.qj`.
pub fn convert(rime_dir: &Path, dict: &Path, out_dir: &Path) -> Result<(), ConvertError> {
    let source = if dict.is_absolute() {
        dict.to_owned()
    } else {
        rime_dir.join(dict)
    };
    let started = Instant::now();
    let text = std::fs::read_to_string(&source).map_err(|error| ConvertError::Format {
        path: source.clone(),
        line: 0,
        reason: format!("读万象总表失败: {error}"),
    })?;
    if !looks_like_rime(&text) {
        return Err(ConvertError::Format {
            path: source,
            line: 0,
            reason: "不是 Rime .dict.yaml (需要 `---` 或 `name:`)".to_owned(),
        });
    }
    let parsed = from_path(&source).map_err(|error| ConvertError::Format {
        path: source.clone(),
        line: 0,
        reason: format!("合表失败: {error}"),
    })?;
    if parsed.tsv.is_empty() {
        return Err(ConvertError::Format {
            path: source,
            line: 0,
            reason: "合表之后没有词条 (总表是否只有英文 / 反查表?)".to_owned(),
        });
    }
    std::fs::create_dir_all(out_dir)?;
    let tsv_path = out_dir.join("dict.tsv");
    std::fs::write(&tsv_path, &parsed.tsv)?;
    let dictionary = Dictionary::parse(&parsed.tsv)?;
    let metadata = Metadata {
        name: parsed
            .name
            .filter(|name| !name.is_empty())
            .unwrap_or_else(|| "万象拼音".to_owned()),
        license: "CC-BY-4.0".to_owned(),
        attribution: "amzxyz / 万象拼音".to_owned(),
        source: SOURCE.to_owned(),
        version: parsed.version.unwrap_or_default(),
        generator: format!("qingjian-dict-convert {}", env!("CARGO_PKG_VERSION")),
        ..Metadata::default()
    };
    let qj_path = out_dir.join("dict.qj");
    dictionary.write_qj(&qj_path, &metadata)?;
    tracing::info!(
        entries = dictionary.len(),
        tsv = %tsv_path.display(),
        qj = %qj_path.display(),
        elapsed_ms = started.elapsed().as_millis() as u64,
        "万象词库已写出"
    );
    Ok(())
}

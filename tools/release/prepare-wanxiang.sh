#!/usr/bin/env bash
# 临时 clone 万象拼音到 `.tmp/rime-wanxiang`, 转成 `data/generated/dict.qj`,
# 再把仓库里的释义表 / 英文词表打进同一目录, 给 CI 打包和本机没 Rime 用户目录时用.
#
#   tools/release/prepare-wanxiang.sh
#
# 覆盖来源: `WANXIANG_REPO` (缺省 https://github.com/azazo1/oh-my-rime.git),
# `WANXIANG_REF` (缺省 wanxiang 分支).
# 已有 `.tmp/rime-wanxiang/wanxiang.dict.yaml` 就不再 clone.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
CLONE="$ROOT/.tmp/rime-wanxiang"
REPO="${WANXIANG_REPO:-https://github.com/azazo1/oh-my-rime.git}"
REF="${WANXIANG_REF:-wanxiang}"
cd "$ROOT"

sha256() {
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$1"
  else
    shasum -a 256 "$1"
  fi | cut -d' ' -f1
}

mkdir -p "$ROOT/.tmp"
if [[ ! -f "$CLONE/wanxiang.dict.yaml" ]]; then
  rm -rf "$CLONE"
  clone_args=(--depth 1 --filter=blob:none --sparse)
  if [[ -n "$REF" ]]; then
    clone_args+=(--branch "$REF")
  fi
  echo "clone $REPO -> $CLONE"
  git clone "${clone_args[@]}" "$REPO" "$CLONE"
  # cone 模式只接受目录: 根上的 wanxiang.dict.yaml 会跟着带上, 不要把文件名塞进去
  git -C "$CLONE" sparse-checkout set dicts
fi
[[ -f "$CLONE/wanxiang.dict.yaml" ]] || {
  echo "clone 之后没有 wanxiang.dict.yaml: $CLONE" >&2
  exit 1
}

echo "convert wanxiang $(git -C "$CLONE" rev-parse --short HEAD)"
cargo run --release --locked -p qingjian-dict-convert -- wanxiang --rime-dir "$CLONE"

for lang in en ja zh es; do
  src="assets/glossary/glossary-$lang.tsv"
  out="data/generated/glossary-$lang.qj"
  [[ -f "$src" ]] || continue
  if [[ "$lang" == es ]]; then
    license="GPL-3.0-or-later"
    attribution="Azure Translator 机器翻译（Tofuzhu，tools/corpus/glossary_es.py）"
  else
    license="MIT"
    attribution="LLM 生成（DeepSeek），qingjian-gloss-gen"
  fi
  if [[ ! -f "$out" || "$src" -nt "$out" ]]; then
    cargo run --release --locked -q -p qingjian-dict-convert -- pack glossary --language "$lang" --input "$src" \
      --name "青简释义表（${lang}）" --license "$license" --attribution "$attribution"
  fi
done
if [[ -f assets/lexicon/english.tsv ]]; then
  cp assets/lexicon/english.tsv data/generated/english.tsv
fi

[[ -f data/generated/dict.qj ]] || {
  echo "转换之后没有 data/generated/dict.qj" >&2
  exit 1
}

rev="$(git -C "$CLONE" rev-parse HEAD)"
echo "万象 $rev 已写成 data/generated/dict.qj"
if [[ -n "${GITHUB_ENV:-}" ]]; then
  {
    echo "DATA_TAG=wanxiang-${rev:0:7}"
    echo "DATA_SHA256=$(sha256 data/generated/dict.qj)"
  } >> "$GITHUB_ENV"
fi

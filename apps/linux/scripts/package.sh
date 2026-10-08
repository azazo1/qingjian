#!/usr/bin/env bash
# //! 打预编译包 target/linux-package/qingjian-<版本>-linux-<cpu>.tar.gz：Server、Fcitx5 插件、产品数据与安装脚本。
# //! 先跑 tools/release/prepare-wanxiang.sh；包里按仓库相对路径放文件，解开后的 install.sh 与源码安装共用 files.py 的清单。
set -euo pipefail
root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../../.." && pwd)
cd "$root"
version=$(sed -n 's/^version = "\(.*\)"/\1/p' apps/linux/server/Cargo.toml | head -1)
if [[ "$version" == *-dev ]]; then
  version+="-$(git rev-parse --short=7 HEAD)"
  git diff --quiet HEAD || version+='+'
fi
[[ -f data/generated/dict.qj ]] || { echo '缺少产品数据, 先运行 tools/release/prepare-wanxiang.sh' >&2; exit 1; }

cargo_output=$(realpath -m -- "${CARGO_TARGET_DIR:-$root/target}")
cargo build --release --locked -p qingjian-linux-server --target-dir "$cargo_output"
cmake_output="$root/target/fcitx5-package"
cmake -S apps/linux/fcitx5 -B "$cmake_output" -DCMAKE_BUILD_TYPE=Release -DBUILD_TESTING=OFF
cmake --build "$cmake_output" --parallel "${CMAKE_BUILD_PARALLEL_LEVEL:-2}"

name="qingjian-$version-linux-$(uname -m)"
out="$root/target/linux-package"
stage="$out/$name"
rm -rf "$stage" "$out/$name.tar.gz"
mkdir -p "$stage/bin" "$stage/lib"
install -m 755 "$cargo_output/release/qingjian-linux-server" "$stage/bin/"
install -m 755 "$cmake_output/qingjian.so" "$stage/lib/"
install -m 755 apps/linux/scripts/package-install.sh "$stage/install.sh"
install -m 755 apps/linux/scripts/uninstall.sh "$stage/"
install -m 644 apps/linux/scripts/files.py "$stage/"
# 产品数据随包放 data/generated/, install.sh 不再解上游 data-vN 压缩包
cp -r --parents LICENSE assets/icon/logo.png apps/linux/fcitx5/data assets/sample assets/glossary assets/levels assets/emoji \
  data/generated/dict.qj "$stage/"
for extra in data/generated/english.tsv data/generated/glossary-en.qj data/generated/glossary-ja.qj \
  data/generated/glossary-zh.qj data/generated/glossary-es.qj; do
  [[ -f "$extra" ]] && cp -r --parents "$extra" "$stage/"
done
tar -C "$out" -czf "$out/$name.tar.gz" "$name"
echo "已打包：$out/$name.tar.gz"

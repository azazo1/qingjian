[private]
default:
    @just --list

# tools/release/prepare-wanxiang.sh
# 临时 clone 万象拼音并转成 data/generated/dict.qj.
prepare-wanxiang:
    bash tools/release/prepare-wanxiang.sh

# 先跑 just prepare-wanxiang 备好词库; 没有 data/generated/ 时 bundle.sh 会静默退回 assets/sample/ 的样例.
# 打包 macOS 分发包到 target/pkg/ (release 构建, 未签名未公证).
macos-pkg:
    bash apps/macos/scripts/bundle.sh --pkg

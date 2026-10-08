[private]
default:
    @just --list

# tools/release/prepare-wanxiang.sh
# 临时 clone 万象拼音并转成 data/generated/dict.qj.
prepare-wanxiang:
    bash tools/release/prepare-wanxiang.sh

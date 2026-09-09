#!/usr/bin/env bash
# Linux -> Windows x64 候选入口；构建/真机验收分别记录，不相互冒充。
set -Eeuo pipefail
repo_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd -P)"
exec python3 "$repo_root/scripts/build_internal_release.py" --platform win-x64 --cross-windows "$@"

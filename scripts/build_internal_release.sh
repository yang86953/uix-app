#!/usr/bin/env bash
# Linux x64 当前候选入口；参数原样传递给共同合同。
set -Eeuo pipefail
repo_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd -P)"
exec python3 "$repo_root/scripts/build_internal_release.py" --platform linux-x64 "$@"

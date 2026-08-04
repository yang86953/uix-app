# 使用方构建基线 fixture

这里维护 ODC-01 的三个相互独立入口：

- `minimal`：关闭默认 feature 的最小入口。
- `d3d11-default`：使用当前默认 D3D11 feature 的入口。
- `settings-serde`：关闭默认 feature，仅启用 `settings-serde`。

从仓库根目录运行：

```powershell
python scripts/measure_usage_build.py --dry-run
python scripts/measure_usage_build.py --locked
```

采集器对每个 fixture 默认执行以下顺序：`cargo clean`、`cargo metadata`、`cargo build --release`、`cargo clean`。因此正常采集结束后不会保留 Rust `target` 构建产物；只有显式传入 `--keep-build-artifacts` 才会跳过后置清理。`--dry-run` 在没有 Rust 工具链时只展示命令，不写入基线证据。

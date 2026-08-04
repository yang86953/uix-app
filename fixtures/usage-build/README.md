# 使用方构建基线 fixture

这里维护 ODC-01/ODC-06/ODC-07 的七个相互独立入口：

- `minimal`：关闭默认 feature 的最小入口。
- `d3d11-default`：使用当前默认 D3D11 feature 的入口。
- `settings-serde`：关闭默认 feature，仅启用 `settings-serde`。
- `qrcode`：关闭默认 feature，仅启用 `qrcode` 并使用公开 `QRCode` 类型。
- `form-pattern`：关闭默认 feature，仅启用 `form-pattern` 并使用 `validate_pattern` 规则。
- `qrcode-disabled`：关闭默认 feature，却故意导入公开 `QRCode`；该入口必须以匹配诊断的 compile-fail 结束。
- `form-pattern-disabled`：关闭默认 feature，却故意调用 `validate_pattern`；该入口必须以匹配诊断的 compile-fail 结束。

从仓库根目录运行：

```powershell
python scripts/measure_usage_build.py --dry-run
python scripts/measure_usage_build.py --locked
```

采集器先为每个 fixture 执行 `cargo clean`、`cargo metadata` 与依赖存在性断言。五个正向入口继续执行 `cargo build --release`；两个 `*-disabled` 入口执行预期失败的 `cargo check`，并匹配各自稳定诊断片段。`minimal`、`settings-serde` 和两个禁用入口断言 `qrcode`、`regex` 均缺席；单能力入口只允许自己的专属 package 出现；默认入口断言两个默认 capability 的专属 package 均存在。所有入口最后执行 `cargo clean`，因此正常采集结束后不会保留 Rust `target` 构建产物；只有显式传入 `--keep-build-artifacts` 才会跳过后置清理。`--dry-run` 在没有 Rust 工具链时只展示命令，不写入基线证据。

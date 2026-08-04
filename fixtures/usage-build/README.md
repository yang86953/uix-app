# 使用方构建基线 fixture

这里维护 ODC-01/ODC-06/ODC-07 的九个独立 fixture 与两个根二进制配置，共十一个相互清理的入口：

- `minimal`：关闭默认 feature 的最小入口。
- `d3d11-default`：使用当前默认 D3D11 feature 的入口。
- `demo-logging-disabled`：根清单关闭默认 feature，显式启用演示所需的 `d3d11 + image-codecs + qrcode + form-pattern`，但不启用日志订阅器。
- `demo-logging`：使用相同演示基础组合，再启用 `demo-logging` 并构建 `uix-demo`。
- `settings-serde`：关闭默认 feature，仅启用 `settings-serde`。
- `image-codecs`：关闭默认 feature，仅启用 `image-codecs` 并调用公开字节解码方法。
- `qrcode`：关闭默认 feature，仅启用 `qrcode` 并使用公开 `QRCode` 类型。
- `form-pattern`：关闭默认 feature，仅启用 `form-pattern` 并使用 `validate_pattern` 规则。
- `qrcode-disabled`：关闭默认 feature，却故意导入公开 `QRCode`；该入口必须以匹配诊断的 compile-fail 结束。
- `form-pattern-disabled`：关闭默认 feature，却故意调用 `validate_pattern`；该入口必须以匹配诊断的 compile-fail 结束。
- `image-codecs-disabled`：关闭默认 feature，却故意调用 `ImageService::load_from_bytes`；该入口必须以匹配诊断的 compile-fail 结束。

从仓库根目录运行：

```powershell
python scripts/measure_usage_build.py --dry-run
python scripts/measure_usage_build.py --locked
```

采集器先为每个入口执行 `cargo clean`、`cargo metadata` 与依赖存在性断言。八个正向入口继续执行 `cargo build --release`；三个公开面禁用入口执行预期失败的 `cargo check`，并匹配各自稳定诊断片段。`minimal`、`settings-serde` 与三个公开面禁用入口断言未选择的 `image`、`qrcode`、`regex`、`tracing-subscriber` 均缺席；单能力入口只允许自己的专属 package 出现；默认入口断言三个默认 capability 与 `tracing-subscriber` 均存在。十一个入口还统一断言已删除且无源码调用点的 `raw-window-handle` 不得重新进入 resolved graph。两个演示日志场景直接绑定根清单的 `uix-demo` 目标并使用相同的非日志基础 feature 组合：禁用场景要求 `image`、`qrcode`、`regex` 存在且 `tracing-subscriber` 缺席，启用场景再要求订阅器出现，从而分别证明订阅器启用与禁用时演示二进制都能 clean release build。所有入口最后执行 `cargo clean`，因此正常采集结束后不会保留 Rust `target` 构建产物；只有显式传入 `--keep-build-artifacts` 才会跳过后置清理。`--dry-run` 在没有 Rust 工具链时只展示命令，不写入基线证据。

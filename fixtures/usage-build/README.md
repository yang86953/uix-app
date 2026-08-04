# 使用方构建基线 fixture

这里维护 ODC-01/ODC-06/ODC-07 的十三个独立 fixture、两个根二进制配置与一个复用最小 fixture 的 Linux 目标配置，共十六个相互清理的入口：

- `minimal`：关闭默认 feature 的最小入口。
- `minimal-linux`：复用 `minimal` 的 feature 集，显式绑定 `x86_64-unknown-linux-gnu`，要求 Linux 平台依赖存在且 Windows 依赖缺席，并执行真实 `cargo check`。
- `d3d11-default`：使用当前默认 D3D11 feature 的入口。
- `demo-logging-disabled`：根清单关闭默认 feature，显式启用演示所需的 `d3d11 + image-codecs + qrcode + form-pattern + rich-text`，但不启用日志订阅器。
- `demo-logging`：使用相同演示基础组合，再启用 `demo-logging` 并构建 `uix-demo`。
- `settings-serde`：关闭默认 feature，仅启用 `settings-serde`。
- `agent-control`：关闭默认 feature，仅启用 `agent-control` 并调用公开应用 builder 方法。
- `image-codecs`：关闭默认 feature，仅启用 `image-codecs` 并调用公开字节解码方法。
- `qrcode`：关闭默认 feature，仅启用 `qrcode` 并使用公开 `QRCode` 类型。
- `form-pattern`：关闭默认 feature，仅启用 `form-pattern` 并使用 `validate_pattern` 规则。
- `rich-text`：关闭默认 feature，仅启用 `rich-text` 并同时使用公开组件与解析辅助函数。
- `qrcode-disabled`：关闭默认 feature，却故意导入公开 `QRCode`；该入口必须以匹配诊断的 compile-fail 结束。
- `form-pattern-disabled`：关闭默认 feature，却故意调用 `validate_pattern`；该入口必须以匹配诊断的 compile-fail 结束。
- `image-codecs-disabled`：关闭默认 feature，却故意调用 `ImageService::load_from_bytes`；该入口必须以匹配诊断的 compile-fail 结束。
- `rich-text-disabled`：关闭默认 feature，却故意导入 `RichText` 与 `parse_rich_text`；该入口必须以同时匹配两个公开符号的 compile-fail 结束。
- `agent-control-disabled`：关闭默认 feature，却故意调用 `App::enable_agent_control`；该入口必须以匹配缺失方法诊断的 compile-fail 结束。

从仓库根目录运行：

```powershell
python scripts/measure_usage_build.py --dry-run
python scripts/measure_usage_build.py --locked
```

采集器先从 `rustc -vV` 解析 host target，并为每个入口执行 `cargo clean`、带同一 `--filter-platform` 的 `cargo metadata`、依赖存在性断言与 uix feature 正反断言；正向构建和正反类型检查都显式传入相同的 `--target`，避免 resolved graph 混入其他平台条件依赖。十个正向 host 入口继续执行 `cargo build --release`；`minimal-linux` 在 Windows 宿主上执行真实 `cargo check`，不要求额外的 Linux 链接器，并断言 `libc`、`wayland-client` 存在而 `windows`、`windows-core` 缺席；五个公开面禁用入口执行预期失败的 `cargo check`，并匹配各自稳定诊断片段。未选择对应 capability 的场景断言 `image`、`qrcode`、`regex`、`serde_json`、`tracing-subscriber` 等专属 package 缺席；有专属 package 的单能力入口只允许自己的 package 出现；默认入口断言三项默认第三方 capability 与 `tracing-subscriber` 均存在。`settings-serde` 与 `agent-control` 都要求共享的 `serde_json` 存在，但前者另要求直接 `serde` package 与 `settings-serde` feature，后者要求 `agent-control` feature 并禁止设置 feature 合并；Agent 禁用入口同时要求 `serde_json` 与公开 builder 方法缺席。schema v4 继续对没有专属 package 的 `rich-text` 执行精确 uix feature 断言，并对 Agent、设置、默认、演示与最小场景执行 feature 正反断言，避免共享 package 或默认组合掩盖能力误入。十六个入口还统一断言已删除且无源码调用点的 `raw-window-handle` 不得重新进入 resolved graph；十二个未启用 `image-codecs` 的入口另行断言已删除直接边的精确 package `bytemuck` 缺席，四个启用图片能力的入口则允许 `image` 自身合法传递引入它。两个演示日志场景直接绑定根清单的 `uix-demo` 目标并使用相同的 `d3d11 + image-codecs + qrcode + form-pattern + rich-text` 非日志基础组合：禁用场景要求 `image`、`qrcode`、`regex` 存在且 `tracing-subscriber` 缺席，启用场景再要求订阅器出现，从而分别证明订阅器启用与禁用时演示二进制都能 clean release build。所有入口最后执行 `cargo clean`，因此正常采集结束后不会保留 Rust `target` 构建产物；只有显式传入 `--keep-build-artifacts` 才会跳过后置清理。`--dry-run` 在没有 Rust 工具链时只展示命令，不写入基线证据。

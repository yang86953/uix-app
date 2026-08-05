# 使用方构建基线 fixture

这里维护 ODC-01/ODC-04/ODC-06/ODC-07 的二十八个独立 fixture 目录、两个根二进制配置、复用最小与 Agent fixture 的两个 Linux 目标配置，以及复用 `backend-selection` fixture 的额外五个 feature 配置，共三十七个相互清理的入口：

- `minimal`：关闭默认 feature 的最小入口。
- `minimal-linux`：复用 `minimal` 的 feature 集，显式绑定 `x86_64-unknown-linux-gnu`，要求 Linux 平台依赖存在且 Windows 依赖缺席，并执行真实 `cargo check`。
- `d3d11-default`：使用当前默认 D3D11 feature 的入口。
- `d3d11`：关闭默认 feature，仅启用 `d3d11` 并显式选择公开 `Direct3D11` 变体。
- `opengles`：关闭默认 feature，仅启用 `opengles` 并显式选择公开 `OpenGlEs` 变体。
- `d3d12-selection`：复用 `backend-selection`，只转发 `d3d12` 并选择公开 `Direct3D12` 变体；只证明选择面与清单 feature，不作为生产 backend 证据。
- `vulkan-selection`：复用 `backend-selection`，只转发 `vulkan`、要求 `ash` 进入解析图并选择公开 `Vulkan` 变体；只证明选择面、依赖与源码构建，不作为生产 registry 证据。
- `metal-selection`：复用 `backend-selection`，只转发空依赖的 `metal` 并选择公开 `Metal` 变体；只证明选择面，不作为 Windows 或 macOS 生产 backend 证据。
- `demo-logging-disabled`：根清单关闭默认 feature，显式启用演示所需的 `d3d11 + image-codecs + qrcode + form-pattern + rich-text + charts + table + navigation + feedback + tree-widgets`，但不启用日志订阅器。
- `demo-logging`：使用相同演示基础组合，再启用 `demo-logging` 并构建 `uix-demo`。
- `settings-serde`：关闭默认 feature，仅启用 `settings-serde`。
- `agent-control`：关闭默认 feature，仅启用 `agent-control` 并调用公开应用 builder 方法。
- `agent-control-linux`：复用 `agent-control` 的公开入口，显式绑定 `x86_64-unknown-linux-gnu`，要求 Unix/Wayland 与 Agent 依赖存在、Windows 依赖缺席，并执行真实 release ELF 链接。
- `image-codecs`：关闭默认 feature，仅启用 `image-codecs` 并调用公开字节解码方法。
- `qrcode`：关闭默认 feature，仅启用 `qrcode` 并使用公开 `QRCode` 类型。
- `form-pattern`：关闭默认 feature，仅启用 `form-pattern` 并使用 `validate_pattern` 规则。
- `rich-text`：关闭默认 feature，仅启用 `rich-text` 并同时使用公开组件与解析辅助函数。
- `charts`：关闭默认 feature，仅启用 `charts` 并同时使用基础 `BarChart` 与高级 `Gauge` 公开类型。
- `table`：关闭默认 feature，仅启用 `table` 并同时使用基础 `Table` 与泛型 `DataTable` 公开类型。
- `navigation`：关闭默认 feature，仅启用 `navigation` 并同时使用基础 `Breadcrumb` 与泛型 `Navigation<u8>` 公开类型。
- `feedback`：关闭默认 feature，仅启用 `feedback` 并同时使用基础 `Alert`、弹层 `Modal` 与全局 `message()/notify()` 门面。
- `tree-widgets`：关闭默认 feature，仅启用 `tree-widgets` 并同时使用 `Tree`、`TreeNode`、`TreeSelect`、`DropPosition` 与 `SnapshotTreeNode` 公开类型。
- `qrcode-disabled`：关闭默认 feature，却故意导入公开 `QRCode`；该入口必须以匹配诊断的 compile-fail 结束。
- `form-pattern-disabled`：关闭默认 feature，却故意调用 `validate_pattern`；该入口必须以匹配诊断的 compile-fail 结束。
- `image-codecs-disabled`：关闭默认 feature，却故意调用 `ImageService::load_from_bytes`；该入口必须以匹配诊断的 compile-fail 结束。
- `rich-text-disabled`：关闭默认 feature，却故意导入 `RichText` 与 `parse_rich_text`；该入口必须以同时匹配两个公开符号的 compile-fail 结束。
- `charts-disabled`：关闭默认 feature，却故意导入基础 `BarChart` 与高级 `Gauge`；该入口必须以同时匹配两个公开类型的 compile-fail 结束。
- `table-disabled`：关闭默认 feature，却故意导入基础 `Table` 与泛型 `DataTable`；该入口必须以同时匹配两个公开类型的 compile-fail 结束。
- `navigation-disabled`：关闭默认 feature，却故意导入基础 `Breadcrumb` 与泛型 `Navigation<u8>`；该入口必须以同时匹配两个公开类型的 compile-fail 结束。
- `feedback-disabled`：关闭默认 feature，却故意导入 `Alert`、`Modal` 与全局 `message()/notify()`；该入口必须以同时匹配组件族和两个门面函数的 compile-fail 结束。
- `tree-widgets-disabled`：关闭默认 feature，却故意导入 `Tree`、`TreeNode`、`TreeSelect`、`DropPosition` 与 `SnapshotTreeNode`；该入口必须以同时匹配树组件族和快照公开类型的 compile-fail 结束。
- `agent-control-disabled`：关闭默认 feature，却故意调用 `App::enable_agent_control`；该入口必须以匹配缺失方法诊断的 compile-fail 结束。
- `d3d11-disabled`：关闭默认 feature，却故意选择公开 `Direct3D11` 变体；该入口必须以匹配缺失变体诊断的 compile-fail 结束。
- `opengles-disabled`：关闭默认 feature，却故意选择公开 `OpenGlEs` 变体；该入口必须以匹配缺失变体诊断的 compile-fail 结束。
- `d3d12-selection-disabled`：复用 `backend-selection`，不向 `uix` 转发 `d3d12` 却故意选择 `Direct3D12`；该入口必须以匹配缺失变体诊断的 compile-fail 结束。
- `vulkan-selection-disabled`：复用 `backend-selection`，不向 `uix` 转发 `vulkan` 却故意选择 `Vulkan`；该入口必须在 `ash` 缺席时以匹配缺失变体诊断的 compile-fail 结束。
- `metal-selection-disabled`：复用 `backend-selection`，不向 `uix` 转发 `metal` 却故意选择 `Metal`；该入口必须以匹配缺失变体诊断的 compile-fail 结束。

从仓库根目录运行：

```powershell
python scripts/measure_usage_build.py --dry-run
# Windows 等交叉宿主需把该变量指向可执行的 Linux GNU linker；例如调用 `zig cc -target x86_64-linux-gnu` 的包装器。
$env:CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER = "C:\tools\zigcc.exe"
python scripts/measure_usage_build.py --locked
```

采集器先从 `rustc -vV` 解析 host target，并为每个入口执行 `cargo clean`、带同一 `--filter-platform` 的 `cargo metadata`、依赖存在性、精确 package feature 与 uix feature 正反断言；正向构建和正反类型检查都显式传入相同的 `--target`，避免 resolved graph 混入其他平台条件依赖。二十个正向 host 入口继续执行 `cargo build --release`；`minimal-linux` 在 Windows 宿主上执行真实 `cargo check`，不要求额外的 Linux 链接器，并断言 `libc`、`wayland-client` 存在而 `windows`、`windows-core` 缺席；`agent-control-linux` 则必须完成最终 release 链接，交叉宿主未设置 Cargo 标准 linker 环境变量时会在构建前明确失败。十五个公开面禁用入口执行预期失败的 `cargo check`，并匹配各自稳定诊断片段。未选择对应 capability 的场景断言 `image`、`qrcode`、`regex`、`serde_json`、`tracing-subscriber` 等专属 package 缺席；有专属 package 的单能力入口只允许自己的 package 出现；默认入口断言三项默认第三方 capability 与 `tracing-subscriber` 均存在。`settings-serde`、Windows Agent 与 Linux Agent 三个入口都要求共享的 `serde_json` 存在，但设置入口另要求直接 `serde` package 与 `settings-serde` feature，两个 Agent 入口要求 `agent-control` feature 并禁止设置 feature 合并；Agent 禁用入口同时要求 `serde_json` 与公开 builder 方法缺席。无专属 package 的 `charts` 场景直接断言 `uix` resolve 节点选择根 feature，并以 `BarChart + Gauge` 正反导入证明基础/高级图表、patch 与 snapshot 源码边界同步收缩；无专属 package 的 `table` 场景同样绑定根 feature，并以 `Table + DataTable` 正反导入证明表格实现、patch、snapshot、render handler 与动态树分派边界同步收缩；无专属 package 的 `navigation` 场景绑定根 feature，并以 `Breadcrumb + Navigation<u8>` 正反导入证明组件族、patch、snapshot、无障碍转换与导航兄弟联动边界同步收缩；无专属 package 的 `feedback` 场景绑定根 feature，并以 `Alert + Modal + message() + notify()` 正反导入证明组件族、全局门面、patch、snapshot、应用通知浮层与 Modal 树钩子同步收缩，同时保持 `TriggerMode`、`TooltipPlacement` 和 Slider 共用提示气泡原语属于基础层；无专属 package 的 `tree-widgets` 场景绑定根 feature，并以 `Tree + TreeNode + TreeSelect + DropPosition + SnapshotTreeNode` 正反导入证明树组件、patch、snapshot、无障碍与兼容重导出同步收缩，同时保持 `AccessibilityRole::Tree`、虚拟滚动原语和基础 `tree!` 宏可用。全部场景都必须对 `rich-text`、`charts`、`table`、`navigation`、`feedback`、`tree-widgets` 六项纯源码 capability 给出正向或负向断言，防止新增入口静默合并它们。schema v6 继续执行 package、既有 package feature 与 `uix` feature 正反断言，并为跨目标 release 记录 linker 环境变量、实际覆盖与 `--version` 探针；D3D11 正向入口要求 `windows` package 的五项 Direct3D/DXGI API feature，D3D12 选择面要求六项声明 feature 且排除 D3D11 专属 API，Vulkan 选择面要求 `ash` 且排除全部 D3D API feature，Metal 选择面要求空依赖边界；三者的正向 release 只证明公开选择面与构建图，生产 registry 仍须单独满足薄 RHI 和目标平台运行门禁。OpenGL ES 正向入口要求 `glow` package 与 `opengles` 根 feature，禁用入口要求两者缺席。三十七个入口还统一断言已删除且无源码调用点的 `raw-window-handle` 不得重新进入 resolved graph；三十三个未启用 `image-codecs` 的入口另行断言已删除直接边的精确 package `bytemuck` 缺席，四个启用图片能力的入口则允许 `image` 自身合法传递引入它。两个演示日志场景直接绑定根清单的 `uix-demo` 目标并使用相同的 `d3d11 + image-codecs + qrcode + form-pattern + rich-text + charts + table + navigation + feedback + tree-widgets` 非日志基础组合：禁用场景要求 `image`、`qrcode`、`regex` 存在且 `tracing-subscriber` 缺席，启用场景再要求订阅器出现，从而分别证明订阅器启用与禁用时演示二进制都能 clean release build。所有入口最后执行 `cargo clean`，因此正常采集结束后不会保留 Rust `target` 构建产物；只有显式传入 `--keep-build-artifacts` 才会跳过后置清理。`--dry-run` 在没有 Rust 工具链时只展示命令，不写入基线证据。

# UIX Framework Demo GUI QA

## 结论

**Passed**。真实 `uix-demo --agent-control` 窗口完成“首页 → 框架能力 → English → 向下滚动”的应用控制场景；每次操作均等待对应 `presented_revision`，中文/英文空态、Provider 尺寸优先级、typed token 与统一空态都通过语义断言和像素取证。视觉 QA 无 P0/P1 缺陷。

## 环境

- 日期：2026-07-17（Asia/Shanghai）
- OS：Windows 11 专业版 10.0.26200（Build 26200）
- Rust：`rustc 1.96.0 (ac68faa20 2026-05-25)`
- D3D11 smoke：`cargo build --bin uix-demo` 后启动真实 `target/debug/uix-demo.exe`，由 WGC 精确窗口截图
- Provider acceptance：`cargo test --features agent-control,test-harness --test agent_gui_windows real_demo_switches_provider_locale_and_presents_framework_capabilities -- --ignored --nocapture`
- 控制面：UIX `uix.agent.v1` Agent Bridge；外部 `ai-computer-toolkit` 用于独立的 D3D11 启动、WGC 截图与受控关闭 smoke
- 图形路径：Provider acceptance 使用 Vulkan CPU raster / pixel upload，便于现有前景像素 oracle 读取；D3D11 GPU swapchain 由 WGC smoke 覆盖
- 账户：当前本机交互用户；无网络、凭据或生产数据操作

## Coverage

| 场景 | 预期 | 实际 | 状态 | 证据 |
|---|---|---|---|---|
| 启动与首页首帧 | demo 显示真实窗口，首页与新增“框架能力”导航可见 | D3D11 WGC smoke 与 Vulkan acceptance 首页均显示 12 页壳层和“框架能力”入口 | Passed | [`evidence/01-home-baseline.png`](evidence/01-home-baseline.png)、[`evidence/00-agent-home.png`](evidence/00-agent-home.png) |
| 框架页中文基线 | 进入“框架能力”，显示中文状态、空态与 Provider 样例 | `click_at` 导航后等待真实 present；中文状态、`暂无数据`、Large > Small 和 custom empty 语义断言通过 | Passed | [`evidence/02-framework-zh.png`](evidence/02-framework-zh.png) |
| LocaleProvider 切换 | 点击 `English`，状态与 Empty 文案共同更新 | Agent Bridge `invoke` 后等待真实 present；状态为 `English`，空态为 `No data` | Passed | [`evidence/03-framework-en.png`](evidence/03-framework-en.png) |
| 下半页能力 | 滚动后可见 ComponentOverrides、typed TokenPatch 与统一空态 | 语义 `scroll` 后等待真实 present；token button 与 `List 自定义空态` 可见性断言通过 | Passed | [`evidence/04-framework-lower.png`](evidence/04-framework-lower.png) |
| 受控清理 | 本轮 demo 正常退出并清理 discovery descriptor | 集成测试通过 `WM_CLOSE` 等待成功退出；外部 smoke 也通过 exact session 关闭 | Passed | 测试 structured result |
| 代码侧契约 | 页面重建、root reconcile 和所有 targets 保持通过 | demo 单测覆盖中文/英文子树、自定义空态和点击状态持久化 | Passed | `cargo test --features test-harness --all-targets` |

## Gates

- 真实窗口 Provider acceptance：1 passed。
- 快速框架页契约：2 passed。
- `cargo test --features test-harness --all-targets`：lib 2082 passed / 15 ignored；demo 31 passed；其余 targets 全通过。
- `cargo fmt --check` 与 `git diff --check`：通过。
- 四文档内容审计：`产品 / 使用 / 架构 / 进度` 已准确描述这些已落地 Provider 能力，无需重复改写；通用 strict checker 只报告 `docs/使用指南/**/*.md`，该目录是仓库 `AGENTS.md` 明确允许的项目特例。

## Findings

### VIS-001 — Empty emoji 在当前字体后端显示为缺字方框

- 严重度：P1，已修复
- 发现方式：真实窗口中文/English 截图视觉 QA
- 修复：demo 从 `Empty::image("default")` 改为公开 API `Empty::icon("inbox")`，使用框架内置 Lucide 字体
- 回归：同一真实窗口 acceptance 重拍后图标完整，中英文状态均通过

### CTRL-001 — 外部 toolkit 的 foreground pointer 被 Windows 拒绝

- 严重度：Low，非阻塞工具限制
- 现象：外部 `ui.input.pointer@1` 两次在发送输入前返回 `FOREGROUND_ACTIVATION_FAILED`
- 处置：没有继续重试同类输入；改由产品内置 Agent Bridge 对真实窗口执行稳定语义操作，并独立等待 `presented_revision`
- 影响：不再阻塞 Provider 场景结论；外部 toolkit 仍成功完成无焦点 WGC 截图和 exact-session 关闭

## Visual QA

| 轴 | Verdict | 说明 |
|---|---|---|
| 需求符合 | pass | 四类框架能力均有真实可观察样例 |
| 主题/审美适配 | pass | 延续现有 UIX demo 视觉系统 |
| 层级与精致度 | pass | 分区、状态和说明层级清晰；缺字图标已修复 |
| 可用性/可访问性 | pass | 选中、disabled、语言状态不只依赖颜色；关键控件尺寸稳定 |
| 领域清晰度 | pass | Provider 继承、构造覆盖、token 与空态职责明确分组 |

## Residual Scope

- 本报告只覆盖当前 Windows 真实桌面，不外推 macOS / Wayland。
- D3D11 WGC smoke 与 Vulkan Provider acceptance 是互补证据，不宣称已做跨 GPU 厂商矩阵。
- 外部 toolkit 的 foreground pointer 限制仍保留为控制环境记录，但不属于 UIX 产品缺陷。

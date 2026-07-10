# UIX 项目总设计

← [index](index.md) · 需求 → [requirements.md](requirements.md) · 架构导航 → [areas/architecture.md](areas/architecture.md)

本文是跨系统的总设计与追踪入口；系统细节仍由 `docs/areas/systems/` 叶子文档负责，持久取舍仍以 [decisions.md](decisions.md) 为准。

## Design Goal

在“**用最少资源，做最好效果**”（[#105](decisions.md#d105)）下，为 UIX 建立一条从应用作者声明式代码到原生窗口、按需运行时和可组合渲染后端的单一主路径，并使 Windows 当前交付与桌面/移动长期目标共享同一套上层架构。

本设计优先优化：正确的域边界、空闲零工作、变化窄处理、应用作者低维护、平台可替换、失败可诊断、实现可验证。它不以绝对“最优”为目标，而以 [project.md](project.md) 的范围、[#166–#169](decisions.md#d166) 的决策和当前 P6 约束为准。

## Design Metadata

| 字段 | 值 | 说明 |
|------|----|------|
| Mode | Supplement | 在既有 #1–#169、14 个核心系统设计与 P6 图形专项上补充项目级端到端设计、需求追踪和评审基线，不替换系统叶子文档。 |
| Maturity | Executable | P0–P5 主体与 P6 部分能力已有实现证据；P6 后续、跨平台真机和 P7+ 仍按计划推进。 |
| Baseline Date | 2026-07-10 | 本次审计与补充日期。 |
| Authority | AGENTS/#105 → latest unsuperseded decisions → role-specific docs | `requirements` 定义目标，`design` / system docs 定义方案，`plan` / `implementation` 记录执行与证据；角色冲突必须先对齐，禁止就近选文档。 |

## Inputs And Constraints

| 输入 / 约束 | 来源 | 对设计的影响 |
|-------------|------|--------------|
| 跨平台（桌面 + 移动）全栈 App 框架 | [#166](decisions.md#d166) | 总设计必须覆盖 UI、运行时、客户端数据与平台扩展边界；移动端是目标而非当前已交付。 |
| Windows 优先 | [#167](decisions.md#d167) | 当前生产验收先在 Windows 闭环；不得把 Windows 细节泄漏到上层。 |
| 用最少资源，做最好效果 | [#105](decisions.md#d105) | 主循环、Timer、动画、事件、布局和渲染全部由显式触发与失效驱动。 |
| 六域单向依赖与 OS 隔离 | [AGENTS.md](../AGENTS.md#架构硬约束) | `native` 封装平台差异，上层只见 trait；跨域通信使用正交接口。 |
| 可组合组件模型 | [#168](decisions.md#d168) | 以 capability trait + registry 组合能力，不使用 bundled 枚举表达架构。 |
| 正交渲染轴 | [#169](decisions.md#d169) | 初始化装配由 `RasterMode × PresentMode × GraphicsBackend` 决定。 |
| 合法组合与 API identity 边界 | [#172](decisions.md#d172) | “×”不是完整笛卡尔积；caps/registry 裁决合法组合，具体 API identity 只进入配置、诊断、native 候选表与 draw adapter registry。 |
| 平台可选能力 Result-only | [#170](decisions.md#d170) | 可失败窗口能力公开返回 `Result`；不保留仅记日志的 void 兼容面。 |
| 可执行边界与 L1 证据 | [#171](decisions.md#d171)、[#173](decisions.md#d173) | 直接依赖/cfg 守卫必须与文档同强度；零闲置验收记录 wake 与 active-frame 证据。 |
| 文档是设计权威，不保留兼容层 | [AGENTS.md](../AGENTS.md) | 实现与文档冲突时重构实现；旧命名和旧分派不保留并行入口。 |
| 当前资源边界 | [project.md](project.md#范围) | 无内置网络/同步/服务端；Settings 仅显式 opt-in；移动端和部分原生光栅延后。 |

## Assumptions

| 假设 | 置信度 | 验证方式 |
|------|--------|----------|
| Windows 是 P6 的主开发、回归和生产验收环境。 | 高 | [plan.md](plan.md) 与 #167；Windows demo/测试矩阵。 |
| Linux/macOS backend 的“已编码”不等于生产 parity 或真机验收完成。 | 高 | 分平台编译、Linux 运行、macOS 真机验收；未验证前保持 backlog 状态。 |
| 现有系统叶子文档可继续承载细节，总设计只维护跨系统契约和追踪。 | 高 | 文档可达性检查与变更评审。 |
| 网络、同步与凭据管理不在当前 `data` 范围，扩展前需要新的产品边界决策。 | 高 | [plan.md · 开放问题](plan.md#开放问题)；新增 #175+ 决策。 |
| 移动端可复用上层域，但事件循环、窗口/surface 和应用生命周期仍需独立 backend 设计验证。 | 中 | P7 切片前做 iOS/Android spike 与 trait 缺口审计。 |

## Evaluation Criteria

| 准则 | 优先级 | 判定方式 |
|------|--------|----------|
| 架构正确性 | 最高 | 无反向依赖、无上层平台细节、无第二套并行分派。 |
| 资源效率 | 最高 | DeepIdle 真阻塞；无无效帧；布局、绘制和 present 受失效与 damage 约束。 |
| 应用作者体验 | 高 | 推荐入口稳定；常见能力由 View/State/App API 完成；无需手工 registry 或主循环。 |
| 可组合与可扩展 | 高 | 新组件、平台或图形 API 通过 trait/registry 增量接入，不修改无关上层域。 |
| 可靠性与可诊断 | 高 | 可选能力返回 `Result`；初始化失败有 probe 报告与确定回退；生命周期可清理。 |
| 可测试性 | 高 | FakePlatform/TestClock/语义断言/paint snapshot 可验证关键路径。 |
| 交付风险 | 高 | P6 先闭合 Windows 阻塞项，再做 parity；跨平台与移动端不提前扩散上层分支。 |
| 安全与隐私边界 | 中 | 无隐式网络；Settings 显式 opt-in；未提供的凭据保护能力必须明确。 |

## Requirement Traceability

| 需求 / 目标 | 设计元素 | 计划 / 推进步骤 | 验证 | 状态 |
|-------------|----------|-----------------|------|------|
| REQ-001 声明式应用入口 | prelude → View/State → Reconciler → WidgetTree | P0–P5 维护；公开 API 变更同步文档 | 示例构建、View/Reconciler 测试 | 基线已实现 |
| REQ-002 原生端到端 UI | native 事件 → SystemEvent → layout/render → present | P6 Windows 生产闭环 | demo 冒烟、事件/布局/绘制测试 | 主体已实现；ScrollView 三方向与动态展开回归已转绿 |
| REQ-003 零闲置与窄工作 | 三态主循环、ActiveWorkRegistry、InvalidationQueue、damage | P0–P5 维护；P6 回归 | DeepIdle、Timer、失效、present 测试 | 主体已实现 |
| REQ-004 框架托管活动工作 | Timer、Animation、MainThreadQueue、WindowSession/AppRuntime | P0–P5 维护；多窗回归 | TestClock、关闭清理、路由测试 | 主体已实现 |
| REQ-005 跨平台目标 / Windows 优先 | `native::traits` + backend/factory；上层无平台 cfg | P6 Windows → Linux/macOS parity → P7 移动 | 边界扫描、分平台编译与真机验收 | 部分满足 |
| REQ-006 图形选择与回退 | bootstrap 单 probe + 正交轴 + registry + SoftwareEngine | P6.8 与后续原生 raster | factory/bootstrap/backend 错误路径测试 | 部分满足 |
| REQ-007 Settings opt-in | `data::SettingsService` + App 显式配置 | P6 API/错误行为回归 | 无配置无 IO；load/save `Result` 测试 | 基线已实现 |
| REQ-008 分层验证 | FakePlatform、FakeTimer/TestClock、语义与 paint snapshot | 每个阶段按影响扩大验证 | 相关测试、平台 smoke、文档检查 | 持续 |

## Chosen Design

### 1. 产品与使用路径

UIX 对应用作者暴露一条推荐路径：

1. 从 `uix::prelude::*` 构建 App、View、State、布局和常用组件。
2. App builder 注入窗口配置、主题、Settings、`on_start` 等显式能力。
3. View build/reconcile 生成或更新 WidgetTree；业务回调留在 HandlerTable/View 绑定，不进入组件 struct。
4. 平台事件映射为 SystemEvent，再产生语义事件、状态变化或分级失效。
5. 仅有 pending 工作时执行 reconcile → layout → paint/composite → present；完成后回到等待。

平台贡献者只实现 `native::traits`、graphics peer 与 factory registry 条目；UI 组件作者只实现需要的 capability；两者都不修改主循环的公共语义。

### 2. 六域架构与边界

canonical 直接依赖集合由 [AGENTS.md](../AGENTS.md#架构硬约束) 定义；`A → B` 表示 A 可直接依赖 B：

| 域 | 允许的直接依赖 | 责任 | 禁止 |
|----|----------------|------|------|
| `core` | 无 | 几何、ID、错误、日志、damage 基础类型 | 依赖其他功能域 |
| `native` | `core` | OS 窗口/事件/系统能力、present/graphics trait、backend/factory | 把平台类型泄漏到上层 |
| `draw` | `core`, `native` | 光栅、display list、合成、失效、damage、engine 装配 | 依赖 `ui`/`app`/`data`；按 OS 分支 |
| `ui` | `core`, `native`, `draw` | View、组件、布局、事件、主题、浮层、响应式状态 | 依赖 `app`/`data`；访问 native backend；把业务回调存入 widget |
| `data` | `core` | 显式配置持久化基线 | 依赖 `native`/`draw`/`ui`/`app`；参与 UI 热路径 |
| `app` | `core`, `native`, `draw`, `ui`, `data` | 启动、主循环、多窗、事件映射、ScenePaint 桥接、主题和 Settings 注入 | 把平台细节传回 UI；固定轮询保活 |

跨域交互使用稳定 trait、只读桥或表驱动 registry；不得建立旁路或兼容分派。

### 3. 可组合组件模型

- UI widget 按 Layout、Render、Event、Lifecycle、Animation 等能力按需实现，WidgetTree 负责托管与组合。
- 布局以 pass-local `measure_children → LayoutChild → layout_children` 两阶段处理，不跨收敛轮缓存（[#174](decisions.md#d174)）；事件以平台、系统、语义三层隔离。
- 平台以 `Platform` 聚合子 trait；可选能力用 `Result` 表达，而不是让上层识别具体 OS。
- 渲染以 `RasterMode × PresentMode × GraphicsBackend` 独立描述能力；registry 行与 caps 共同给出**合法稀疏组合**，engine 是合法装配结果而不是架构枚举（[#172](decisions.md#d172)）。

### 4. 运行时状态机与单帧生命周期

| 状态 | 进入条件 | 允许工作 | 退出条件 |
|------|----------|----------|----------|
| DeepIdle | 无事件、无失效、无到期/注册活动工作 | 阻塞等待；不 layout/render/present/Effect tick | OS 事件、waker、deadline 到期 |
| RegisteredActive | 有 Animation/Timer/IME 等已登记工作，但尚未到期 | 以最近 deadline 单次 `wait_timeout`；不执行 frame 工作 | 到期后目标 session 进入 Active，或全部注销后回 DeepIdle |
| Active | 有 UiEvent、queue、State/reconcile 或 invalidation pending | 按固定帧序执行必要阶段 | pending 清空后回到前两态 |

Active 帧的项目级顺序为：事件映射与派发 → 到期 Timer / 主线程队列 → State/reconcile 合并 → 必要 layout → update/paint/composite → damage present → 清理与重新计算 deadline。细节以 [application](areas/systems/application.md#主循环) 与 [demand-driven](areas/systems/demand-driven.md#主循环状态机) 为准。

### 5. 图形 bootstrap 与回退

1. `native::factory` 提供已编译候选和单条目 context 创建，不自行循环 probe。
2. `draw::bootstrap_graphics_engine` 是唯一候选迭代点，在初始化期读取 registry 顺序。
3. context caps 给出 `GraphicsBackend`、`RasterMode`、`PresentMode` 与 present 能力。
4. `create_graphics_engine` 只按 caps + registry 装配合法 engine；`GpuNative × Swapchain` 与 `Cpu × PixelUpload` 是当前主组合，非法组合返回带候选/轴信息的诊断。
5. 候选未编译、context 初始化失败、组合非法或 engine 创建失败都必须记录原因；已创建资源须关闭，然后继续下一候选；全部 GPU 候选失败后由 app 建立 `SoftwareEngine + IPresenter`。
6. 初始化完成后不逐帧探测、不热切换 API；运行期只执行选定组合。

### 6. 状态、数据与所有权

| 状态 / 数据 | 所有者 | 生命周期 | 变更传播 |
|-------------|--------|----------|----------|
| View factory / pending root | WindowSession / AppRuntime | 单窗口 session | 帧内合并后 reconcile |
| `State<T>` / Computed / Effect | UI 响应式层 | clone 共享；按 bind site 登记 | 窄标脏并唤醒相关窗口 |
| WidgetTree / HandlerTable | WindowSession | 窗口创建到关闭 | reconcile、事件和 lifecycle 托管 |
| ActiveWorkRegistry / Timer / MainThreadQueue | WindowSession/AppRuntime | 注册到 cancel/完成/关窗 | deadline 或 waker 触发 Active |
| InvalidationQueue / DisplayList / LayerTree | draw/session | 帧间缓存 | Layout/Paint/Composite 分级更新 |
| AppState / ComponentHandle snapshot | App/UI registry | mount 到 unmount/关窗 | 只读 snapshot、invalidate、semantic emit |
| Settings KV | `data::SettingsService` | 应用显式 load/save | 不自动进入 UI 热路径 |

### 7. 接口与扩展点

| 接口面 | 使用者 | 稳定契约 |
|--------|--------|----------|
| `uix::prelude::*` | 应用作者 | App、View、State、常用组件、事件、handle、布局类型 |
| `GraphicsBackend` | 应用配置与诊断 | 具体 API identity 可用于显式选择、环境/Settings、报告，以及 native/draw 两个表驱动 registry 的 adapter 配对；其他上层代码不得据此分支 |
| `native::traits` | 平台/backend 实现与上层桥接 | 窗口、事件、系统、present、graphics 能力；上层禁止 backend 导入 |
| ScenePaint / GraphicsEngine / Canvas2D | app ↔ draw ↔ UI 场景 | 只读场景、单帧编排与绘制能力，不暴露 widget/backend 细节 |
| capability traits / registries | 组件与后端扩展者 | 按需实现、表驱动选择、无第二套分派 |
| AppHandle / ComponentHandle | 应用运行期控制 | 窗口作用域路由、自动清理、只读 snapshot 与显式失效/事件 |
| SettingsService | 应用与数据层 | 扁平 KV、显式 IO、错误经 `Result` 返回 |

### 8. 质量、运维与可观测性

- **性能**：以 L0/L1/L2、deadline、damage 和无固定轮询作为设计门槛；性能优化不能破坏正确效果。
- **可靠性**：可选能力与初始化失败显式返回；图形有确定回退；窗口关闭清理 Timer、queue 和 session 注册项。
- **可观测性**：保留默认日志、fatal crash log、backend/probe 诊断和错误 Toast；生产阻塞项必须可复现与定位。
- **测试**：纯逻辑优先单元测试，运行时使用 FakePlatform/TestClock，视觉输出使用 paint snapshot，平台交付增加真实 backend smoke；L1 必须记录 blocking wait、timeout、wake 来源、`tick_effects`、reconcile 与每窗 active-frame 次数。
- **安全/隐私**：当前无隐式网络；本地 Settings 只在应用显式配置时读写；凭据加密/保险库不是已提供能力。
- **文档运维**：需求变化先更新 requirements；跨系统取舍进入 design/decisions；执行状态进入 plan/implementation；细节留在最近的 system 叶子。

### 9. 交付边界

- P6 首先闭合 Windows 稳定性、D3D11 原生光栅剩余能力、demo/docs 准确性和生产阻塞项。
- Windows 基线达标后做 Linux/macOS parity 与 macOS 真机验收，上层域保持不变。
- P7+ 才启动移动端 backend；开始前先验证事件循环、surface、生命周期与输入 trait 是否足够。
- 网络/同步等全栈扩展必须先决定产品边界和域归属，不直接堆入现有 Settings 热路径。

## Alternatives Considered

| 方案 | 优点 | 缺点 | 结论 |
|------|------|------|------|
| 按需三态主循环 + registry/deadline | 空闲零工作；Timer/动画可统一托管；容易验证 | 状态和清理契约更严格 | Chosen（#105、#106、#115） |
| 固定 interval 帧循环/探活 | 实现直观；动画天然推进 | 空闲耗电、无效帧、破坏 L1；调用方易形成轮询 | Rejected |
| trait/registry 正交组件 | 能力可独立组合；新增 backend/组件影响局部 | 需要明确 caps、合法组合和诊断 | Chosen（#168、#169） |
| bundled pipeline/profile 枚举 | 初期分派简单 | 组合爆炸；混淆 API/光栅/present；形成第二套 mental model | Rejected，旧 `RenderPipelineProfile` 已删除 |
| 平台差异集中在 `native` | 上层同构；扩平台主要增加 backend | native trait 设计要求高，需处理能力差异 | Chosen（#167） |
| 上层按 OS/API 写 `#[cfg]` | 局部接入快 | 架构分叉、测试矩阵扩大、移动端难复用 | Rejected |
| 原生窗口/渲染 | 性能与系统能力可控；符合框架定位 | backend 与渲染工程成本高 | Chosen |
| WebView 作为主 UI 运行时 | 跨平台启动快、生态成熟 | 不符合原生渲染与按需管线定位，难复用现有 draw/ui | Rejected |

## Tradeoffs And Rationale

- 选择更严格的域边界与 breaking 重构，换取单一长期架构；因此不保留旧 API/旧分派兼容层。
- 选择 Windows 先行降低当前交付风险，但不允许把 Windows 专属条件带入上层；跨平台风险被延后到 backend parity 与真机验证。
- 选择自动托管 Picture、Registry、标脏和 handler 生命周期，增加框架内部复杂度，换取应用作者零维护与更可靠的资源效率。
- 选择初始化期静态候选 probe，而不支持运行中热切换或动态插件，换取确定性、资源生命周期可控和更小验证面。
- 选择单 crate 六域分层而非立即拆成多个包；边界由模块、trait、测试和文档约束，避免尚未必要的发布与依赖管理成本。

## Design Review Checklist

| 检查 | 状态 | 说明 |
|------|------|------|
| 目标与成功标准明确 | Pass | 已链接 project、#105 与 P6/P7 交付边界。 |
| 关键需求有追踪 | Pass | REQ-001–008 已映射设计、计划与验证。 |
| 约束与假设可见 | Pass | Windows 优先、移动端/网络边界、真机验证缺口已列出。 |
| 方案与取舍有记录 | Pass | 零闲置、正交组件、平台隔离、原生路径均与替代方案比较。 |
| 安全与隐私已考虑 | Pass with boundary | 当前无网络；Settings opt-in；凭据保护未声称已提供。 |
| 运维、可观测与支持已考虑 | Pass | 日志、probe 诊断、错误路径、平台 smoke 和文档维护已覆盖。 |
| 性能、成本与扩展性已考虑 | Pass | L0/L1/L2、damage、Windows 先行与 trait 扩展路径明确。 |
| rollout、验证与回退明确 | Pass | P6→parity→P7；GPU 失败回 SoftwareEngine；文档与测试门禁明确。 |
| 风险有缓解与责任人 | Pass | 见风险表；产品边界问题由主人决策。 |
| 开放问题有默认假设或 owner | Pass | 见 Open Questions。 |

## Detailed Areas

| 主题 | 详细设计 |
|------|----------|
| 架构与系统导航 | [areas/architecture.md](areas/architecture.md) |
| 实现状态与 backlog | [areas/implementation.md](areas/implementation.md) |
| 零闲置与主循环 | [demand-driven.md](areas/systems/demand-driven.md)、[application.md](areas/systems/application.md) |
| View、组件、布局、事件 | [view-reactive.md](areas/systems/view-reactive.md)、[component.md](areas/systems/component.md)、[layout.md](areas/systems/layout.md)、[event.md](areas/systems/event.md) |
| 渲染与图形后端 | [rendering.md](areas/systems/rendering.md)、[graphics-backend-pluggable.md](areas/systems/graphics-backend-pluggable.md) |
| 平台与数据 | [platform.md](areas/systems/platform.md)、[data.md](areas/systems/data.md) |
| 公开 API 与测试 | [public-api.md](areas/systems/public-api.md)、[testing.md](areas/systems/testing.md) |

## Rollout And Validation

当前证据（2026-07-10，基于当前工作树）：`cargo test --all-targets` 为 lib **1050/1050**、demo **19/19**；Windows default/no-default/all-features 与 Linux/macOS default/all-features `cargo check` 全部通过；`cargo clippy --all-targets`、严格文档检查通过。FrameRenderer 的版本 `0` 首帧与无 offscreen Picture 直绘回退已有自动化守卫，并经 SoftwareEngine GUI 首帧与按钮 `0→1` 复验。Windows D3D11 与无 GPU feature 的 SoftwareEngine 已完成首帧、颜色、按钮点击、最大化/恢复和关闭基础 GUI smoke；Win32 双真实窗口的独立状态、resize 事件路由与非全局退出已有自动化守卫。详见 [implementation · 当前验证基线](areas/implementation.md#当前验证基线-2026-07-10)。自动化/编译 gate 已恢复，但 OpenGL ES、输入/IME、主题、多窗口完整 GUI、硬件矩阵与 P0 生产阻塞清零仍未完成，因此 P6 尚未达到 `production`。

| 步骤 | 目的 | 验证 |
|------|------|------|
| 1. 保持 P0–P5 设计不回退 | 防止新功能破坏零闲置、多窗、响应式和 handle 体系 | 相关单测、架构边界、DeepIdle/Timer/TestClock 回归 |
| 2. 完成 P6 Windows 生产基线 | 闭合稳定性、原生 D3D11 raster、demo/docs 与错误诊断 | [P6 退出门槛](plan.md#p6-退出门槛默认工程-gate)：全量零失败、双路径 smoke、边界与文档 gate |
| 3. Linux/macOS parity | 验证 native 扩展不需要上层分叉 | 分平台编译、Linux 运行、macOS 真机生命周期/输入/present 验收 |
| 4. P7 移动端 spike 与切片 | 验证 traits 对移动生命周期、surface 和输入足够 | iOS/Android 最小窗口+事件+present 原型；缺口决策 #175+ |
| 5. 全栈扩展决策 | 明确网络/同步/服务端边界与域归属 | 产品需求、威胁模型、API/存储设计评审后再排期 |
| 6. 每次文档变更门禁 | 保持结构、链接、追踪和设计内容合格 | `check_project_docs.py <root> --strict-design` + 旧路径搜索 |

回退原则：代码切片应保持可独立验证；GPU 初始化失败回退 SoftwareEngine；文档设计被新证据否定时，通过新决策标明 supersede，不恢复并行旧方案。

## Risks And Mitigations

| 风险 | 影响 | 缓解 | Owner |
|------|------|------|-------|
| 文档与快速演进的源码漂移 | 错误实现状态和 API 指引 | 变更同步 implementation + 最近 system 叶子；CI/本地严格文档检查 | 项目维护者 |
| 状态符号把“代码存在”误读为“可交付” | 平台能力和里程碑被高估 | 统一 coded / automated / compiled / hardware / production 五级证据矩阵 | 项目维护者 |
| Windows 优先演化为上层 Windows 耦合 | Linux/macOS/移动端复用失败 | 平台 cfg 边界测试；上层只见 trait；评审拒绝 backend 导入 | 架构维护者 |
| 正交轴合法组合增长后诊断不足 | 初始化失败难定位 | registry 元数据、caps、probe report、非法组合测试 | draw/native 维护者 |
| 零闲置规则被 Timer/动画/Effect 旁路 | 空闲耗电与无效帧 | ActiveWorkRegistry 唯一登记；禁止裸循环；DeepIdle 计数回归 | app/draw 维护者 |
| macOS 与移动端缺少真实设备证据 | “可移植”停留在编译层 | P6 parity 后安排真机 gate；P7 先做最小 spike | 主人 / 平台维护者 |
| Settings 被误用为敏感凭据库 | 隐私与安全风险 | 文档明确能力边界；需要凭据时单独设计系统安全存储接口 | API 维护者 |
| P6 backlog 过宽 | 交付持续延后 | Windows 生产阻塞优先，按垂直切片验收，不并行扩散到远期功能 | 主人 / 项目维护者 |

## Open Questions

| 问题 | 为什么重要 | 默认假设 | Owner |
|------|------------|----------|-------|
| Windows “生产可用”是否还需产品级 SLA/兼容矩阵？ | 决定默认工程 gate 之外的发布承诺 | 先采用 [plan · P6 退出门槛](plan.md#p6-退出门槛默认工程-gate)；SLA、Windows 版本与硬件矩阵由主人补充 | 主人 |
| macOS 真机验收矩阵包含哪些版本、输入和 GPU 路径？ | 当前“已编码”不足以证明 parity | 先验收主流 macOS + AppKit/Metal CpuUpload 生命周期、输入、resize/present | 主人 / 平台维护者 |
| P7 首批 iOS 还是 Android？ | 影响事件循环、surface、图形 API 和工具链切片 | P6 完成前不默认选边；先做 trait 缺口审计 | 主人 |
| 网络/同步属于 `data` 扩展还是新域？ | 会改变依赖图、安全边界和公开 API | 当前不纳入；形成独立需求与 #175+ 决策后再设计 | 主人 |
| 屏幕阅读器桥的 v1 平台和验收范围？ | 影响 native accessibility trait 与组件语义映射 | 保持现有快照/键盘基线，平台桥暂不进入 P6 阻塞项 | 主人 |
| D3D11 复杂 path 与 Metal/D3D12 原生 raster 的排序？ | 决定 GPU 覆盖与跨平台投资 | 先完成 Windows 生产阻塞的 D3D11 缺口，再按 parity 价值排序 | draw/native 维护者 |

## Supplements

| 日期 | 变更 | 原因 | 后续 |
|------|------|------|------|
| 2026-07-10 | 新增项目级总设计、需求追踪、替代方案、质量属性、rollout、风险与开放问题。 | 原文档已有完整系统细节，但缺少跨系统 `design.md` 与独立需求 → 设计 → 验证链。 | 以 [plan.md](plan.md) 推进 P6；新事实同步 requirements/design/implementation，持久取舍写入 #175+。 |
| 2026-07-10 | 收敛依赖/cfg、Result-only、wake/L1、caps 合法组合和证据状态语义。 | 设计审查发现文档间存在可执行契约冲突，且部分守卫弱于文档。 | 以 [#170–#173](decisions.md#d170) 为统一裁决；实现、测试和系统叶子同步收敛。 |

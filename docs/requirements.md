# 项目需求基线

← [index](index.md) · 总设计 → [design.md](design.md)

本文提取产品与工程的稳定需求；解决方案取舍在 [design.md](design.md) 与 [decisions.md](decisions.md)，执行顺序在 [plan.md](plan.md)。

## Purpose

为 UIX 的产品愿景、架构硬约束、当前交付边界和验收方式建立统一追踪基线，避免系统文档只描述“怎么做”而无法回答“必须满足什么”。

## Sources

| 来源 | 日期 | 范围 | 说明 |
|------|------|------|------|
| [project.md](project.md) | 持续维护 | 产品愿景、范围、受众、成功标准 | canonical 项目简报 |
| [AGENTS.md](../AGENTS.md) | 持续维护 | #105、六域依赖、平台隔离、文档权威 | 编码与架构硬约束 |
| [decisions.md](decisions.md) #105、#166–#169 | 截至 2026-07-10 | 零闲置、产品定位、Windows 优先、可组合组件与渲染轴 | 持久设计决策 |
| [areas/implementation.md](areas/implementation.md) | 截至 2026-07-10 | 已实现能力与 backlog | 状态证据，不改变需求本身 |
| 2026-07-10 文档设计审查 | 2026-07-10 | 文档合格化、设计补充与契约收敛 | 审查记录；不把审查请求伪装成产品需求来源 |

状态采用证据级别而不是单一 `✅`：`designed`（契约明确）→ `coded`（代码存在）→ `automated`（自动测试通过）→ `compiled:<platform>` → `hardware:<platform>` → `production`。一项可同时拥有多个平台证据；缺少后续级别不得从前一级推断。

## Requirement Summary

| ID | 需求 | 类型 | 优先级 | 来源 | 当前状态 |
|----|------|------|--------|------|----------|
| REQ-001 | 应用作者能从 `uix::prelude::*` 以 View + State + 组合式组件声明原生应用，无需维护命令式 UI 树。 | Functional | Must | project、public-api | 已满足基线 |
| REQ-002 | 框架提供原生窗口、输入、布局、绘制、主题、浮层和语义事件的端到端路径，不以 WebView 作为运行时。 | Functional | Must | project、architecture | `coded + automated`；ScrollView 三方向与动态展开回归已转绿 |
| REQ-003 | 无事件、无失效、无已注册活动工作时，运行时进入阻塞等待；有变化时只执行必要阶段与最小脏区。 | Functional | Must | #105、demand-driven | 已满足主体 |
| REQ-004 | Timer、动画、跨线程 UI 投递、响应式更新和多窗路由由框架登记、唤醒与清理，调用方不维护轮询循环或裸 registry。 | Functional | Must | #115、#130–#150 | 已满足主体 |
| REQ-005 | 产品目标覆盖 Windows、Linux、macOS、iOS、Android；当前交付优先完成 Windows 生产可用，其他平台不得要求上层代码分叉。 | Product | Must | #166、#167、#171 | 桌面 `coded`；Windows `automated`；其他平台 compile/hardware 证据待补；移动端未实现 |
| REQ-006 | 图形 API 在初始化期按已编译能力选择并可诊断失败；候选失败时继续回退，终态可使用软件渲染，不进行逐帧探测或运行时热切换。 | Functional | Must | #162、#163、#169、#172 | 轴/registry `coded+automated`；后端与平台证据按矩阵维护 |
| REQ-007 | Settings 持久化必须显式 opt-in，不默认 load/save，也不进入 UI 热路径。 | Functional | Must | project、data | 已满足基线 |
| REQ-008 | 核心行为可在无真实窗口/GPU 的环境中验证，并对平台边界、主循环、事件、布局、绘制和公开 API 提供分层证据。 | Quality | Must | testing、AGENTS、#173 | `automated` 基线；真实平台证据持续扩充 |

## Acceptance Checks

| 需求 ID | 验收检查 | 证据 |
|---------|----------|------|
| REQ-001 | Counter/最小应用仅经 prelude、View、State 与公开 App API 完成；公开入口与示例可编译。 | `cargo test --bin uix-demo`；`prelude_exports_event_and_state_capture_types`；`demo_default_entrypoint_stays_on_prelude_app_path` |
| REQ-002 | 从平台事件到 SystemEvent/Handler，再到布局、绘制和 present 存在单一主路径；ScrollView Vertical/Horizontal/Both 动态内容不被 viewport cap 截断；不依赖 WebView。 | `collapse_expand_updates_scrollview_content_bounds` 及轴向回归；事件/绘制测试；Windows demo smoke |
| REQ-003 | DeepIdle 无 timeout 探活、active frame、layout/render/present/reconcile/Effect tick；Paint/Layout/Composite 分级失效只处理关联区域。 | `deep_idle_waits_without_fixed_timeout_or_extra_present`；`finished_animation_returns_to_deep_idle_without_timeout`；[#173](decisions.md#d173) 计数断言 |
| REQ-004 | Timer cancel、窗口关闭和 `post_to_ui`/semantic queue 路由均能清理注册项并回到 DeepIdle；多窗状态互不串路由。 | `run_interval_reschedules_until_handle_drops`；`close_session_drops_queued_work_and_timers`；`routed_post_to_ui_drains_target_queue`；`app_state_lookup_emit_drains_in_event_loop_and_returns_deep_idle` |
| REQ-005 | 除允许目录外无平台 `#[cfg]`；上层不导入 backend/graphics 实现；各新增平台只实现 trait/factory/platform adapter。 | `platform_cfgs_stay_inside_native_boundary`；`domain_dependencies_stay_layered`；分平台 `cargo check` 与 hardware matrix |
| REQ-006 | 每次启动只有 bootstrap 执行候选 probe；非法 caps 组合/初始化失败记录候选与原因、关闭资源并进入下一候选，终态 SoftwareEngine。 | `bootstrap_shuts_down_context_when_engine_creation_fails`；`bootstrap_shuts_down_context_when_engine_initialize_fails`；`gpu_native_swapchain_assembles_gpu_engine_for_d3d11` |
| REQ-007 | 未配置 Settings 时无隐式文件 IO；配置后 load/save 错误经 `Result` 暴露。 | `app_settings_is_opt_in`；`app_settings_load_registers_settings_service`；data settings tests |
| REQ-008 | FakePlatform、FakeTimer/TestClock、语义断言和 paint snapshot 能覆盖关键路径；跨模块变更扩大验证；证据记录命令、提交、平台和结果。 | [testing](areas/systems/testing.md)；`cargo test --all-targets`；严格文档检查；平台 smoke 记录 |

## Non-Functional Requirements

| ID | 属性 | 需求 | 验收检查 |
|----|------|------|----------|
| NFR-001 | 资源效率 | 遵循 L0 零像素、L1 零帧循环、L2 最小脏区；不得以固定 interval 保活。 | #105 审查 + DeepIdle/失效测试 |
| NFR-002 | 可移植性 | 平台差异只位于 `native` 允许目录；`core`/`draw`/`ui`/`app`/`data` 保持跨 OS 同构。 | #171 边界扫描 + 分平台 compile/hardware 证据 |
| NFR-003 | 可组合性 | 各域以正交 capability trait 与表驱动 registry 做受 caps 约束的组合；不引入 bundled 枚举、非法组合猜测或第二套并行分派。 | #168/#169/#172 审查 + registry/factory 错误路径测试 |
| NFR-004 | 可靠性 | 可选平台能力与初始化失败通过 `Result`/诊断报告表达；图形失败有确定回退，资源关闭可验证。 | 错误路径与 shutdown 测试 |
| NFR-005 | 可维护性 | 文档是设计权威；入口、索引、需求、设计、计划、决策、实现状态和系统叶子保持单一职责与可达性。 | 项目文档严格检查 + 文档审查 |
| NFR-006 | 可测试性 | 关键运行时不得只能依赖真实时钟、真实窗口或人工点击验证。 | FakePlatform/TestClock/语义与快照测试 |
| NFR-007 | 隐私与 IO | 框架当前不内置网络/同步；本地 Settings 仅显式启用，不把凭据保护能力默认为已提供。 | API 审查 + Settings 行为测试 |

## Out Of Scope

- 当前阶段不交付 iOS / Android backend；它们属于 P7+，但上层架构必须保持可承接。
- 当前 `data` 不包含 HTTP 客户端、同步、服务端或凭据保险库。
- 不支持运行中热切换图形 API、动态 `dlopen` 图形插件或同进程多窗口异构图形 API。
- 不为旧设计保留兼容层、别名或并行分派。
- 屏幕阅读器平台桥、D3D12/Metal 原生光栅和复杂 D3D11 path 仍按 [implementation · 后续工作](areas/implementation.md#后续工作) 管理。

## Requirement Changes

| 日期 | 需求 ID | 变更 | 原因 | 设计 / 计划影响 |
|------|---------|------|------|-----------------|
| 2026-07-10 | REQ-001–REQ-008、NFR-001–NFR-007 | 从现有项目简报、硬约束、决策和系统设计提取首版需求基线。 | 补齐需求 → 设计 → 计划 → 验证追踪。 | 新增 [design.md](design.md) 追踪矩阵；[plan.md](plan.md) 继续承载执行顺序。 |
| 2026-07-10 | REQ-002–REQ-006、REQ-008、NFR-002–NFR-003 | 补齐轴向 ScrollView、cfg 守卫、wake/L1、Result-only、caps 合法组合与证据等级。 | 设计审查发现原验收过于宽泛，无法阻止真实回归或弱守卫假绿。 | 同步 [#170–#173](decisions.md#d170)、系统设计、布局/架构测试与证据矩阵。 |

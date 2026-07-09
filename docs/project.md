# 项目简报

← [index](index.md)

## 产品愿景

**uix-app** 是一个 **跨平台（桌面 + 移动端）的全栈 App 开发框架**（[#166](decisions.md#d166)）。让 Rust 应用作者用 **Web 式声明 UI** 写出 **原生性能** 的应用，运行时遵循 **零闲置**（[#105](decisions.md#d105)）——有触发才工作，无 pending 则休眠。

| 层 | 目标 | 对应用作者意味着什么 |
|----|------|----------------------|
| **声明式 UI** | View + State + 布局 DSL，组合式组件 | 像写前端一样描述界面，无需手写 imperative 树维护 |
| **原生性能** | 原生窗口、事件、GPU/CPU 渲染管线 | 真原生应用，非 WebView；按需局部重绘 |
| **零闲置运行时** | 主循环三态、窄标脏、Registry 托管 | 空闲时不空转；动画/Timer 由框架 register，调用方零维护 |
| **全栈能力** | UI + 应用运行时 + 数据层（`data`） | 单 crate 内完成界面、生命周期、配置持久化；网络/同步等为远期扩展 |

**目标平台**（[#166](decisions.md#d166)）：桌面 **Windows / Linux / macOS** 与 **移动端（iOS / Android）** 均为对等目标；平台差异**仅**封装在 `native`，上层只见 trait。

推荐入口：`use uix::prelude::*;`。从愿景到首屏代码 → [public-api · 从目标到代码](areas/systems/public-api.md#从目标到代码) · [README Counter 示例](../README.md#示例)。

## 开发策略

**当前阶段**（[#167](decisions.md#d167)）：**Windows 优先** — 先把 Windows 端做到生产可用；Linux / macOS / 移动端按同一架构后续跟进。

| 原则 | 说明 |
|------|------|
| **平台层抹平差异** | 除 `native`（`backends/`、`graphics/`、`factory/`）外，`core` / `draw` / `ui` / `app` / `data` **代码完全一致**，不写 OS `#[cfg]` |
| **上层只见 trait** | `draw` / `ui` / `app` 只依赖 `native::traits`；禁止 `use native::backends::*` |
| **后续平台同构** | Linux / macOS / 移动端仅新增或完善 `native` backend；**不** fork 上层域 |
| **愿景不变** | [#166](decisions.md#d166) 跨平台全栈仍为 canonical 产品定义；开发优先级 ≠ 产品愿景 |

### 当前阶段 vs 目标愿景

| 维度 | 目标愿景 | 当前实现（诚实状态） |
|------|----------|----------------------|
| **跨平台** | 桌面 + 移动端，各 OS 对等 | **桌面三平台** backend 已编码；**移动端未实现**（P7+ backlog） |
| **开发优先级** | 各 OS 对等交付 | **Windows 优先**生产级打磨；Linux / macOS parity 与验证随后续（[#167](decisions.md#d167)） |
| **框架代码** | 除 `native` 外各 OS 共享同一套 | `core` / `draw` / `ui` / `app` / `data` **无平台分支**；差异仅在 `native`；各层 **正交组件可组装**（[#168](decisions.md#d168)） |
| **UI / 运行时** | 声明式 UI + 零闲置 App | P0–P5 主体已落地；P6 以 **Windows 生产可用** 为首要里程碑 |
| **全栈** | UI + `app` + `data`，远期可扩网络/同步 | **`data`** 仅 Settings KV 持久化（opt-in）；**无**内置服务端、HTTP 客户端或数据同步层 |
| **图形** | 多 API 可插拔 | Windows D3D11/WGL 为当前主验证路径；Linux / macOS 图形后端已接；native raster 为增强 backlog |

实现细节与 backlog → [implementation · 实现进度总览](areas/implementation.md#实现进度总览) · [后续工作](areas/implementation.md#后续工作)。

## 范围

| 范围内 | 范围外或暂缓 |
|--------|--------------|
| `core` 几何、错误、日志、标识与 damage 基础设施 | 不做应用级 i18n 系统 |
| `native` 平台 traits；**当前** Windows / Linux / macOS 后端与测试平台；**目标**含移动端 backend | 移动端 backend **未实现**（见 [implementation · 后续工作](areas/implementation.md#后续工作)）；屏幕阅读器平台桥待后续 |
| `draw` 渲染、失效、合成、GPU/软件引擎 | D3D12 native raster、WebGPU 仍为后续工作 |
| `ui` View、组件、布局、事件、主题、浮层 | — |
| `app` App builder、主循环、多窗、Timer、post_to_ui、handle | 不为旧设计保留兼容层或并行方案 |
| `data` Settings 持久化 opt-in 能力（全栈 **客户端数据层** 基线） | `data` 不参与 UI 热路径，不默认自动 load/save；**无**服务端/网络栈（远期扩展） |

## 受众

- 框架维护者：按文档权威推进实现与重构。
- 应用作者：从 `uix::prelude::*`、demo 和公开 API 文档了解使用方式。
- AI Agent：从 [AGENTS.md](../AGENTS.md) 进入，按 [areas/architecture.md](areas/architecture.md) 只读必要系统文档。

## 约束

架构硬约束与 #105 细则 → [AGENTS.md](../AGENTS.md) · [demand-driven · 设计美学](areas/systems/demand-driven.md#设计美学最高规则-105)。摘要：`docs` 与源码冲突时按文档重构；功能域依赖方向固定；平台差异仅在 `native`；公开 API 以 [public-api](areas/systems/public-api.md) 与 [glossary](glossary.md) 为准。

## 成功标准

- 主循环、渲染、事件和响应式状态满足 #105 零闲置约束。
- 文档如实区分 **产品愿景**（跨平台含移动端 + 全栈）、**开发优先级**（Windows 优先，[#167](decisions.md#d167)）与 **当前交付**（桌面、Windows 主验证）。
- 系统文档、[plan](plan.md)、[implementation](areas/implementation.md)、源码路径和公开 API 保持一致；不得用单一 `✅` 混淆 coded、automated、compiled、hardware 与 production 证据。
- 关键需求有编号，并能追踪到 [总设计](design.md)、执行步骤、精确测试/命令、验证提交和平台证据。
- 新工作能从 [index.md](index.md) → [areas/architecture.md](areas/architecture.md) 或分区索引 → 叶子文档定位。
- 局部改动有最小必要验证；跨模块、公共接口或发布前扩大验证。

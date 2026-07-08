# 项目简报

← [index](index.md)

## 项目目的

UIX 是一个 Rust 跨平台原生桌面 UI 框架，面向 Windows 与 Linux，推荐应用入口为 `use uix::prelude::*;`。项目目标是在原生窗口、事件、渲染、布局、组件、响应式状态和应用运行时之间建立清晰分层，同时保持按需工作、无闲置轮询的运行模型。

## 范围

| 范围内 | 范围外或暂缓 |
|--------|--------------|
| `core` 几何、错误、日志、标识与 damage 基础设施 | v1 不做应用级 i18n 系统 |
| `native` 平台 traits、Windows / Linux 后端与测试平台 | v1 不做无障碍扩展；无障碍 v2 见 backlog |
| `draw` 渲染、失效、合成、GPU/软件引擎 | macOS / Metal 仍为后续工作 |
| `ui` View、组件、布局、事件、主题、浮层 | D3D12、WebGPU 等高级图形路径仍为后续工作 |
| `app` App builder、主循环、多窗、Timer、post_to_ui、handle | 不为旧设计保留兼容层或并行方案 |
| `data` Settings 持久化 opt-in 能力 | `data` 不参与 UI 热路径，不默认自动 load/save |

## 受众

- 框架维护者：按文档权威推进实现与重构。
- 应用作者：从 `uix::prelude::*`、demo 和公开 API 文档了解使用方式。
- AI Agent：从 [AGENTS.md](../AGENTS.md) 进入，按 [areas/architecture.md](areas/architecture.md) 只读必要系统文档。

## 约束

- 最高规则 #105：用最少资源，做最好效果；有触发才工作，无 pending 则休眠。
- `docs` 与源码冲突时按文档重构，不保留兼容层。
- 功能域依赖方向固定：`core ← native ← draw ← ui ← app`，`data` 仅按约束参与。
- 平台差异只在 `src/native/backends/` 与 `src/native/factory.rs`；上层使用 trait + `Result`。
- 入口和公开 API 以 [systems/public-api.md](areas/systems/public-api.md) 与 [glossary.md](glossary.md) 为准。

## 成功标准

- 主循环、渲染、事件和响应式状态满足 #105 零闲置约束。
- 系统文档、roadmap、源码路径和公开 API 保持一致。
- 新工作能从 [index.md](index.md) → [areas/architecture.md](areas/architecture.md) 或分区索引 → 叶子文档定位。
- 局部改动有最小必要验证；跨模块、公共接口或发布前扩大验证。

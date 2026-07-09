# 系统文档索引

← [docs index](../../index.md) · [架构导航](../architecture.md)

本目录存放 UIX 的系统级设计文档。优先从 [architecture.md](../architecture.md) 按任务定位；本页是系统清单、功能域、摘要和关键决策的**唯一目录源**。

| # | 系统 | 文档 | 功能域 | 一句话 | 关键决策 |
|---|------|------|--------|----------|----------|
| 1 | 应用 | [application.md](application.md) | `app` | 启动、主循环、桥接 | #59 #64 #74 #88 #93 |
| 2 | View 与响应式 | [view-reactive.md](view-reactive.md) | `ui` | 声明式 UI、State、Reconciler | #21 #24 #31 #49 #60 |
| 3 | 组件 | [component.md](component.md) | `ui` | Widget、trait、生命周期 | #8 #20 #35 #58 #83 |
| 4 | 主题与样式 | [theme-style.md](theme-style.md) | `ui` / `draw` | Theme、Style/StyleSet | #1–#3 #9 #14–#17 |
| 5 | 事件 | [event.md](event.md) | `ui` / `app` | 三层事件、HandlerTable | #4–#7 #36 #68 |
| 6 | 布局 | [layout.md](layout.md) | `ui` | measure/arrange、Intrinsic、Flex/Grid、Scroll | #29 #38 #45 #53 #165 |
| 7 | 渲染 | [rendering.md](rendering.md) | `draw` / `app` | ScenePaint、合成、局部重绘、多图形 API | #59 #70 #82 #122 #129 #162–#164 #168 #169 #172 |
| 8 | 平台 | [platform.md](platform.md) | `native` | OS 隔离、UiEvent、可选能力 | #40 #59 #164 #170 #171 |
| 9 | 浮层 | [overlay.md](overlay.md) | `ui` | OverlayStack、Modal/菜单 | #96–#98 #100 |
| 10 | 基础设施 | [foundation.md](foundation.md) | `core` | 几何、错误、日志 | #38 #56 #70 |
| 11 | 数据 | [data.md](data.md) | `data` | Settings KV | #64 |
| 12 | 测试 | [testing.md](testing.md) | 跨域 | FakePlatform、语义断言、门禁 | #40 #171 #173 |
| 13 | 按需零闲置 | [demand-driven.md](demand-driven.md) | 跨域 | 核心理念 #105 | #105–#159 #173 |
| 14 | 公开 API | [public-api.md](public-api.md) | `prelude` | 应用作者入口与导出 | #69 #101 #170 #172 |
| — | 可插拔图形后端（**P6 专项**） | [graphics-backend-pluggable.md](graphics-backend-pluggable.md#可组合渲染轴) | `native` / `draw` / `app` | caps 约束的正交渲染轴、registry、API 对等实现 | #162–#164 #168 #169 #172 |

维护系统文档时，同步 [architecture.md](../architecture.md) 的任务路由和 [implementation.md](../implementation.md) 的实现状态。

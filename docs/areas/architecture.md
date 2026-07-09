# UIX 架构导航

> **纯索引**。硬约束 → [`AGENTS.md`](../../AGENTS.md)。**禁止通读** `docs/` — 按需打开链接即可。

---

<a id="ai-按需阅读省-token"></a>

## AI 按需阅读（省 token）

> 路径：`AGENTS.md`（必读）→ 本页定位 → 常规任务**仅**打开下列 1–3 篇；P6 图形专项最多 4 篇（优先锚点章节，勿通读整篇）。

| 任务 | 只读这些（顺序；常规 ≤3，P6 ≤4） |
|------|---------------------|
| 查项目目标 / 需求 / 跨系统方案 | [`project.md`](../project.md) → [`requirements.md`](../requirements.md) → [`design.md`](../design.md) |
| 查 API / 设计名 vs 源码名 | [`public-api.md`](systems/public-api.md) → [`glossary.md#术语对照`](../glossary.md#术语对照) → 相关 `#d{N}` in [`decisions.md`](../decisions.md) |
| 查实现是否已落地 | [`implementation.md#实现进度总览`](implementation.md#实现进度总览) → 对应系统 `> **实现注记**` |
| 改主循环 / 帧调度 / 三态 | [`demand-driven.md#主循环状态机`](systems/demand-driven.md#主循环状态机) → [`application.md#主循环`](systems/application.md#主循环) |
| 改标脏 / Picture / 局部重绘 | [`demand-driven.md#失效与窄标脏`](systems/demand-driven.md#失效与窄标脏) → [`rendering.md#管线与失效`](systems/rendering.md#管线与失效) |
| 新 App API（Timer/post_to_ui/多窗） | [`demand-driven.md#开发者契约零维护`](systems/demand-driven.md#开发者契约零维护) → [`application.md#app-定时-api`](systems/application.md#app-定时-api) |
| 写应用 / View | [`view-reactive.md`](systems/view-reactive.md) → [`theme-style.md`](systems/theme-style.md) → [`event.md#传播--handlertable`](systems/event.md#传播--handlertable) |
| 改内置组件 | [`component.md#能力`](systems/component.md#能力) → [`layout.md`](systems/layout.md) → [`event.md`](systems/event.md) |
| 改样式 / Theme | [`theme-style.md#style--styleset`](systems/theme-style.md#style--styleset) → [`theme-style.md#色板架构`](systems/theme-style.md#色板架构) |
| 改布局 / Scroll | [`layout.md`](systems/layout.md) → [`component.md`](systems/component.md) |
| 接平台 / 窗口 / 输入 | [`platform.md#traits-清单`](systems/platform.md#traits-清单) → [`platform.md#平台贡献指南`](systems/platform.md#平台贡献指南) → [`application.md`](systems/application.md) |
| 多图形 API / GPU 选型（**P6 专项**） | [`graphics-backend-pluggable.md#可组合渲染轴`](systems/graphics-backend-pluggable.md#可组合渲染轴) → [`rendering.md#多图形-api`](systems/rendering.md#多图形-api) → [implementation · P6.7](implementation.md#p67-图形后端架构) / [P6.8](implementation.md#p68-可组合渲染轴) → [#162](../decisions.md#d162) [#163](../decisions.md#d163) [#168](../decisions.md#d168) [#169](../decisions.md#d169) |
| Modal / Tooltip / 菜单 | [`overlay.md`](systems/overlay.md) → [`event.md`](systems/event.md) |
| 持久化 Settings | [`data.md`](systems/data.md) → [`application.md#appstate--多窗--settings`](systems/application.md#appstate--多窗--settings) |
| 写测试 | [`testing.md#fakeplatform`](systems/testing.md#fakeplatform) → [`event.md#测试`](systems/event.md#测试) |
| 改几何 / Damage / 日志 | [`foundation.md`](systems/foundation.md) |
| 新 API 设计审查 | [`demand-driven.md#新-api-审查清单`](systems/demand-driven.md#新-api-审查清单) → [#105](../decisions.md#d105) |

**人类读者**：首次了解 → [`project.md` · 产品愿景](../project.md#产品愿景)（含 [当前阶段 vs 目标](../project.md#当前阶段-vs-目标愿景)）→ [`design.md`](../design.md) → [`demand-driven.md#设计美学最高规则-105`](systems/demand-driven.md#设计美学最高规则-105) → [`application.md#启动`](systems/application.md#启动) → [`public-api.md`](systems/public-api.md)。非 P6 图形后端任务 **不必** 通读 [`graphics-backend-pluggable.md`](systems/graphics-backend-pluggable.md)。

---

<a id="功能域--系统"></a>

## 功能域 ↔ 系统

| 功能域 | 路径 | 主要系统文档 |
|--------|------|--------------|
| `core` | `src/core/` | [foundation](systems/foundation.md) |
| `native` | `src/native/` | [platform](systems/platform.md) |
| `draw` | `src/draw/` | [rendering](systems/rendering.md) |
| `ui` | `src/ui/` | [view-reactive](systems/view-reactive.md)、[component](systems/component.md)、[theme-style](systems/theme-style.md)、[event](systems/event.md)、[layout](systems/layout.md)、[overlay](systems/overlay.md) |
| `app` | `src/app/` | [application](systems/application.md) |
| `data` | `src/data/` | [data](systems/data.md) |
| *(跨域)* | — | [public-api](systems/public-api.md)、[testing](systems/testing.md)、[**demand-driven**](systems/demand-driven.md)（#105） |

依赖方向 → [`AGENTS.md` · 架构硬约束](../../AGENTS.md#架构硬约束)

---

## 系统索引

系统列表、功能域、摘要和关键决策只在 [`systems/index.md`](systems/index.md) 维护；本页只负责按任务和跨系统主题路由，避免双份目录漂移。

---

## 主题索引

跨系统话题 → 优先章节（按需深入）。

| 主题 | 主文档 · 章节 | 关联 |
|------|---------------|------|
| **按需零闲置** | [demand-driven · 开发者契约](systems/demand-driven.md#开发者契约零维护) | application, rendering, event |
| 主循环 / 帧调度 | [application · 主循环](systems/application.md#主循环) | demand-driven, rendering |
| Timer / post_to_ui | [application · App 定时](systems/application.md#app-定时-api) · [#132](../decisions.md#d132) | demand-driven |
| ActiveWorkRegistry / 多窗 | [demand-driven · Registry](systems/demand-driven.md#activeworkregistry) | application |
| View / Reconcile | [view-reactive](systems/view-reactive.md) | component, event |
| Widget trait | [component · 能力](systems/component.md#能力) | layout, event |
| Style / 色板 | [theme-style · Style](systems/theme-style.md#style--styleset) | component, rendering |
| 事件传播 | [event · 传播](systems/event.md#传播--handlertable) | view-reactive |
| measure / Scroll | [layout · 总览](systems/layout.md#布局设计总览) · [Measure/Arrange](systems/layout.md#measure--arrange-两阶段) | component |
| ScenePaint / 失效 | [rendering · 管线](systems/rendering.md#管线与失效) | foundation |
| **多图形 API / 可插拔后端** | [graphics-backend-pluggable · 可组合渲染轴](systems/graphics-backend-pluggable.md#可组合渲染轴) · [rendering · 多图形 API](systems/rendering.md#多图形-api) · [implementation · P6.7](implementation.md#p67-图形后端架构) / [P6.8](implementation.md#p68-可组合渲染轴) · [#162](../decisions.md#d162) [#163](../decisions.md#d163) [#168](../decisions.md#d168) [#169](../decisions.md#d169) | platform |
| **可组合组件模型** | [#168](../decisions.md#d168) · [#169](../decisions.md#d169) · [component · 能力](systems/component.md#能力) · [demand-driven · 分域要求](systems/demand-driven.md#分域要求) | 全域 |
| Platform traits | [platform · Traits](systems/platform.md#traits-清单) | application |
| Modal / Overlay | [overlay](systems/overlay.md) | event, component |
| Settings | [data](systems/data.md) | application, theme-style |
| 实现进度 / backlog | [implementation · 实现进度总览](implementation.md#实现进度总览) · [P6 生产级框架](implementation.md#p6-生产级框架) · [后续工作](implementation.md#后续工作)（含移动端 P7+、全栈扩展） | 各系统 · 实现注记 |
| 术语 / API 名 | [glossary · 术语对照](../glossary.md#术语对照) · [public-api](systems/public-api.md) | decisions #101–#104 |
| 源码目录 | [implementation · 源码目录详表](implementation.md#源码目录详表) | 各系统 |

---

## 参考索引

| 文档 | 用途 |
|------|------|
| [`AGENTS.md`](../../AGENTS.md) | 硬约束 + Agent checklist（编码前必读） |
| [`requirements.md`](../requirements.md) | 编号需求、优先级、状态与验收检查 |
| [`design.md`](../design.md) | 项目级端到端设计、追踪、质量属性、rollout 与风险 |
| [`implementation.md`](implementation.md) | 实现进度、backlog、源码目录详表 |
| [`decisions.md`](../decisions.md) | 决策台账 #1–#173；新决策 #174+ |
| [`glossary.md`](../glossary.md) | 术语；[设计名 vs 源码](../glossary.md#术语对照) |
| [`demo/README.md`](../../demo/README.md) | 演示程序模式与运行说明 |

---

## 维护约定

- 新系统 → 更新 [`systems/index.md`](systems/index.md) → 写 `systems/*.md` → 仅在需要新任务路由时更新本页。
- 新 `src/` 路径 → [implementation · 源码目录详表](implementation.md#源码目录详表) + 系统「源码模块」。
- 落地/差距 → [implementation · 实现进度总览](implementation.md#实现进度总览) + 系统 `> **实现注记**`。
- 跨系统主题 → 「主题索引」+ 最相关系统章节。
- 新术语 → [`glossary.md`](../glossary.md)；取舍 → [`decisions.md`](../decisions.md) #174+。

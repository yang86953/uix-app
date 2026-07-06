# UIX 框架设计文档 — 总目录

> 入口文档。设计按**系统**组织，不按源码文件夹。  
> 维护规则 → [`AGENTS.md`](../AGENTS.md)

---

## 文档分层

| 层级 | 位置 | 内容 |
|------|------|------|
| 总目录 | **本文件** | 系统划分、文件说明、阅读顺序 |
| 设计规范 | [`systems/`](systems/) | 各系统的定稿行为、边界与跨系统关系 |
| 决策台账 | [`decisions.md`](decisions.md) | 100 项已定稿选择；冲突以废止关系为准 |
| 术语表 | [`glossary.md`](glossary.md) | 核心名词 |

---

## 系统全景

框架由 **11 个系统** + **1 份决策台账** 描述。系统之间按运行时数据流协作，而非按仓库目录一一对应。

| 序号 | 系统 | 文档 | 一句话 |
|------|------|------|--------|
| 1 | 应用系统 | [systems/application.md](systems/application.md) | 启动、主循环、多窗、AppState、引擎与主题调度 |
| 2 | View 与响应式系统 | [systems/view-reactive.md](systems/view-reactive.md) | 声明式 UI、State/Computed/Effect、View diff |
| 3 | 组件系统 | [systems/component.md](systems/component.md) | 组件模型、能力拆分、生命周期、authoring |
| 4 | 主题与样式系统 | [systems/theme-style.md](systems/theme-style.md) | 色板、Typography、StyleSet、预设 |
| 5 | 事件系统 | [systems/event.md](systems/event.md) | 三层事件、HandlerTable、传播与绑定 |
| 6 | 布局系统 | [systems/layout.md](systems/layout.md) | measure、Flex/Grid、盒模型 |
| 7 | 渲染系统 | [systems/rendering.md](systems/rendering.md) | 引擎、失效、合成、离屏缓存 |
| 8 | 平台系统 | [systems/platform.md](systems/platform.md) | OS 抽象、输入、呈现、HiDPI |
| 9 | 浮层系统 | [systems/overlay.md](systems/overlay.md) | Modal、Tooltip、菜单、OverlayStack |
| 10 | 基础设施系统 | [systems/foundation.md](systems/foundation.md) | 错误、几何、日志、诊断 |
| 11 | 数据系统 | [systems/data.md](systems/data.md) | 配置持久化 |
| — | 决策台账 | [decisions.md](decisions.md) | 全部设计选择的唯一索引 |
| — | 术语表 | [glossary.md](glossary.md) | 统一名词解释 |

---

## 各文件说明

### Main.md（本文件）

设计文档总入口：系统列表、每个系统文档的职责说明、推荐阅读顺序、系统间协作关系。新增或重命名系统文档时**必须先更新本文件**。

### decisions.md

**唯一决策台账**（#1–#100）。记录「选了什么方案」、废止关系、实现顺序（#50）。不写长篇设计正文；细节在 `systems/` 各文档。新决策追加 #101+。

### glossary.md

核心术语表。系统文档新增术语时同步到这里；术语按定稿设计解释，不保留并行含义。

### systems/application.md — 应用系统

App 如何启动与运行：GUI/CLI 模式、主循环一帧顺序、GPU/CPU 引擎策略、ScenePaint 桥接职责、多窗口与共享 AppState/Theme、Settings 可选注入、Light/Dark 与 OS 主题同步。

**主要决策**：#55、#59、#64、#74、#76、#93–#94

### systems/view-reactive.md — View 与响应式系统

声明式 View 如何构建组件树；State 变更如何触发重建与细粒度 diff；key、handler 重新注册；Computed/Effect 边界。

**主要决策**：#21、#24、#31、#49、#60–#62、#78–#79

### systems/component.md — 组件系统

单个 UI 组件的设计：核心原则、struct 放什么、Layout/Render/Event/Lifecycle 拆分、生命周期阶段、AnimationRegistry（无 per-component tick）、Button v1 范围、Big Bang 落地、component 宏与 prelude。

**主要决策**：#8、#16、#20–#23、#29–#30、#58、#77、#83、#85

### systems/theme-style.md — 主题与样式系统

120+13 色板、NeutralRole、TypographyScale、ColorValue 规则、StyleSet 五态与优先级、品牌主题两档 API、主题切换 invalidate、使用侧改样式方式。

**主要决策**：#1–#3、#9、#11–#17、#27–#28、#34、#37、#39、#42、#47、#48、#51–#57、#63、#65、#74、#76、#81、#84

### systems/event.md — 事件系统

System / Semantic / Custom 三层；HandlerTable；传播、stop、preventDefault；when/once；剪贴板与 IME 相关语义；FileDrop、ContextMenu。

**主要决策**：#4–#7、#10、#25–#26、#36、#40、#61–#62、#66、#68、#71、#90–#92、#95、#98

### systems/layout.md — 布局系统

Constraints、measure、Flex + Grid、Style 上的对齐与轨道、Scroll 容器与 Wheel。

**主要决策**：#38、#41→#53、#45、#73、#81、#84

### systems/rendering.md — 渲染系统

双引擎与 Idle 帧、失效类型、ScenePaint 边界、层树与 DisplayList、深度≥4 自动 Picture 缓存、绘制上下文能力。

**主要决策**：#59、#82、#85–#87

### systems/platform.md — 平台系统

Platform 契约、窗口与 CPU/GPU 呈现、UiEvent 到 SystemEvent 的应用边界映射、平台支持矩阵、FakePlatform 测试。

**主要决策**：#59、#70、#71

### systems/overlay.md — 浮层系统

OverlayStack 统一管理 Modal（focus trap）、Tooltip（延迟显示）、右键 ContextMenu；与事件系统、主题共享的关系。

**主要决策**：#96–#97、#98、#100

### systems/foundation.md — 基础设施系统

错误、几何、日志、诊断；全框架共用约束；Fail Fast。

**主要决策**：（被多系统引用）#38、#56、#70

### systems/data.md — 数据系统

SettingsService 模型、与 App 可选集成、主题/locale 持久化边界。

**主要决策**：#64

---

## 系统协作（运行时）

| 方向 | 关系 |
|------|------|
| 平台 → 应用 | 平台系统产出输入事件；应用系统映射并驱动主循环 |
| 应用 → View/组件 | 应用持有 AppState、Theme、多窗组件树 |
| View/响应式 → 组件 | View 构建与 reconciler 产出组件树 |
| 组件 → 布局 | 组件请求 measure；布局系统写回几何 |
| 主题/样式 → 渲染 | StyleSet 在绘制阶段经 Theme 解析 |
| 组件 → 事件 | dispatch 产生语义事件 → HandlerTable |
| 组件 → 渲染 | 经 ScenePaint 绘制；失效队列驱动局部重绘 |
| 渲染 → 平台 | present 上屏 |
| 浮层 → 应用/事件 | OverlayStack 平行于主树，共享 Theme 与 HandlerTable |
| 基础设施 | 横切所有系统 |
| 数据 | 侧向；App 可选加载持久化配置 |

---

## 推荐阅读顺序

1. 本文件
2. [`glossary.md`](glossary.md) — 统一术语  
3. [`decisions.md`](decisions.md) — 实现顺序 #50 与关键 #  
4. [application.md](systems/application.md) — 一帧怎么跑  
5. [view-reactive.md](systems/view-reactive.md) + [component.md](systems/component.md)  
6. [theme-style.md](systems/theme-style.md) + [event.md](systems/event.md)  
7. [layout.md](systems/layout.md) + [rendering.md](systems/rendering.md) + [platform.md](systems/platform.md)  
8. [overlay.md](systems/overlay.md) + [foundation.md](systems/foundation.md) + [data.md](systems/data.md)  

---

## 实现顺序（#50）

Theme/Style → Event/HandlerTable → Lifecycle → Button → 全量 Widget + demo。详见 [decisions.md](decisions.md)。

---

## 文档维护约定

| 场景 | 必须同步 |
|------|----------|
| 新增或重命名系统文档 | 更新本文件的系统全景、文件说明和阅读顺序 |
| 新增设计选择 | 在 [`decisions.md`](decisions.md) 追加 #101+，并在相关系统文档引用 |
| 决策发生冲突 | 在 `decisions.md` 的废止关系中说明，以较新的决策为准 |
| 源码边界或数据流变化 | 更新相关 `docs/systems/*.md`；若涉及硬约束，同步 [`AGENTS.md`](../AGENTS.md) |
| 架构硬约束变化 | 更新 [`AGENTS.md`](../AGENTS.md)，并在相关文档只做链接引用 |
| 新增或重命名术语 | 更新 [`glossary.md`](glossary.md)，并检查系统文档是否用词一致 |

文档分工原则：`docs/systems/` 解释“为什么这样设计”和“系统边界”；`AGENTS.md` 记录协作规则、功能域和硬约束；`README.md` 面向第一次上手。

---

## 系统文档模板

系统文档建议保持以下结构：

1. 职责与边界
2. 输入 / 输出
3. 核心规则或不变量
4. 与其他系统的关系
5. 落地要求
6. 相关决策

---

## 与源码功能域（仅供实现对照）

设计按**系统**写；代码按 **core / native / draw / ui / app / data** 分域实现。功能域边界和跨平台硬约束见 [`AGENTS.md`](../AGENTS.md)，系统文档不维护长源码清单。

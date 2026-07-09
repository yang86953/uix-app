# 实现落地计划

← [architecture](architecture.md) · 决策 [#154](../decisions.md#d154) [#157](../decisions.md#d157)

> **P0–P5 主体已落地**。本文保留 **阶段摘要**、**实现进度**、**后续 backlog** 与 **源码目录详表**；刻意保留或未决项边界 → [demand-driven · 剩余差距](systems/demand-driven.md#剩余差距)。

## 索引

| 章节 | 说明 |
|------|------|
| [已落地阶段（P0–P5）](#已落地阶段p0p5) | 分阶段目标与验收摘要 |
| [实现进度总览](#实现进度总览) | 已落地能力按域汇总 |
| [后续工作](#后续工作) | 未实现 backlog（权威清单） |
| [P6 生产级框架](#p6-生产级框架) | 生产级优先项、图形后端里程碑、P6.8 可组合渲染轴 |
| [源码目录详表](#源码目录详表) | `src/` 路径与系统文档映射 |
| [维护](#维护) | 文档同步约定 |

**关联**：[demand-driven](systems/demand-driven.md) · [application](systems/application.md) · [view-reactive](systems/view-reactive.md) · [decisions](../decisions.md)

---

<a id="分阶段路线图"></a>
<a id="已落地阶段p0p5"></a>

## 已落地阶段（P0–P5）

设计（#154）— 下列阶段 **主体接线已完成**；按域能力清单见 [实现进度总览 · 已实现](#已实现按域)。

```text
P0 基础零闲置          P1 热更新           P2 App 运行时 API      P3 渲染窄路径
├─ 三态主循环 #106     ├─ reconcile 接入   ├─ Timer #132          ├─ PicturePolicy #122
├─ Registry #115       ├─ 帧内合并 #118    ├─ post_to_ui #133     ├─ Composite #107
├─ wait_until #127     ├─ reconcile合并#153├─ MainThreadQueue #137  └─ PointerMove #109
└─ Effect DeepIdle     ├─ handler重绑#123  ├─ on_start #140
                       └─ StateSlotId #143 └─ TestClock #139

P4 多窗                P5 组件 Handle 体系
├─ WindowSession #116  ├─ AppState #145
├─ open_window #148    ├─ Snapshot #146 #151 #152
├─ update_view #149    ├─ Handle emit #147
└─ State fan-out #150  └─ ComponentId #101

P6 图形后端            ← #162 #163，详见下文
├─ GraphicsBackend 枚举 + registry probe
├─ Windows D3D11/12
├─ Linux Vulkan
└─ macOS Metal + AppKit SoftwareEngine bootstrap
```

| 阶段 | 目标 | 关键决策 | 验收（已达成） |
|------|------|----------|----------------|
| **P0** | DeepIdle 真休眠 | #106 #115 #127 #105 | 无事件无 present；DeepIdle 不 `tick_effects` |
| **P1** | 热更新不上全量 build | #118 #153 #123 #143 | State 批次单次 reconcile；handler 智能重绑；显式 capture API 与 `semantic_handler!` 宏可生成 fingerprint，普通闭包仍保守重绑（#159/#160） |
| **P2** | App 公开定时/投递 | #132–#137 #140 #139 | Timer cancel 回 DeepIdle；TestClock 测 drain / wait_until |
| **P3** | L2 最小脏区 | #122 #129 #107 #109 | 滚动 Composite；Picture 自适应；PointerMove 窄路径 |
| **P4** | 多窗编排 | #116 #148 #149 #150 | 副窗独立 tree/session；State fan-out 仅 wake 有关窗 |
| **P5** | Handle 零维护 | #145–#152 #147 | mount 自动 snapshot；emit 走 dispatch_semantic |

**原则**（仍适用于后续工作）：

- **不**为兼容保留固定 interval 探活或每轮 `tick_effects`；新 API / 主循环路径继续受 #105 零闲置约束。
- 新增能力须能回答 [demand-driven · 新 API 审查清单](systems/demand-driven.md#新-api-审查清单)。

---

## 实现进度总览

**能力清单**（按域）；未实现 → [后续工作](#后续工作)；细节 → 各系统 `> **实现注记**`。阶段摘要 → [已落地阶段（P0–P5）](#已落地阶段p0p5)。

### 已实现（按域）

#### 基础设施 · `core`

| 能力 | 要点 | 文档 |
|------|------|------|
| 几何与标识 | `Point` / `Rect` / `EdgeInsets`；`ComponentId` / `WindowId`（Generational，跨窗 tree scope） | [foundation](systems/foundation.md) · [#101](../decisions.md#d101) |
| Damage | `DirtyRegion` → `DamageRegion` → `PresentDamage` | [foundation · Damage](systems/foundation.md#damage-区域) |
| 错误 / 日志 / 诊断 | `Errc` / `Error`；Logger；Fatal crash log；**默认 log**（#89）；App 默认错误 Toast overlay | [foundation](systems/foundation.md) |

#### 平台 · `native`

| 能力 | 要点 | 文档 |
|------|------|------|
| 生产后端 | **Windows、Linux Wayland、macOS**（AppKit + Metal CpuUpload）；`create_platform()` 三平台 backend 均已编码；**交付优先级 Windows 优先**（[#167](../decisions.md#d167)） | [platform](systems/platform.md) |
| 测试后端 | `FakePlatform` 完整 trait 实现 | [platform · 测试](systems/platform.md#测试平台) · [testing](systems/testing.md) |
| 事件循环 | `IEventLoop`；`EventLoopWaker` 外部线程 wake | [platform · 事件模型](systems/platform.md#ieventloop) |
| 窗口可选能力 | `WindowOps` 返回 `Result`；未支持记 `NotImplemented` | [platform · 窗口可选能力](systems/platform.md#窗口可选能力) |
| GPU 上下文 | Windows D3D11/WGL；Linux Vulkan/EGL；`create_gpu_context` | [platform · 工厂与后端](systems/platform.md#工厂与后端) · [rendering · 多图形 API](systems/rendering.md#多图形-api) |

#### 绘制 · `draw`

| 能力 | 要点 | 文档 |
|------|------|------|
| GPU 引擎 | OpenGL ES：`GpuEngine` + WGL/EGL；D3D11/Vulkan：`PresentUploadEngine`；失败回退 SoftwareEngine | [rendering · 引擎](systems/rendering.md#引擎) · [#59](../decisions.md#d59) |
| 失效管线 | `Invalidation::Paint` / `Layout` / `Composite`；脏区合并 | [rendering · 管线](systems/rendering.md#管线与失效) |
| Composite 滚动 | Wheel / 键盘 / 拖拽 → `scroll_region` memmove | [rendering](systems/rendering.md) · [#107](../decisions.md#d107) |
| PicturePolicy | 元数据 + 运行时信号；`node_count≥8 && est_pixels≥65536`；部分静态 widget `Eligible` | [rendering](systems/rendering.md) · [component · PicturePolicy](systems/component.md#picturepolicy-元数据122) |
| 动画帧 | `AnimationRegistry` deadline；`tree.update(dt)` 窄标脏 | [rendering · 动画帧](systems/rendering.md#动画帧) · [component · 动画](systems/component.md#动画) |

#### 应用 · `app`

| 能力 | 要点 | 文档 |
|------|------|------|
| 三态主循环 | DeepIdle / RegisteredActive / Active；`wait_until`；Effect 仅 Active 且 pending 时 tick | [demand-driven · 主循环](systems/demand-driven.md#主循环状态机) · [application](systems/application.md#主循环) |
| WindowSession | 每窗独立 tree / Registry / 三态 / MainThreadQueue | [application](systems/application.md) |
| 多窗单 loop | `open_window`；`window_id` 路由；共享 AppState + Theme | [application · 多窗](systems/application.md#appstate--多窗--settings) |
| App 运行时 API | `run_after` / `run_interval`；`post_to_ui`；`update_view` / `set_root` | [application](systems/application.md) · [demand-driven](systems/demand-driven.md) |
| AppHandle 生命周期 | cloneable；close 时 cancel Timer、清空队列 | [application · AppHandle](systems/application.md#apphandle-生命周期) |
| reconcile 帧内合并 | `pending_root` + State 批次；layout 前至多一次 | [view-reactive · reconcile](systems/view-reactive.md#reconcile-合并) |
| on_start 回调 | `.on_start` / `.on_window_start` 首帧前注入 handle | [application · on_start](systems/application.md#on_start) |
| DI / Settings | `AppHandle::resolve`；`App::settings(path)` opt-in load | [application · DI](systems/application.md#cli-与-di) · [data](systems/data.md) |
| TestClock | `AppClock` / `TestClock` 注入 deadline 与 drain 测试 | [testing](systems/testing.md) |

#### 界面 · `ui`

| 能力 | 要点 | 文档 |
|------|------|------|
| View / Reconciler | keyed diff；同类型 patch；`view_factory` + `pending_root` | [view-reactive](systems/view-reactive.md) |
| 响应式 | `State` / `Computed` / `Effect`；窄 paint；跨窗 paint site fan-out | [view-reactive · 响应式](systems/view-reactive.md#响应式) |
| StateSlotId | `State::new` 单调 slot；capture 指纹 `TypeId + slot_id` | [view-reactive · StateSlotId](systems/view-reactive.md#stateslotid) · [#143](../decisions.md#d143) |
| Handler 智能重绑 | 显式 capture fingerprint / `semantic_handler!` → generation；无指纹保守重绑（#160） | [view-reactive · Handler](systems/view-reactive.md#handler-变更判定-135) · [event](systems/event.md) |
| ComponentHandle | mount 自动 snapshot；getter / 窄 invalidate / emit | [component · ComponentHandle](systems/component.md#componenthandle6172145) |
| component! | 推荐 authoring；`SnapshotSource` 自动提取；`#[snapshot(skip)]` | [component · Authoring](systems/component.md#authoring) |
| 无障碍快照元数据 | `ComponentConfigSnapshot::accessibility()` / `ComponentHandle::accessibility()` 派生 role、name、state，并提供静态 ARIA role/attribute 导出 | [component · ComponentConfigSnapshot](systems/component.md#componentconfigsnapshot) · [#99](../decisions.md#d99) |
| Manager 横切 | Focus / Interaction / Drag per-tree | [component · Manager](systems/component.md#manager-横切) |
| 内置 Widget | 86 个 Big Bang；OverlayStack（Modal / Tooltip / 菜单等） | [component · 内置 Widget](systems/component.md#内置-widget-目录) · [overlay](systems/overlay.md) |
| 事件 | HandlerTable（`ComponentId` key）；PointerMove 窄路径 | [event](systems/event.md) |
| 主题样式 | Theme / StyleSet 五态；`follow_system_theme` opt-in | [theme-style](systems/theme-style.md) |

### 关键机制（跨域）

| 机制 | 一句话 | 细则 |
|------|--------|------|
| **按需零闲置**（#105） | 有触发才工作；无 pending 零开销 | [demand-driven](systems/demand-driven.md) |
| ActiveWorkRegistry | 内置动画 / Tooltip / AppTimer / IME 自动 register | [demand-driven · Registry](systems/demand-driven.md#activeworkregistry) |
| 帧内合并（#118） | UiEvent → due work → post_to_ui → reconcile → layout → render | [demand-driven · 帧内合并](systems/demand-driven.md#帧内合并) |
| 开发者零维护（#130） | 调用方不维护 Picture 名单、Registry、标脏范围 | [demand-driven · 开发者契约](systems/demand-driven.md#开发者契约零维护) |

<a id="组件-snapshot-覆盖"></a>

### 组件 Snapshot 覆盖

全部 86 个内置 widget 均已手写 `SnapshotSource` 静态配置提取，排除 hover / pressed / focused / scroll / animation phase 等运行态。

| 分类 | 已覆盖 Widget |
|------|---------------|
| general | Button, Icon, Typography, Label, Divider, Space, FloatButton |
| containers | Container, Grid, Layout, Header, Sider, Content, Footer, Splitter, Affix, BackTop |
| navigation | Menu, MenuItem, Tabs, Breadcrumb, BreadcrumbItem, Pagination, Steps, Anchor, AnchorItem, Dropdown |
| input | Input, InputNumber, Select, Checkbox, Radio, Switch, Slider, Rate, Form, FormItem, TreeSelect, DatePicker, TimePicker, ColorPicker, Cascader, AutoComplete, Mentions, Segmented |
| display | Card, List, Tree, Carousel, Collapse, Descriptions, Avatar, Badge, Tag, Image, Empty, Result, Skeleton, Timeline, Calendar, Table, SelectableList |
| feedback | Modal, Drawer, Tooltip, Popover, Popconfirm, Alert, Message, Notification, ProgressBar, Spin |
| other | ScrollView, BarChart, LineChart, PieChart, QRCode, RichText, ThemeToggle, Transfer, Upload, Watermark |

`component!` 自定义组件自动提取 pub 字段，`#[snapshot(skip)]` 可排除。

---

<a id="p6-生产级框架"></a>

## P6：生产级框架

**当前优先级**（[#167](../decisions.md#d167)）：**Windows 优先** — 先把 Windows 端做到 **生产可用**；Linux / macOS parity 与 macOS 原生验证 **随后**；native raster 仍为性能增强 backlog。开发策略摘要 → [project · 开发策略](../project.md#开发策略)。

**图形后端目标**（#162）：在保持 `IGraphicsContext` / `GraphicsEngine` 契约不变的前提下，扩展多种 GPU API 与 factory 选型；选型仅在初始化完成，符合 #105。架构原则 → [graphics-backend-pluggable · 架构原则](systems/graphics-backend-pluggable.md#图形-api-架构原则)（非 P6 任务不必通读全文）。

### 生产级优先项

| 优先 | 项 | 状态 | 说明 |
|------|-----|------|------|
| P0 | **Windows 生产可用** | 进行中 | 稳定性、阻塞项、demo/docs 同步；**当前主验证与交付环境**（[#167](../decisions.md#d167)） |
| P0 | 平台层抹平差异 | 已确立 | 上层无 OS `#[cfg]`；差异仅在 `native` traits 实现 |
| P1 | Linux / macOS parity | 持续 | backend 已编码；parity 与回归 **Windows 基线达标后**加大权重 |
| P1 | macOS 原生运行验证 | 待验证 | AppKit backend、Metal CpuUpload、IME 已接；需真机验收 |
| P1 | demo / docs 与实现同步 | 进行中 | 覆盖矩阵、backlog、公开 API 对齐 |
| P2 | 无障碍 v1 基线 | 部分 | role/name/state 快照、键盘导航已落地；屏幕阅读器桥待后续 |
| P1 | D3D11 GPU native raster | backlog | **Windows 优先**（[#167](../decisions.md#d167) [#169](../decisions.md#d169)）；`RenderBackendRegistry` + caps 改 `RasterMode::GpuNative` |
| P2 | native raster（非 Win） | backlog | Metal / D3D12 GPU 光栅 — **增强**，D3D11 之后 |
| P2 | WebGPU 评估 | backlog | P6.6 远期 |

### 图形后端里程碑

| 里程碑 | 内容 | 状态 |
|--------|------|------|
| P6.0 设计 | 抽象分层、平台矩阵、回退链、术语 | ✅ |
| P6.1 基线 | `GraphicsBackend` 枚举；registry 单条目创建 | ✅ |
| P6.2 Windows | D3D11 CpuUpload + WGL native raster | ✅ |
| P6.3 Linux | Vulkan CpuUpload + EGL native raster | ✅ |
| P6.4 macOS | AppKit + Metal CpuUpload context；SoftwareEngine 回退 | 部分 — 原生验证待完成 |
| P6.5 配置 | App builder / env / Settings opt-in | ✅ |
| P6.6 WebGPU | 远期评估 | backlog |
| P6.7 可插拔 registry | registry、Profile 预设分派、present 统一 | ✅ |
| P6.8 可组合渲染轴 | `RasterMode` × `PresentMode` 替换 Profile；`BackendKind` 统一；表驱动装配 | backlog — [#169](../decisions.md#d169) |

<a id="p68-可组合渲染轴"></a>

### P6.8 可组合渲染轴（#169）

**目标**：以 [#168](../decisions.md#d168) 正交三轴为 **唯一** mental model；`RenderPipelineProfile` **breaking 移除**，分派改 `RasterMode` × `PresentMode`（× `GraphicsBackend`）；`BackendKind` 与上述轴对齐；factory / engine **表驱动正交装配**。

| 优先 | 项 | 状态 | 说明 |
|------|-----|------|------|
| P0 | Profile → 正交轴（breaking） | backlog | 删除 `RenderPipelineProfile`；`GraphicsContextCaps` 改 `raster` + `present`；`create_graphics_engine` 按轴 match |
| P0 | 表驱动 factory / engine 装配 | backlog | registry 行表达 API + 光栅 + present 组合；**禁止** bundled profile 第二套分派 |
| P1 | D3D11 GPU native raster | backlog | **Windows 优先**（[#167](../decisions.md#d167)）；扩展 `RenderBackendRegistry`；D3D11 context caps → `RasterMode::GpuNative` |
| P2 | `BackendKind` 统一 | backlog | 与 `RasterMode` / `PresentMode` / `GraphicsBackend` 对齐；消除 Profile 遗留 bundled 语义 |
| P2 | Metal / D3D12 GPU 光栅 | backlog | D3D11 之后；同 registry + caps 模式 |

<a id="p67-图形后端架构"></a>
<a id="p67-迁移验收-g1g11--m2m7"></a>

### P6.7 图形后端架构

可插拔 registry 与 **Profile 预设分派**（`RenderPipelineProfile`，#163）已落地；**Profile 已确认 breaking 移除**（[#169](../decisions.md#d169)），目标为 `RasterMode` × `PresentMode` — 见 [#168](../decisions.md#d168) · [P6.8](#p68-可组合渲染轴) · [graphics-backend-pluggable · 可组合渲染轴](systems/graphics-backend-pluggable.md#可组合渲染轴)。`draw::bootstrap_graphics_engine` 为唯一 probe 入口；`create_graphics_engine` **当前**按 profile 分派（过渡）；`native/graphics/<api>/` 为 API 对等实现根目录；`IGraphicsContext::present(PresentFrame)` 为统一 present 契约。

**Backlog**（按 [#169](../decisions.md#d169) 排序；完整分项见 [P6.8](#p68-可组合渲染轴)）：

| 项 | 说明 |
|----|------|
| **D3D11 GPU native raster** | **Windows 优先**；`RenderBackendRegistry` + caps → `RasterMode::GpuNative` |
| Profile → 正交轴（breaking） | 删除 `RenderPipelineProfile`；`RasterMode` × `PresentMode` 分派 |
| `BackendKind` 统一 | 纳入可组合 refactor，与正交轴对齐 |
| 表驱动 factory / engine 装配 | registry 正交组合，替代 bundled profile |
| Metal / D3D12 GPU 光栅 | D3D11 之后；扩展 `RenderBackendRegistry` |
| D3D12 context | registry `Planned` |
| WebGPU | P6.6 远期评估 |

实现细节 → [rendering · 多图形 API](systems/rendering.md#多图形-api) · [platform · 多图形 API 与 factory](systems/platform.md#多图形-api-与-factory) · [graphics-backend-pluggable · 架构原则](systems/graphics-backend-pluggable.md#图形-api-架构原则)。

---

## 后续工作

**权威 backlog 清单**（下列表为唯一完整枚举；其他文档仅链接至此）。明细与边界见 [demand-driven · 剩余差距](systems/demand-driven.md#剩余差距)。P6 分项见 [P6 生产级框架](#p6-生产级框架)；可组合渲染轴见 [P6.8](#p68-可组合渲染轴)；其余推进前须人类决策或新决策 #170+。

| 项 | 说明 | 文档 |
|----|------|------|
| **生产级框架（P6 优先）** | **Windows 优先**生产可用；Linux/macOS parity 与 macOS 验证随后。子项含可组合渲染轴（[P6.8](#p68-可组合渲染轴)）、D3D11 GPU raster、macOS 真机验证 | [P6 生产级框架](#p6-生产级框架) |
| **移动端（P7+ backlog）** | **未实现**。目标 iOS / Android 原生 backend；复用 `ui`/`app`/`draw` trait 与零闲置主循环；须 [#166](../decisions.md#d166) 后分阶段切片 | [project · 当前阶段 vs 目标](../project.md#当前阶段-vs-目标愿景) · [plan · P7+](../plan.md#里程碑与工作域) |
| **全栈扩展（backlog）** | **部分**：`data` 仅 Settings KV（opt-in）。**未实现**：网络层、HTTP 客户端、数据同步、服务端集成 | [data](systems/data.md) · [#166](../decisions.md#d166) |
| 无障碍（屏幕阅读器桥） | #99：v1 基线（role/name/state 快照、静态 ARIA 映射、键盘导航）已落地；屏幕阅读器平台桥待后续 | [component](systems/component.md#componentconfigsnapshot) |

---

## 源码目录详表

`src/` 子路径与系统文档的细粒度映射；功能域总览见 [architecture · 功能域 ↔ 系统](architecture.md#功能域--系统)。

| 路径 | 系统文档 | 说明 |
|------|----------|------|
| `src/core/*` | [foundation](systems/foundation.md) | geometry, damage, error, log, diagnostic, **component_id**, **window_id** |
| `src/native/traits/*` | [platform](systems/platform.md) | public OS API；含 `EventLoopWaker` |
| `src/native/shared/*` | [platform](systems/platform.md) | `window_lifecycle`、`ime_events` 等跨后端 helper |
| `src/native/backends/*` | [platform](systems/platform.md) | OS 壳：窗口、事件、CPU presenter（`gdi_presenter` 等）；**不含** GPU API 对等实现 |
| `src/native/graphics/*` | [platform](systems/platform.md) · [graphics-backend-pluggable](systems/graphics-backend-pluggable.md) | **#164** IGraphicsContext 对等 API 实现（vulkan / opengl / d3d11 / metal / d3d12 stub） |
| `src/native/test_harness/*` | [platform](systems/platform.md), [testing](systems/testing.md) | FakePlatform |
| `src/draw/pipeline/*` | [rendering](systems/rendering.md) | invalidation, FrameRenderer, AnimationRegistry |
| `src/draw/compositor/*` | [rendering](systems/rendering.md) | ScenePaint, LayerTree |
| `src/draw/engine/*` | [rendering](systems/rendering.md) | `bootstrap.rs`（GPU probe）、`factory.rs`（profile 分派）、`SoftwareEngine`（cpu）、`PresentUploadEngine`（`present_upload.rs`） |
| `src/draw/gpu_engine/*` | [rendering](systems/rendering.md) | GpuEngine |
| `src/draw/backend/*` | [rendering](systems/rendering.md) | RenderBackend 抽象、`registry.rs`（backend 配对） |
| `src/draw/font/*` | [rendering](systems/rendering.md) | FontService, text backends |
| `src/draw/spatial/*` | [rendering](systems/rendering.md) | PhysicalBox, Mat4, 3D 命中 |
| `src/app/shell/*` | [application](systems/application.md) | App builder, CLI, DI |
| `src/app/window_session.rs` | [application](systems/application.md) | WindowSession、三态、Registry |
| `src/app/window/*` | [application](systems/application.md) | Window — PlatformWindow 轻量包装 |
| `src/app/session_runtime.rs` | [application](systems/application.md) | AppRuntime：window_id 路由、EventLoopWaker |
| `src/app/app_handle.rs` | [application](systems/application.md) | AppHandle |
| `src/app/app_timer.rs` | [application](systems/application.md), [demand-driven](systems/demand-driven.md) | Timer API |
| `src/app/main_thread_queue.rs` | [application](systems/application.md) | post_to_ui FIFO |
| `src/app/active_work_registry.rs` | [demand-driven](systems/demand-driven.md) | ActiveWorkRegistry（内部） |
| `src/app/test_clock.rs` | [testing](systems/testing.md) | AppClock / TestClock |
| `src/app/window_config.rs` | [application](systems/application.md) | WindowConfig、open_window |
| `src/app/event_loop/*` | [application](systems/application.md) | run_widget_loop（设计名 run_app_loop） |
| `src/app/bridge/*` | [application](systems/application.md), [rendering](systems/rendering.md) | ScenePaint impl |
| `src/ui/app_state.rs` | [application](systems/application.md), [component](systems/component.md) | AppState snapshot registry |
| `src/ui/component_handle.rs` | [component](systems/component.md) | ComponentHandle |
| `src/ui/component_snapshot.rs` | [component](systems/component.md) | SnapshotSource |
| `src/ui/view/*` | [view-reactive](systems/view-reactive.md) | View DSL, adapter |
| `src/ui/core/widget/*` | [component](systems/component.md), [event](systems/event.md) | WidgetTree；`tree_core` / `tree_events` / `tree_dirty` |
| `src/ui/event.rs` | [event](systems/event.md) | (single file) |
| `src/ui/layout/*` | [layout](systems/layout.md) | |
| `src/ui/theme/*` | [theme-style](systems/theme-style.md) | |
| `src/ui/foundation/state.rs` | [view-reactive](systems/view-reactive.md) | State / Computed / Effect（经 `ui::state` 重导出） |
| `src/ui/foundation/virtual_scroll.rs` | [layout · VirtualScroll](systems/layout.md#virtual-scroll) | 大列表虚拟滚动 helper；Table/Tree/SelectableList/Select/TreeSelect 已接 VirtualListScroll |
| `src/ui/foundation/*` | [theme-style](systems/theme-style.md), [view-reactive](systems/view-reactive.md), [event](systems/event.md) | style, config, locale, focus_trap, clipboard |
| `src/ui/foundation/style/*` | [theme-style](systems/theme-style.md) | Style, StyleSet |
| `src/ui/animation/*` | [component](systems/component.md) | WidgetAnimation, easing, transition |
| `src/ui/widgets/*` | [component](systems/component.md) | built-in widgets |
| `src/ui/managers/*` | [component](systems/component.md) | per-tree manager container |
| `src/ui/overlay.rs` | [overlay](systems/overlay.md) | |
| `src/data/settings/*` | [data](systems/data.md) | |
| `src/tests/**` | [testing](systems/testing.md) | mirror `src/` layout |
| `src/prelude.rs`、`src/lib.rs` | [public-api · prelude](systems/public-api.md) | 推荐入口 `uix::prelude::*`；导出 App、View、State、ComponentId、常用组件、事件、handle 与布局类型 |

---

## 维护

- 阶段划分或原则变更 → 同步 [decisions.md](../decisions.md) #154（或 #170+ 新决策）与本文件。
- 能力落地或产生新差距 → 更新 [实现进度总览](#实现进度总览) 与各系统 `> **实现注记**`；**不**在本文件恢复 per-file 接线 checklist。
- 新 backlog 项追加到 [后续工作](#后续工作)；边界说明同步 [剩余差距](systems/demand-driven.md#剩余差距)。
- 新增 `src/` 路径映射 → 更新 [源码目录详表](#源码目录详表) 与对应系统文档「源码模块」。

# UIX 架构设计

> 最后更新: 2026-07-05
> 本文档是项目的实时架构地图，随代码变更同步更新。

---

## 一、Workspace 结构

```
uix workspace
├── src/        (uix)     单一框架 crate — 按系统模块组织
└── demo/       (uix-demo) 演示二进制
```

### 依赖拓扑

```
demo ──→ uix
```

---

## 二、uix 模块划分

```
uix/src/
├── api/              稳定公开契约（按系统分子模块）
│   ├── platform/     （契约内嵌于 platform::api）
│   ├── render/       渲染系统 API
│   ├── widget/       组件系统 API
│   └── runtime/      运行时系统 API
├── platform/         平台系统 — OS 抽象（Win32 / Wayland）
├── render/           渲染系统 — 2D 引擎、光栅化、字体、合成
├── widget/           组件系统 — Widget 框架、布局、主题、内置组件
│   └── scene/        Widget 呈现桥接（事件循环、RenderContext）
├── runtime/          运行时系统 — 应用入口、窗口、CLI、DI
└── view/             视图系统 — 声明式 UI API
```

### 系统职责

| 系统 | 模块路径 | 职责 |
|------|---------|------|
| **platform** | `uix::platform` | OS 抽象：窗口、事件、呈现、输入、文件、日志 |
| **render** | `uix::render` | 2D 渲染引擎、RenderPipeline、光栅化、字体、LayerTree |
| **widget** | `uix::widget` | Widget 框架、状态、布局、动画、主题、60+ 内置组件 |
| **runtime** | `uix::runtime` | 应用生命周期、窗口、CLI、依赖注入 |
| **view** | `uix::view` | 声明式 View API（函数式组合子） |
| **api** | `uix::api::{render,widget,runtime}` | 各系统稳定 trait 与类型契约 |

### platform 系统内部

```
platform/
├── api/              稳定公开契约（error / geometry / event / window / …）
├── services/         文件、通知、设置
├── log/              日志基础设施
├── diagnostic/       诊断与恢复
├── shared/           跨平台共享实现
├── presenter.rs      像素呈现器
├── test_harness/     测试用 Fake 实现
├── windows/          Windows 实现
└── linux/            Linux（Wayland）实现
```

### render 系统内部

```
render/
├── backend/          CPU/GPU/Null 渲染后端
├── compositor/       LayerTree + Picture + 视口变换
├── engine/           CPU 渲染引擎
├── font/             字体加载、布局、文本渲染
├── gpu_engine/       GPU 渲染引擎
├── painting/         PaintContext、DisplayList
├── pipeline/         帧调度与 RenderSession
├── primitives/       颜色、路径、描边
├── rasterizer/       纯函数光栅化
├── render_object/    渲染对象树
└── spatial/          空间坐标系统
```

### widget 系统内部

```
widget/
├── core/             Widget 运行时（WidgetTree、事件、布局钩子）
├── scene/            呈现桥接（event_loop、RenderContext、LayerTree 合成）
├── foundation/       状态、样式、国际化、剪贴板
├── layout/           Flexbox + Grid 布局引擎
├── theme/            设计令牌（Ant Design 5）
├── animation/        动画与过渡
├── managers/         跨组件服务（焦点、拖拽、事件等）
├── widgets/          内置组件库
└── macros.rs         define_widget! / tree! 宏
```

---

## 三、核心数据流

### 渲染管线

```
WidgetTree.update(dt) → dirty regions + scroll deltas
    ↓
WidgetTree.layout() → 仅遍历脏子树
    ↓
scene.scroll_region() → begin_frame(DirtyRects) → LayerTree.render() → end_frame()
    ↓
begin_frame(Overlay) → LayerTree.render_overlays() → end_frame()
    ↓
IPresenter.present() → 屏幕
```

### 事件流

```
OS 事件 → IEventLoop → pending_events → WidgetTree.dispatch_event()
    → 命中测试 → WidgetEventHandler.on_event()
    → EventBus.publish() → 外部订阅者
```

---

## 四、关键设计决策

| 决策 | 方案 | 原因 |
|------|------|------|
| 单一 crate | 全部系统合入 `uix` | 消除跨 crate 边界，模块内聚 |
| 系统模块 | platform / render / widget / runtime / view | 职责清晰，依赖单向 |
| API 契约 | `api/{system}/` + `platform/api/` | 实现与契约分离 |
| Widget 能力位 | `WidgetCapabilities` + 上转型 | 按需实现 Layout/Render/Event/Lifecycle |
| 增量渲染 | scroll_region + DirtyRects + PresentDamage | CPU 渲染只重绘变化像素 |

---

## 五、代码约束

- `deny(clippy::unwrap_used)`, `deny(clippy::expect_used)`
- 每个 Rust 文件 ≤ 900 行
- 中文注释
- 组合优于继承

# UIX 架构设计

> 最后更新: 2026-07-04
> 本文档是项目的实时架构地图，随代码变更同步更新。

---

## 一、Workspace 结构

```
uix workspace
├── platform/   (uix-platform)  OS 抽象层 — Win32/Wayland
├── graphics/   (uix-graphics)  2D 渲染引擎 — CPU + 可选 GPU
├── ui/         (uix-ui)        Widget 框架 — 60+ 组件 + 状态 + 布局 + 主题
├── app/        (uix-app)       应用入口 — 窗口生命周期 + CLI + DI
├── demo/       (uix-demo)      演示二进制
└── uix         根 crate        聚合重导出层
```

### 依赖拓扑

```
uix ──→ app ──→ ui ──→ graphics ──→ platform
           ↘        ↘              ↙
            ui ──→ graphics ──→ platform
```

| Crate | 核心导出 |
|-------|---------|
| **uix-platform** | `Platform`, `IPresenter`, `IEventLoop`, `Error`, `EventBus`, `UiEvent`, `Point/Size/Rect` |
| **uix-graphics** | `RenderSession`, `RenderBackend`, `Canvas2D`, `DamageRegion`, `PresentDamage`, `SoftwareEngine`, `Color`, `Path`, `FontService`, `SpatialContext` |
| **uix-ui** | `WidgetComponent`, `WidgetLayout`, `WidgetRender`, `WidgetEventHandler`, `WidgetLifecycle`, `State`/`Computed`/`Effect`, `WidgetTree`, `TokenProvider`, `LayoutEngine` |
| **uix-app** | `App`, `AppMode`, `Window`, `Cli`, `Container` |

---

## 二、分层架构

```
app 层     入口 + 窗口 + CLI + DI
ui 层      Widget 框架 + 组件库 + 状态 + 布局 + 动画 + 主题 + LayerTree
graphics 层 2D 渲染引擎 + RenderPipeline + 光栅化 + 字体 + 空间坐标
platform 层 OS 抽象 — Win32 / Wayland / 文件 / 日志 / 通知 / 设置
```

- **每层依赖下层接口**，下层不反向依赖上层
- 同层组件通过 trait 接口通信，不依赖具体实现
- 各 crate 均采用 **API 外观模式**：`api/traits.rs` + `api/types.rs` 定义公开契约，内部模块只保留实现

### uix-ui 内部目录

```
ui/src/
├── api/              稳定公开契约（traits + types）
├── core/             Widget 运行时（widget/ 子树、context、children）
├── render/           渲染管线（event_loop、context、layer、text、debug）
├── foundation/       基础能力（state、style、locale、clipboard、config、focus_trap、virtual_scroll）
├── layout/           Flexbox + Grid 布局引擎
├── theme/            Ant Design 5 设计令牌
├── animation/        动画与过渡
├── managers/         跨组件服务（焦点、拖拽、事件等）
├── view/             简化声明式 API（View / App）
├── macros.rs         define_widget! / tree! 声明宏
└── widgets/          内置组件库（按 Ant Design 分类）
    ├── general/      通用（Button、Icon、Typography、Divider、Space）
    ├── containers/   布局容器（Container、Grid、Layout、Splitter、Affix）
    ├── navigation/   导航（Menu、Tabs、Breadcrumb、Pagination、Steps）
    ├── input/        输入（Input、Select、Checkbox、Form、DatePicker 等）
    ├── display/      数据展示（Table、List、Card、Tree、Tag 等）
    ├── feedback/     反馈（Modal、Drawer、Alert、Message、Spin 等）
    └── other/        其他（Chart、RichText、ScrollView、ThemeToggle、Misc）
```

旧模块路径（`widget`、`render_loop`、`state`、`widgets::button` 等）通过 `lib.rs` 兼容层重导出，外部代码无需修改。

---

## 三、核心数据流

### 渲染管线

```
WidgetTree.update(dt) → dirty regions + scroll deltas
    ↓
WidgetTree.layout() → 仅遍历脏子树
    ↓
canvas.scroll_region() → begin_frame(DirtyRects) → LayerTree.render() → end_frame()
    ↓
begin_frame(Overlay) → LayerTree.render_overlays() → end_frame()
    ↓
IPresenter.present() → 屏幕
```

每帧两步：Geometry Pass（清除+绘制）→ Overlay Pass（叠加），均裁剪到脏区域。

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
| Widget 能力位 | `WidgetCapabilities` 位标记 + `WidgetComponent` 上转型 | 避免上帝接口，按需实现 Layout/Render/Event/Lifecycle |
| 增量渲染 | scroll_region + DirtyRects + multi-rect PresentDamage | CPU 渲染性能关键，只重绘变化像素 |
| API 外观模式 | 每个 crate 的 `api/` 模块定义公开契约 | 内部重构不影响外部使用者 |
| 响应式状态 | `State<T>` + thread_local 依赖追踪 | Computed 自动追踪依赖，惰性求值 |
| LayerTree | Picture(离屏) / ClipRect(裁剪) / Direct(直接) 三类节点 | 支持 RepaintBoundary 嵌套，按 z_index 预排序 |

---

## 五、代码约束

- `deny(clippy::unwrap_used)`, `deny(clippy::expect_used)`
- 每个 Rust 文件 ≤ 900 行
- 中文注释
- 组合优于继承，无 trait 继承模拟 OOP

# UIX API 重构报告

> 日期：2026-06-28
> 范围：所有 4 个业务层（platform / graphics / ui / app）+ 根 crate（uix）

---

## 1. 架构目标

### 重构前

```
内部模块定义 trait + struct + impl
lib.rs pub use 所有模块内容到 crate 根

← 内部模块改名 → 外部使用者编译失败
← 新增内部模块 → 外部 namespace 膨胀
```

### 重构后

```
api/traits.rs  定义接口契约（trait）
api/types.rs   导出数据契约（值类型）
api/mod.rs     pub use traits::*; pub use types::*;
lib.rs         pub mod api; pub use api::*;

内部模块只保留实现（impl），移除 trait 定义

← 内部模块改名 → 只需改 api 中的 pub use 路径
← 内部模块重构 → 接口不变则外部不受影响
```

---

## 2. 各层 api 模块结构

### platform 层（uix-platform）

```
platform/src/api/
├── mod.rs       — pub use traits::*; pub use types::*;
├── traits.rs    — 19 个接口定义
└── types.rs     — 值类型 re-export
```

| 文件 | 内容 |
|---|---|
| `traits.rs` | `Platform`, `PlatformWindow`, `IWindowManager`, `IWindowProperties`, `INativeHandle`, `IEventLoop`, `IPresenter`, `IGraphicsContext`, `IClipboard`, `ICursor`, `ITextInput`, `IKeyboard`, `IDisplay`, `IConsole`, `IFileDialog`, `IFileSystem`, `INotification`, `ITimer`, `ISystemInfo` |
| `types.rs` | `Error`, `Errc`, `Result`, `Point`, `Rect`, `Size`, `EdgeInsets`, `UiEvent`, `EventBus`, `KeyCode`, `KeyMod`, `MouseButton`, 以及所有 I* 的关联值类型 |

**迁移的内部模块**：
- `shared/platform.rs` → 已删除（仅 trait 定义）
- `shared/window.rs` → 移除 4 个 trait 定义，保留 `WindowOps`、`PlatformWindowCore`、impl
- `shared/event_loop.rs` → 移除 `IEventLoop` trait 定义，保留 `OsEventSource` + blanket impl
- `presenter.rs` → 移除 `IPresenter`/`IGraphicsContext` 定义，保留 `NullPresenter` + impl
- `types/key.rs` → 移除 `IKeyboard` 定义，保留 `KeyCode`/`KeyMod`
- `types/input.rs` → 移除 `IClipboard`/`ICursor`/`ITextInput` 定义，保留 `MouseButton`/`CursorType`
- `types/console.rs` → 移除 `IConsole` 定义，保留 `ConsoleColor`/`TerminalCapabilities`
- `types/display.rs` → 移除 `IDisplay` 定义，保留 `DisplayInfo`
- `types/system.rs` → 移除 5 个 I* trait 定义，保留 `SpecialDir`/`MemoryInfo`/`OsInfo`

---

### graphics 层（uix-graphics）

```
graphics/src/api/
├── mod.rs       — pub use traits::*; pub use types::*;
├── traits.rs    — 5 个接口定义
└── types.rs     — 值类型 re-export
```

| 文件 | 内容 |
|---|---|
| `traits.rs` | `GraphicsEngine`, `Canvas2D`, `RenderingBackend`, `TextBackend`, `UpdateStrategy` |
| `types.rs` | `Color`, `Path`, `PathBuilder`, `Vec2`, `Vec3`, `Mat4`, `SpatialContext`, `FrameGraph`, `RenderOutcome`, `BlendMode`, `FontHandle`, `ImageHandle`, `Radius`, `Transform`, `DirtyRegion`, `PositionedGlyph`, `TextLayout`, `GlyphRaster`, `NullEngine`, `GpuEngine`, `SoftwareEngine`, `FontService`, `BitmapFont` 等 |

**迁移的内部模块**：
- `traits/mod.rs` → 改为 `pub use crate::api::traits::{...}`
- `traits/engine.rs` → 移除 `GraphicsEngine` 定义（文件保留但未被引用）
- `traits/canvas_2d.rs` → 移除 `Canvas2D` 定义
- `traits/rendering_backend.rs` → 移除 `RenderingBackend` 定义
- `traits/update_strategy.rs` → 移除 `UpdateStrategy` 定义
- `text_backend.rs` → 移除 `TextBackend` trait 定义，保留所有 struct 定义
- `text_backends/ab_glyph.rs` → 改为 `use crate::api::traits::TextBackend;`
- `font_service.rs` → 改为 `use crate::api::traits::TextBackend;`

---

### ui 层（uix-ui）

```
ui/src/api/
├── mod.rs       — pub use types::*;
├── traits.rs    — 占位（后续逐步迁移）
└── types.rs     — 所有公开类型 re-export
```

ui 层有 14 个 trait，当前阶段**保持它们在各自内部模块中**，通过 `api/types.rs` 统一 re-export。这是因为：

- **Widget 系列 trait**（`Widget`, `WidgetLayout`, `WidgetRender`, `WidgetEventHandler`, `WidgetLifecycle`, `IntoWidgetNode`, `WidgetCore`）与 `define_widget!` 宏和 blanket impl 深度耦合
- **主题 trait**（`IColorTokens`, `ISpacingTokens`, `IBoxShadowTokens`, `ITypographyTokens`, `TokenProvider`）方法多、分布广
- `LayoutEngine` 和 `Animatable` 较简单，后续优先迁移

---

### app 层（uix-app）

```
app/src/api.rs — 单文件，无 trait，仅 re-export
```

| 导出项 | 来源 |
|---|---|
| `App`, `AppMode`, `map_ui_event` | `application.rs` |
| `Cli` | `cli.rs` |
| `Container` | `di.rs` |
| `Window` | `window.rs` |

---

## 3. 使用方式

### 推荐路径（优先使用 api 模块）

```rust
// platform 层
use uix_platform::api::traits::{Platform, IClipboard};
use uix_platform::api::types::{Point, Error};

// graphics 层
use uix_graphics::api::traits::{GraphicsEngine, Canvas2D};
use uix_graphics::api::types::{Color, Path, Vec2};

// ui 层（当前阶段 trait 从内部模块 re-export）
use uix_ui::api::{Widget, Button, LayoutEngine, State};

// app 层
use uix_app::api::{App, Window};
```

### 快捷路径（lib.rs 统一 re-export，仍然可用）

```rust
use uix_platform::{Platform, Point};
use uix_graphics::{GraphicsEngine, Color};
use uix_ui::{Widget, Button};
use uix_app::{App, Window};
```

---

## 4. 后续计划

### 短期（ui 层解耦）

1. **LayoutEngine** → 从 `layout/engine.rs` 迁移到 `api/traits.rs`（纯 trait，无默认方法）
2. **Animatable** → 从 `animation/easing.rs` 迁移到 `api/traits.rs`
3. **IColorTokens / ISpacingTokens / IBoxShadowTokens / ITypographyTokens / TokenProvider** → 逐步迁移到 `api/traits.rs`

### 中期（Widget trait 解耦）

Widget trait 分解为更细粒度的子 trait，将纯行为接口逐步迁移到 `api/traits.rs`，实现细节保留在 `widget/` 模块中。

### 远期

- 各层测试改用 api 模块中的接口（而非直接引用内部模块）
- 文档生成从 api 模块提取公开接口描述

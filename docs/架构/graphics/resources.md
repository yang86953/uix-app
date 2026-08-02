# resources 模块

[← 架构索引](../../架构.md)

> **接口**：声明 graphics 系统的目标 `resources` 模块及字体、文本布局、图像生命周期。依赖：[geometry](geometry.md)、[backend](backend.md)。导出：`FontService`、`ImageService` 及资源句柄。

> **当前实现线索**：主要位于 `src/draw/resources/`。

## 组件清单

| 组件 | 类型 | 职责 |
|---|---|---|
| `FontService` | struct | face/fallback、layout、glyph cache 与 raster |
| `TextBackend` | trait | 字体后端布局与光栅协议 |
| `TextLayout` / `LineInfo` / `LineMetrics` / `PositionedGlyph` | struct | 保留源字符索引的布局结果 |
| `GlyphRaster` / `GlyphCache` | struct | 字形位图/轮廓缓存 |
| `ImageService` / `ImageSlot` | struct | 解码、槽位与 backend 资源协调 |
| `BitmapHandle` / `ImageHandle` / `FontHandle` | value | 带失效边界的资源身份 |

## 组件：FontService

布局按实际字体覆盖拆段，再统一应用换行和垂直对齐；CRLF/CR/LF、末尾空行、Tab、软断点和不可断空白保留源字符索引语义。

## 组件：ImageService

组件保存 handle 而非 texture；解码失败返回 typed error，释放或设备重建使旧 generation 失效。

## 模块不变量

字体/图像 I/O 与 cache 有容量和生命周期边界；资源服务不登记无条件逐帧工作。

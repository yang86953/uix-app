# geometry 模块

[← 返回架构索引](../../架构.md)

> **接口**：声明 graphics System 的 `geometry` Module 及绘制值契约。基础依赖：[core/geometry](../core/geometry.md)。值经 graphics System 公开/私有契约供 painting、scene、renderer 与 ui 消费，不授予兄弟 Module 实例依赖。

> **当前实现线索**：主要位于 `src/draw/geometry/`。

## 组件清单

| 模块 | 组件 | 职责 |
|---|---|---|
| `color` | `Color`、`colors` | 8-bit RGBA 绘制值和常用常量 |
| `path` | `Path`、`PathBuilder`、`PathSegment`、`FillRule`、`LineCap`、`LineJoin` | API-neutral 矢量路径 |
| `types` | `Radius`、`BlendMode`、`Transform`、`GradientDirection`、`TextLayoutOptions` | 绘制状态和值对象 |
| `stroker` | `StrokeOptions` | 描边宽度、连接和端点参数 |
| `flattener` / `tessellator` | flat path / mesh | 把曲线转换为后端可消费几何 |
| `spatial` | `Mat4`、`Vec2/3/4`、`AABB3D`、`Quad2D`、`Ray3D`、`SpatialContext` | 空间变换、命中和物理单位 |

## 组件：Color

`Color` 是最终 8-bit RGBA 绘制值，不携带品牌、暗色模式或组件覆盖语义；UI 主题先解析语义 token，再把 Color 交给 graphics。

## 组件：Path / PathBuilder

Path 保存调用方声明的 segment 与 fill/stroke 语义；backend 可以直接执行、flatten 或 tessellate，但不能改变 FillRule、LineCap、LineJoin 和 painter order。

## 组件：Transform / spatial types

变换、clip、命中、语义 bounds 和 damage 使用同一组合链。UI 提交 logical geometry，RenderTarget 在 surface 边界处理 DPR 与 physical extent。

## 模块边界

| core | graphics |
|---|---|
| `Point`、`Size`、`Rect`、`Constraints`、`EdgeInsets` | Path、Color、Transform、BlendMode、Stroke、空间几何 |
| 布局/命中可复用的基础值 | 绘制录制和后端执行需要的值 |
| 不知道颜色或光栅化 | 不决定 Widget 布局策略 |

`ColorScale`、`PrimaryHue`、`ThemePrimitives`、`DesignTokens`、`Theme` 和 `TokenPatch` 属于 [ui/theme](../ui/theme.md)，当前定义在 `src/ui/theme/`。

```text
ui Theme / DesignTokens
  → 解析语义色、字号、圆角、阴影
  → graphics Color / Radius / TextLayoutOptions
  → painting / Renderer
```

graphics 不引用 Theme 或组件 token；这保证 `draw` 不反向依赖 `ui`。`Color` 只是最终绘制值，不带品牌、暗色模式或组件覆盖语义。

## 模块不变量

- 所有传入 backend 的尺寸、坐标和变换在边界处验证为有限值。
- geometry 不持有 GPU/OS 资源，也不登记 timer、frame 或线程。
- API-specific shader、surface handle 和 adapter 信息不进入公共几何类型。

# 术语表

本表解释 UIX 文档中反复出现的核心名词。术语按定稿设计解释；源码若使用非定稿术语，应按文档重构。

| 术语 | 含义 |
|------|------|
| 功能域 | 源码组织维度：`core` / `native` / `draw` / `ui` / `app` / `data`。 |
| 系统 | 设计文档组织维度，例如事件系统、渲染系统、主题系统；不要求与源码目录一一对应。 |
| Widget | UI 组件实例，封装配置、交互态、布局、绘制和事件处理能力。 |
| View DSL | 声明式 UI 构建 API，例如 `column`、`row`、`button`、`dynamic_label`。 |
| ComponentId | 组件树中的稳定 generational ID。 |
| State | 响应式状态原语；变更后驱动失效与重绘。 |
| Computed | 从 State 派生的响应式值，负责依赖追踪和缓存。 |
| Effect | 帧末副作用；通过 State 间接影响 UI。 |
| UiEvent | native 域产出的统一平台事件，是 app 边界的输入。 |
| SystemEvent | app 边界后的系统层事件，位于平台事件与语义事件之间。 |
| SemanticEvent | 语义事件，例如 Click、Change、Submit、FileDrop。 |
| CustomEvent | 业务自定义事件，使用 typed payload。 |
| HandlerTable | 按 ComponentId 存储业务回调的结构。 |
| ScenePaint | app 与 draw 之间的场景绘制契约，使渲染系统不依赖具体组件树类型。 |
| DisplayList | 节点级绘制命令缓存，可 replay 以减少重复构建绘制命令。 |
| LayerTree | 合成阶段的数据结构，组织内容、子节点后绘制和缓存图层。 |
| Picture | 离屏缓存结果，由深度启发式自动创建。 |
| DirtyRects | 需要重绘的逻辑矩形集合。 |
| PresentDamage | 提交到平台呈现器的受影响区域，用于局部 present 或 damage swap。 |
| AnimationRegistry | 动画帧驱动中心；组件级 `tick` / `wants_frame` 已废止。 |
| Style | 组件外观与布局属性集合。 |
| StyleSet | normal / hover / pressed / focused / disabled 五态样式集合。 |
| ColorValue | 颜色引用，可来自色板、中性角色或自定义 RGB。 |
| TypographyToken | 排版令牌，由 Theme 解析为具体字号、行高和字体族。 |
| OverlayStack | 浮层统一调度器，管理 Modal、Tooltip、ContextMenu 等。 |
| FakePlatform | 测试用平台实现，用于内存窗口、事件和 present 断言。 |

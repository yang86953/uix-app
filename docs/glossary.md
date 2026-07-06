# 术语表

| 术语 | 含义 |
|------|------|
| 功能域 | 源码维度：`core` / `native` / `draw` / `ui` / `app` / `data` |
| 系统 | 文档维度；不要求与源码目录一一对应 |
| App / AppState | 应用入口与全局共享状态（多窗共享） |
| Theme | 全局主题快照：色板、NeutralRole、TypographyScale |
| Widget | UI 组件实例 |
| View DSL | `column`、`row`、`button`、`dynamic_label` 等声明式 API |
| ComponentId / ComponentHandle | 稳定 generational ID；只读配置句柄 |
| State / Computed / Effect | 响应式原语；Effect 禁止直接改 UI |
| Constraints | 布局约束 `{ min, max, definite }` |
| UiEvent / SystemEvent / SemanticEvent / CustomEvent | 平台 → app 边界 → 语义/业务 三层事件 |
| HandlerTable | 按 ComponentId 存业务回调 |
| ScenePaint | app↔draw 场景绘制契约 |
| DisplayList / LayerTree / Picture | 绘制命令缓存、合成层树、深度≥4 离屏缓存 |
| DirtyRects / PresentDamage | 逻辑重绘区 / 平台上屏 damage |
| AnimationRegistry | 动画帧驱动 |
| Style / StyleSet | 外观属性；五态 normal/hover/pressed/focused/disabled |
| ColorValue / NeutralRole / TypographyToken | 颜色引用、中性角色、排版令牌 |
| OverlayStack | Modal / Tooltip / ContextMenu 统一调度 |
| FakePlatform | 测试用内存平台 |

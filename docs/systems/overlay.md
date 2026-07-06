# 浮层系统

> Modal、Tooltip、ContextMenu 等，由 **OverlayStack** 统一调度（#100）。

| 类型 | 要点 |
|------|------|
| Modal（#97） | Overlay + focus trap；独立子树；共享 Theme |
| Tooltip（#96） | v1 完整组件，延迟显示 |
| ContextMenu（#98） | 右键 Semantic 事件 → OverlayStack；HandlerTable 规则同主树；Theme 全局共享（#94） |

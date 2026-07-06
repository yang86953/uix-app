# View 与响应式系统

> 声明式 View 描述 UI；State 驱动重建与 diff。业务**仅**通过 View DSL 创建 UI（#21）。

组合器：`column`、`row`、`button`、`label`、`scroll`、`grid` 等。

## Reconciler（#49、#60、#62）

按 **key** 匹配（无 key 则同层索引）。类型不同 → 卸载并新建 ComponentId。类型相同 → 比较 Style、文本、静态 props；Style 变则 paint invalidate；handler **一律重新注册**。

## 响应式

| 原语 | 行为 |
|------|------|
| State | 变更 → paint invalidate（#24） |
| Computed | 依赖追踪（#78） |
| Effect | 帧末副作用，须经 State，禁止直接改 UI（#79） |

Handler 须 `'static`；业务数据经 AppState + ComponentHandle（#32）。产出 Generational ComponentId 树（#35）供布局、事件、绘制消费。

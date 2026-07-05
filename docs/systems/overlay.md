# 浮层系统

> 系统职责：**Modal、Tooltip、右键菜单** 等浮于主树之上的 UI。

---

## 1. 统一管理（#100）

**OverlayStack** 统一调度所有浮层，避免各组件自建根节点。

---

## 2. Modal / Dialog（#97）

- Overlay 层 + **focus trap**  
- 独立 Widget 子树  
- 与主树共享 Theme  

---

## 3. Tooltip（#96）

v1 **完整** Tooltip 组件，含**延迟显示**。

---

## 4. ContextMenu（#98）

右键 → 事件系统 Semantic **ContextMenu** → OverlayStack 弹出菜单。

---

## 5. 与事件 / 主题

- 语义事件 Bubble 可委托；浮层内 HandlerTable 与主树规则相同  
- Theme **全局共享**（#94）  

---

## 6. 落地要求

| 项 | 要求 |
|----|------|
| Modal / Tooltip / Drawer / Popover | 统一纳入 OverlayStack 调度 |
| 焦点 | OverlayStack 统一管理焦点捕获与恢复 |
| 事件 | OverlayStack 统一处理遮罩、穿透、关闭策略 |
| 主题 | 浮层共享全局 Theme |

---

## 7. 相关决策

#96、#97、#98、#100 — [decisions.md](../decisions.md)

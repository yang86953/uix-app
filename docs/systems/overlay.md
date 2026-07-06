# 浮层系统

← [Main](../Main.md) · 系统 **#9** · 功能域：`ui`

> Modal、Tooltip、ContextMenu 等，由 **OverlayStack** 统一调度（#100）。周期性浮层（Tooltip 延迟、Spinner）须 **RegisteredActive register**（#111）。

## 索引

| 主题 | 章节 | 决策 |
|------|------|------|
| 调度 | [OverlayStack](#overlaystack) | #100 |
| 类型 | [浮层类型](#浮层类型) | #96–#98 |
| 生命周期 | [生命周期](#生命周期) | #94 #97 #111 |
| 事件交互 | [事件交互](#事件交互) | #98 |

**关联**：[event](event.md) · [component](component.md) · [application](application.md) · [demand-driven](demand-driven.md)

---

## OverlayStack

独立于主 WidgetTree 的浮层栈（`ui/overlay.rs`），存储于 `WidgetTree.overlay_stack`。

### 数据结构

```rust
struct OverlayEntry {
    id: OverlayId,
    owner: WidgetId,
    kind: OverlayKind,
    bounds: Rect,
    z_index: i32,
    modal: bool,
    dismiss_on_outside: bool,
    focus_trap: bool,
    managed: bool,           // rebuild 时保留
}
```

| API | 行为 |
|-----|------|
| `push(owner, kind)` | 自动分配 id，按 z_index 排序 |
| `push_entry(entry)` | 完整控制 |
| `remove` / `remove_for_owner` | 移除 |
| `top()` / `iter()` | 遍历（iter 双端） |
| `hit_test(x, y)` | 逆序扫描 bounds.contains |

### 与主树关系

```text
主 WidgetTree                    OverlayStack
     │                                │
     ├── 常规 layout/render           ├── 独立 bounds / z_index
     ├── HandlerTable                 ├── 共享 HandlerTable 规则
     └── Theme 快照 ────────────────────► 共享 Theme (#94)
```

浮层 **不** 进入主树 children；命中测试时 **优先** overlay（见 [event · 派发](event.md#派发流程)）。

---

## 浮层类型

| OverlayKind | 决策 | 要点 |
|-------------|------|------|
| **Modal** | #97 | `modal=true`, `focus_trap=true`；遮罩 + 独立子树 |
| **Drawer** | — | 侧边滑出；默认可 dismiss outside |
| **Tooltip** | #96 #111 | v1 完整组件；**框架内** Timer register 延迟显示；非 modal |
| **ContextMenu** | #98 | 右键 Semantic → 160×160 默认菜单 overlay |
| **Popover** | — | 定位浮层 |
| Message / Notification | — | 轻提示 |
| Custom | — | 扩展 |

Modal / Drawer 默认：`dismiss_on_outside=true`（Modal 外点击拦截在 dispatch 层处理）。

---

## 生命周期

### rebuild 同步

布局后 `rebuild_widget_overlays`（`tree_layout.rs`）：

```text
1. 遍历可见 widget
2. 收集 WidgetRender::overlay_entry()
3. 丢弃 managed=false 的旧 entry
4. push widget 提供的新 entry
```

`managed=true` 的 entry（如手动 push 的 ContextMenu）在 rebuild 间保留。

### Focus trap（#97）

Modal 打开 → 焦点限制在 overlay 子树；Tab 循环不逃逸到主树。  
关闭 → 恢复先前焦点 widget。

实现：`ui/foundation/focus_trap.rs` + dispatch 层 modal 外 pointer 拦截。

---

## 事件交互

| 场景 | 行为 |
|------|------|
| PointerDown | `overlay_target_at` 优先于主树 hit_test |
| Modal 外点击 | `intercept_top_overlay_outside_pointer_down` 吞掉或 dismiss |
| 右键 | PointerUp → `open_context_menu_overlay` at click pos |
| Wheel | overlay 命中则 overlay 消费 |
| Semantic | HandlerTable 规则同主树；path 含 overlay widget id |

FileDrop / ContextMenu semantic 与 overlay 协作见 [event](event.md)。

---

## Theme

浮层共享 App 全局 Theme（#94）；不单独持有一份 token。Theme 切换时 overlay 子树同样 palette-only invalidate。

---

## 与 View DSL

用户层通过 Widget 组件（Modal、Tooltip、Drawer）声明；运行时转为 `overlay_entry` 或由 dispatch 动态 `push`。业务 handler 仍在 HandlerTable，不在 overlay struct 内。

---

## 源码模块

```text
ui/overlay.rs              OverlayStack, OverlayEntry, OverlayKind
ui/foundation/focus_trap.rs   Modal focus trap（#97）
ui/core/widget/tree_layout.rs rebuild_widget_overlays
ui/core/widget/tree_events.rs overlay 命中与 modal 外拦截
```

详见 [Main · 源码目录详表](../Main.md#源码目录详表)。

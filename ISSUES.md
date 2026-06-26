# uix-app 框架布局问题报告

> 来源：belldandy 项目 `src/gui/chat_input.rs` 中 Container 包裹 Input 导致输入框不显示
> 日期：2026-06-26

---

## 1. 问题概述

在 Flex Column 布局中，用 Container 包裹一个有明确 `preferred_size` 的子 widget（如 Input textarea），该子 widget 在某些边缘情况下无法正确渲染。

### 复现条件

```rust
// ❌ 不工作：Container 包裹 Input
WidgetNode::new(
    Box::new(Container::new().h(60.0).flex_shrink(0.0).pad(EdgeInsets::new(0, 8, 0, 8))),
    vec![WidgetNode::leaf(Box::new(input))],
)

// ✅ 正常工作：Input 直接作为叶子节点
WidgetNode::leaf(Box::new(input))
```

父容器结构：

```
Container(Column, flex_grow=1)
├── Header (h=48, flex_shrink=0)
├── BrainPanel (h=350, flex_shrink=0)
├── StatusBar (h=42, flex_shrink=0)
├── ScrollView (flex_grow=1)      ← 占据剩余空间
└── Container(wrapper, h=60)       ← 输入框包裹层
    └── Input(textarea)            ← 无法渲染
```

### 触发条件

1. Container 在 Column 末尾，紧跟 `flex_grow=1` 的兄弟节点
2. Container 设置了 `fixed_height` 但没有 `fixed_width`
3. 子 widget（Input）有明确的 `preferred_size`

---

## 2. 根本原因：4 个关联缺陷

### 🔴 缺陷 1：`preferred_size` 签名不支持子节点感知

**文件：** `ui/src/widgets/container.rs:48-51`

```rust
// NOTE(布局): preferred_size 签名仅为 (&self, _engine: ...) -> Size，
// 无法访问 WidgetTree，因此无法遍历子节点估算内容尺寸。
// 当无 fixed_width/fixed_height 时返回 (0,0)，由父容器 flex 布局分配实际空间。
```

**问题：** 当 Container 无 `fixed_width` 时，`preferred_size` 返回 `(0, height)`。父容器 flex 布局依赖 `preferred_size` 做初始空间分配，宽度 0 会导致后续 layout 阶段起点错误。

**建议修复：**
```rust
// 方案 A：扩展签名传入 WidgetTree
fn preferred_size(&self, engine: ..., tree: Option<&WidgetTree>) -> Size;

// 方案 B：在 build 阶段预计算子节点尺寸，缓存到 Container 内部
// Container 新增字段 cached_content_size: Cell<Size>
// build_node 时由 WidgetTree 填充
```

---

### 🟡 缺陷 2：`layout_shrink` 的 viewport 检测只查直接父节点

**文件：** `ui/src/widget/tree_core.rs:486-491`

```rust
let parent_is_viewport = self.get(id)
    .and_then(|n| n.parent())
    .and_then(|pid| self.get(pid))
    .map(|p| p.inner().children_clip(p.frame()).is_some())
    .unwrap_or(false);
if parent_is_viewport { continue; }
```

**问题：** 当 Container 包裹 Input（形成 Container → Input 嵌套），Container 的父节点不是 viewport，但 Container 的子节点 Input 可能被 `layout_shrink` 错误地纳入收缩计算。只检查直接父子关系，不递归检查祖先链中的 viewport 边界。

**建议修复：**
```rust
fn has_viewport_ancestor(tree: &WidgetTree, mut id: WidgetId) -> bool {
    while let Some(pid) = tree.get(id).and_then(|n| n.parent()) {
        if tree.get(pid)
            .map(|p| p.inner().children_clip(p.frame()).is_some())
            .unwrap_or(false)
        {
            return true;
        }
        id = pid;
    }
    false
}
```

---

### 🟡 缺陷 3：布局收敛循环上限偏低

**文件：** `ui/src/widget/tree_core.rs:305`

```rust
for _converge_pass in 0..5 {  // ← 仅 5 次外层迭代
    // Phase 1: Top-down
    // Phase 2: layout_expand (内层 3 次)
    // Phase 3: layout_viewports
    // Phase 4: layout_shrink
}
```

**问题：** 深层嵌套（Container → Container → Widget）时，Phase 2 扩展和 Phase 4 收缩可能形成跨迭代振荡。5 次外层迭代在极端情况下不足以收敛，导致最后一帧的子节点 frame 为错误值。

**建议修复：**
```rust
let max_passes = 10;  // 提升上限
let mut prev_frames = HashMap::new();
for converge_pass in 0..max_passes {
    // ...
    // 检测稳定性：比较本轮与上一轮所有 frame 是否一致
    let stable = /* ... */;
    if stable { break; }
}
```

---

### 🟠 缺陷 4：Container 的 `layout_children` 中 visible 过滤过于激进

**文件：** `ui/src/widgets/container.rs:120-125`

```rust
let visible_children: Vec<WidgetId> = children.iter().copied()
    .filter(|&cid| tree.get(cid).map(|n| n.visible()).unwrap_or(false))
    .collect();
if visible_children.is_empty() {
    return Vec::new();  // ← 无子节点时返回空，父容器不知情
}
```

**问题：** `tree.get(cid)` 返回 `None` 时（节点在树构建中尚未完全就绪），使用 `unwrap_or(false)` 将子节点视为不可见。如果在布局阶段树正在构建中，子节点会被静默过滤，Container 返回空布局结果，Input 得不到 frame。

**建议修复：**
```rust
// 区分"不可见"和"未就绪"
let visible_children: Vec<WidgetId> = children.iter().copied()
    .filter(|&cid| {
        tree.get(cid)
            .map(|n| n.visible())
            .unwrap_or(true)  // ← 未就绪时假设可见（安全侧），而非过滤
    })
    .collect();
```

或者增加日志：
```rust
if tree.get(cid).is_none() {
    log::warn!("[Container::layout_children] child {} not found in tree", cid);
}
```

---

## 3. 影响范围

| 场景 | 是否受影响 |
|------|-----------|
| 叶子 widget 直接参与 flex 布局 | ✅ 不受影响 |
| Container 包裹单子，有 `fixed_width` + `fixed_height` | ✅ 不受影响 |
| Container 包裹单子，只有 `fixed_height`（无 `fixed_width`） | ⚠️ 可能受影响 |
| Container 包裹多子，在 flex_grow=1 兄弟之后 | ⚠️ 可能受影响 |
| 深层嵌套 Container → Container → Widget | ⚠️ 边缘振荡风险 |

---

## 4. 优先级建议

| 缺陷 | 优先级 | 理由 |
|------|--------|------|
| 缺陷 1 (preferred_size) | 🔴 P0 | 架构级缺陷，影响所有无 fixed_width 的 Container |
| 缺陷 4 (visible 过滤) | 🟠 P1 | 静默失败，难以排查 |
| 缺陷 2 (viewport 检测) | 🟡 P2 | 特定嵌套场景触发 |
| 缺陷 3 (收敛上限) | 🟡 P2 | 极端嵌套时触发，提升上限成本极低 |

---

## 5. 临时规避方案（当前 belldandy 采用）

```rust
// 不使用 Container 包裹，直接返回叶子 WidgetNode
// Input 的 preferred_size (80, 60) 在父 Column 中由 AlignItems::Stretch 自然拉伸
WidgetNode::leaf(Box::new(input))
```

**适用范围：** 单个 widget、只需要 fixed_height 的场景。如果确实需要 Container 提供的 padding/bg_color/border，则必须同时设置 `fixed_width`。

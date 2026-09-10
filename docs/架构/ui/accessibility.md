# accessibility 模块

[← 返回架构索引](../../架构.md)

> **接口**：声明 ui System 的无障碍语义、实时快照和动作桥。与[组件运行时框架](widget_runtime.md)及[event](event.md)的协作由 ui System 编排，Module 间不直接持有实例。导出：统一语义树和无障碍动作契约；公开标注见[使用 · 事件](../../使用/交互与反馈/事件.md#无障碍)。
>
> **当前实现线索**：相关实现暂位于 `src/ui/accessibility/accessibility_override.rs`、`src/ui/widget_snapshot/`、`src/ui/accessibility/semantic_snapshot.rs` 等位置；重构后由本模块统一派生语义树。

## 组件清单

| 组件 | 类型 | 职责 |
|---|---|---|
| `AccessibilityRole` | enum | Button、TextBox、Dialog、Tree 等角色及 `None` 排除 |
| `AccessibilityState` | struct | disabled、checked、expanded、selected、value、输入状态 |
| `AriaAttribute` | struct | 受控名称/值覆写 |
| `AccessibilitySnapshot` | struct | 单组件实时语义字段 |
| `SemanticNode` / semantic snapshot body | 内部结构 | 节点树、bounds、actions、revision |
| `AccessibilityOverride` | crate 内 struct | View 声明对默认字段的局部覆盖 |
| `SemanticAction` | crate 内 enum | focus/invoke/set-value/scroll 等统一动作 |

## 组件：AccessibilitySnapshot

- 内置组件从运行态实时产生默认 role、name、state、value 和 actions。
- View 的 role/name/state/ARIA 覆写只替换显式字段，其余仍由组件快照派生。
- `AccessibilityRole::None` 把节点及子树排除出语义树；不能只隐藏父节点而泄漏敏感子节点。
- 语义 bounds 使用实际 frame、clip、滚动、overlay 提升和 visual transform，与命中/绘制一致。

## 语义树生命周期

```text
WidgetTree 当前状态
  → 捕获组件快照
  → 合并 AccessibilityOverride
  → 过滤不可见/排除节点
  → SemanticNode 树 + revision
  → Agent Bridge / 未来平台桥
```

快照由所属 `WindowSession` 在事件、due work 或帧轮次末尾按需发布；发布后不可变，内容变化才单调推进 revision，不创建轮询线程、timer 或额外 frame。快照绑定窗口与 tree generation，旧快照只能用于观察，不能授权新动作。

## 焦点链

- Tab/Shift+Tab 只遍历有效可见、可接收事件且可聚焦的节点。
- 无当前焦点时分别从首项/末项开始；overlay focus trap 把遍历限制在最上层受控子树。
- 逻辑焦点身份与“键盘可见焦点环”分离；pointer modality 可以隐藏焦点环但不清除文本编辑焦点。
- 隐藏/移除含焦点子树时先发 FocusOut、注销 IME、清理 focus-within，再销毁身份。

## 敏感值与动作

- password 或标记 sensitive 的值不进入快照、Agent 响应或日志；可公开 role/state，但 value 必须省略。
- Agent/自动化使用 opaque node ID，并把动作重新投递到目标窗口的正常语义/系统事件路径。
- node ID 只在当前窗口 generation 内有效；跨启动稳定定位使用应用声明的 automation ID。
- automation ID 是定位信息而非凭据；重复或歧义目标必须拒绝，动作执行仍要重新验证窗口 generation、节点可见性、可用 action 与当前授权策略。

## 不变量

- 语义树是 WidgetTree 的派生视图，不是第二棵可写 UI 树。
- 无障碍动作不得在 IPC/后台线程直接执行组件回调。
- 语义变化不等于视觉变化；仅在需要时标记相应 Paint/Layout，而不是无条件请求帧。
- 节点移除、换根、窗口关闭或 generation 变化时，平台桥、Agent wait 和待处理动作必须失效，不能保留语义节点所有权。

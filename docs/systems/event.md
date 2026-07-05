# 事件系统

> 系统职责：输入如何变成**系统 / 语义 / 自定义**事件，业务如何**绑定**回调。

---

## 1. 三层模型

本节定义事件模型。源码若存在不符合本节的事件链路，按本节重构。

| 层 | 内容 | 谁消费 |
|----|------|--------|
| 系统 | 指针、键盘、窗口、主题、剪贴板、IME | 组件 dispatch |
| 语义 | Click、Change、Submit、FileDrop、ContextMenu… | HandlerTable |
| 自定义 | 业务 payload | HandlerTable |

使用侧**只绑定**语义层与自定义层。

---

## 2. 系统事件（概要）

SystemEvent 包含：指针（含 enter/leave/wheel）、键盘、焦点、窗口尺寸与焦点、主题/locale 变更、剪贴板、文本输入（IME）。坐标为**逻辑像素**（#70）。平台事件在应用边界转为 SystemEvent。

---

## 3. 语义事件

内置常用语义 + **register_semantic** 扩展（#66）。Click 携带按键、位置、修饰键（#36）。

---

## 4. 自定义事件

Typed payload + 注册宏（#6）；与语义层 API 形态一致。

---

## 5. 传播

| 规则 | 内容 |
|------|------|
| 系统 | Bubble 默认；Capture opt-in（#4） |
| 语义 | **默认 Bubble**（#25） |
| 多 handler | 顺序执行（#7） |
| stop | 阻止后续 handler **+** 语义冒泡（#68） |
| preventDefault | 语义层支持（#92） |

---

## 6. HandlerTable（#10）

- 挂在 **ComponentId**  
- 闭包 **static**；数据经 AppState / Handle（#26）  
- **when**：每次派发前求值（#91）  
- **once**：首次触发后移除（#90）  
- View rebuild：**一律重新注册**（#62）  

---

## 7. 剪贴板与 IME（#71、#75）

Copy/Cut/Paste、TextInput：系统层进 dispatch；可编辑组件 emit 语义事件。v1 支持单行、多行、完整 IME 组合。

---

## 8. 拖放与右键（#95、#98）

文件拖放 → Semantic **FileDrop**。右键 → Semantic **ContextMenu** + 浮层（见 [overlay.md](overlay.md)）。

---

## 9. 派发流程（概念）

流程：平台输入 → SystemEvent → 命中与焦点链 → 可选 Capture → 命中节点 dispatch → 可选 Bubble → HandlerTable → 标脏。

---

## 10. 测试（#40）

FakePlatform + 派发事件 + 语义断言 + 绘制快照。

---

## 11. 落地要求

| 项 | 要求 |
|----|------|
| 系统输入 | `native::traits::event::UiEvent` 作为平台统一输入 |
| 系统边界 | app 统一转换为 `SystemEvent`，覆盖全部输入类型 |
| 业务事件 | 语义事件与自定义事件经 HandlerTable 统一派发 |
| Handler 生命周期 | View rebuild 时重新注册；HandlerTable 支持 when/once/preventDefault/stop |

---

## 12. 实现顺序

Event + HandlerTable 为 #50 第三、四步。

---

## 13. 相关决策

#4–#7、#10、#25–#26、#36、#40、#61–#62、#66、#68、#71、#90–#92、#95、#98 — [decisions.md](../decisions.md)

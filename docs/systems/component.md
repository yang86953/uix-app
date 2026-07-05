# 组件系统

> 系统职责：单个 UI **组件**是什么、有哪些能力、如何经历生命周期。

---

## 1. 核心原则

| 原则 | 说明 |
|------|------|
| 改样式 = 改属性 | 变外观只改样式数据 |
| 颜色来自主题 | 组件默认只用色板 / 中性角色 |
| 事件分层 | 组件处理系统事件；业务绑语义 / 自定义 |
| 绑定在使用侧 | 回调在 HandlerTable，不在 struct |
| 能力拆分 | Layout、Render、Event、Lifecycle |
| 单一导入 | 组件作者通过 prelude 使用框架 |

---

## 2. 组件数据（struct）

组件 struct 只保留配置、交互态与样式数据；业务绑定和外观变体外置到 HandlerTable 与 StyleSet。

| 放 | 不放 |
|----|------|
| 配置（文本、disabled） | 业务闭包 |
| 交互态（hovered、pressed、focused） | variant / size 枚举 |
| StyleSet | on_click 字段 |

---

## 3. 能力拆分

| 能力 | 职责 |
|------|------|
| Layout | measure → Size（#29） |
| Render | 读 StyleSet + Theme 绘制 |
| Event | dispatch 系统事件；命中测试；焦点 |
| Lifecycle | attach / mount / active / theme / unmount… |

纯展示组件可不实现 Event。

---

## 4. 生命周期

**阶段**：Construct → Attach → Layout → Mount → Active/Inactive → Unmount → Detach → Destroy；另 ThemeChanged 时 palette-only invalidate。

| 规则 | 内容 |
|------|------|
| Mount/Unmount | 对称 setup / teardown |
| Active/Inactive | 焦点 + 视口（#8、#19） |
| 业务 | **不进** lifecycle，走事件 |
| 动画 | **AnimationRegistry**，组件无 tick（#83、#85） |

---

## 5. ComponentHandle（#61、#72）

只读**配置**；可 invalidate 或 emit 自定义事件；不可改 style；不可读交互态。

---

## 6. Authoring

- 结构：类型 → struct → trait → 私有逻辑  
- **component 宏**生成 ID 与 trait 胶水（#20）  
- prelude **精选**高频符号（#69）  

---

## 7. Button v1

| 项 | 设计 |
|----|------|
| 交互 | 指针 hover/press；Enter/Space → Click |
| 样式 | StyleSet 预设；默认 button_default（#77） |
| 范围 | **纯文本**；无 icon/loading/Ripple（#23、#30） |

---

## 8. 落地（#58）

基础设施就绪后 **Big Bang** 全量重写内置组件与 demo（#80）；不保留双实现。

---

## 9. 标脏来源

| 来源 | 负责 |
|------|------|
| 交互态 | dispatch |
| AnimationRegistry | Registry |
| State | 框架自动 |
| Theme | palette-only |

---

## 10. 落地要求

| 项 | 要求 |
|----|------|
| Button 外观 | 纯文本 + StyleSet 预设 + TypographyToken |
| 业务绑定 | 组件 struct 不存业务闭包，统一经 HandlerTable |
| 生命周期 | Widget trait 拆分基础能力；Active/Inactive、ThemeChanged 语义明确 |
| 动画 | 废除组件级 tick / wants_frame；仅由 AnimationRegistry 驱动 |

---

## 11. 相关决策

#8、#16、#20–#23、#29–#30、#58、#77、#83、#85 — [decisions.md](../decisions.md)

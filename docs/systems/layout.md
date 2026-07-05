# 布局系统

> 系统职责：组件**占多大空间**、Flex/Grid **如何排布**、滚动**如何消化** Wheel。

---

## 1. measure 与 Constraints（#29、#38）

组件通过 **measure** 在 **Constraints**（min、max、definite）下返回 **Size**。

输入：组件树、Style 布局字段、父级约束。输出：每个节点的逻辑像素 frame。

---

## 2. 盒模型

margin、padding、border（四边）、content 区域；与 Style 字段一致。

---

## 3. Flex + Grid（#53）

v1 **同时**支持 Flex 与 Grid（废止原「仅 Flex」#41）。

| 能力 | 配置位置 |
|------|----------|
| flex 方向、align、justify、gap | **Style**（#81） |
| grid 轨道 | **Style** grid_template（#84） |
| grid 容器 | View **grid** 组合器（#67） |

---

## 4. Scroll（#45、#73）

**ScrollContainer** 消化 Wheel，必要时 Bubble。View **scroll** 组合器，支持纵 / 横。

---

## 5. 与渲染

Layout 失效 → 走 layout，**不**触发 present。Paint 失效才 present。

---

## 6. Active/Inactive 与视口（#19）

生命周期 **Active**：与祖先 clip/scroll **求交**后有可见像素。

---

## 7. 落地要求

| 项 | 要求 |
|----|------|
| Flex / Grid | 保持 Flex + Grid 双模型 |
| Scroll | 与 Composite 失效语义保持一致 |
| Active/Inactive | 与 clip/scroll 求交后决定可见活跃状态 |

---

## 8. 相关决策

#8、#19、#29、#38、#41→#53、#45、#67、#73、#81、#84 — [decisions.md](../decisions.md)

# 渲染系统

> 系统职责：如何把组件树**画**到像素并**高效**上屏。

---

## 1. 定位

| 原则 | 说明 |
|------|------|
| 不知组件类型 | 只依赖 ScenePaint 契约 |
| 双引擎 | GPU 优先，失败 CPU（#59） |
| 局部重绘 | Layout 不 present；Paint/Composite 才 present |

---

## 2. 引擎

| 结果 | 行为 |
|------|------|
| Idle | 无像素变化；**跳过 present** |
| Present | 带 damage 区域上屏 |

---

## 3. 失效类型

| 类型 | present |
|------|---------|
| Layout | 否 |
| Paint | 是 |
| Composite（如滚动条带） | 是 |

队列可合并 Paint 矩形；无工作 + 无动画 → Idle 短路。

---

## 4. ScenePaint 边界

渲染系统通过 ScenePaint 读取：节点边界、绘制回调、滚动与 layer 元数据。**不**依赖具体组件树类型（由应用系统桥接）。

---

## 5. 合成

- **层树**：Content + AfterChildren 单层 Pass  
- **DisplayList**：节点级记录与 replay  
- **滚动条带**：Composite 优化  

---

## 6. 自动离屏缓存（#82、#86、#87）

- **无**显式 RepaintBoundary 组件  
- 子树深度 **≥ 4** → 启发式 Picture 缓存  

---

## 7. 绘制能力

填色、描边、圆角、文本、图片、裁剪、变换、可选阴影。

绘制阶段解析 ColorValue + TypographyToken（见 [theme-style.md](theme-style.md)）。

---

## 8. 动画帧（#85）

非 Idle 帧由 **AnimationRegistry** 驱动，非组件 wants_frame。

---

## 9. HiDPI（#70）

layout/paint 用逻辑像素；present 前按 scale_factor 缩放。

---

## 10. 数据流

失效队列 ← State / 交互 / 滚动 → 帧渲染 → 平台 present。

---

## 11. 落地要求

| 项 | 要求 |
|----|------|
| 渲染入口 | FrameRenderer、LayerTree、DisplayList 保持 ScenePaint 边界 |
| 离屏缓存 | 无显式 RepaintBoundary；统一为深度启发式 Picture |
| 主题绘制 | 统一通过 ColorValue / TypographyToken 解析 |
| GPU damage | 与 CPU PresentDamage 语义一致 |

---

## 12. 相关决策

#59、#70、#82、#85–#87 — [decisions.md](../decisions.md)

# 平台系统

> 系统职责：隔离 **OS 差异**，提供窗口、输入、呈现。

---

## 1. 定位

| 原则 | 说明 |
|------|------|
| 唯一平台分支区 | 上层禁止 OS 条件编译 |
| traits 对外 | 上层只依赖平台契约 |
| 行为一致 | 各 OS 对外语义相同 |
| Fail Fast | 不可用 → Err |

本文定义平台系统边界；平台只产出统一事件和能力契约，不向上泄漏 OS 细节。

---

## 2. Platform 能力

聚合：窗口管理、事件循环、剪贴板、光标、显示、定时器、文件系统等。

---

## 3. 窗口与呈现

| 能力 | 说明 |
|------|------|
| CPU | 像素缓冲 + 局部 damage present |
| GPU | 图形上下文 + damage swap |
| PresentDamage | 全屏或矩形列表 |

---

## 4. 事件

事件循环：阻塞 / 轮询 / 超时。**UiEvent**：指针、键盘、滚轮、窗口、焦点、定时器、拖放等。

平台 backend 产出 `UiEvent`，应用边界转换为 `SystemEvent`（见 [application.md](application.md) 与 [event.md](event.md)）。

键码、修饰键、鼠标键在此定义，界面层再导出。

---

## 5. 平台支持

| 平台 | 状态 |
|------|------|
| Windows | CPU + 可选 GPU ✅ |
| Linux | Wayland + EGL ✅ |
| macOS | 未实现 |

---

## 6. 服务

文件、通知等：非热路径；差异通过 Result 表达。

---

## 7. 测试（#40）

**FakePlatform**：内存窗口、记录 present；配合事件与绘制断言。

---

## 8. 一帧角色

输入：事件循环 → 应用映射。输出：CPU present 或 GPU swap。

---

## 9. 落地要求

| 项 | 要求 |
|----|------|
| 平台事件 | backend 产出 `UiEvent`，应用边界统一转换为 `SystemEvent` |
| 后端 | 平台差异只封装在 `native/backends` |
| 条件编译 | 仅允许在 `src/native/backends/` 与 `src/native/factory.rs` |

---

## 10. 相关决策

#59、#70、#71、#95 — [decisions.md](../decisions.md)

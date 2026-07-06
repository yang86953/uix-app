# 基础设施系统

← [Main](../Main.md) · 系统 **#10** · 功能域：`core`

> 错误、几何、日志、诊断。最底层，无平台依赖，Fail Fast。

## 索引

| 主题 | 章节 | 决策 |
|------|------|------|
| 几何 | [几何](#几何) | #38 #56 #70 |
| Damage | [Damage 区域](#damage-区域) | #70 |
| 错误 | [错误](#错误) | AGENTS |
| 日志 | [日志](#日志) | — |
| 诊断 | [诊断](#诊断) | — |

**关联**：[rendering](rendering.md) · [layout](layout.md) · [platform](platform.md) · [demand-driven](demand-driven.md)

---

## 几何

定义于 `core/geometry.rs`；**全域复用**，禁止在 native/draw/ui 重复定义（AGENTS.md）。

| 类型 | 关键 API |
|------|----------|
| **Point** | `new`, `zero`, `midpoint` |
| **Size** | `new`（NaN→0）, `zero`, `infinite` |
| **Rect** | `new`, `inset`, `contains`, `intersect`, `union` |
| **EdgeInsets** | `uniform`, `horizontal/vertical` — margin/padding/border（#56） |

### 坐标约定（#70）

| 空间 | 用途 |
|------|------|
| 逻辑 px | layout、hit test、ScenePaint bounds |
| 物理 px | PresentDamage、GPU swapchain |

`scale_factor = display.dpi_scale()`；engine resize 时应用。

---

## Damage 区域

`core/damage.rs` — 连接 ui 失效与 platform 呈现。脏区几何是 **L2 最小脏区**（#105）的基础类型；见 [demand-driven · 三层目标](demand-driven.md#三层目标)。

### DirtyRegion（逻辑）

| 字段 | 含义 |
|------|------|
| `full_frame` | 整帧需重绘 |
| `clear_required` | 需清除背景 |
| `rects` | 逻辑坐标矩形列表 |

来源：`InvalidationQueue::dirty_region()`（Paint + Composite rects 合并）。16+ rects 时自动 merge 优化。

### DamageRegion（渲染）

FrameRenderer `compute_damage` 产出；可 padding 扩展防闪烁；转换为 present 格式。

### PresentDamage（物理）

```rust
enum PresentDamage {
    Full,
    Partial(Vec<(x, y, w, h)>),   // 物理像素
}
```

交给 `IPresenter::present` 或 `IGraphicsContext::swap_buffers`。

```text
ui invalidate_paint_rect (逻辑)
    → DirtyRegion
    → DamageRegion (draw)
    → PresentDamage (native)
```

---

## 错误

### Errc

分类码（`error/codes.rs`）：General、IO、Network、Protocol、Concurrency、Platform、App…

### Error 结构

```rust
struct Error {
    code: Errc,
    message: String,
    severity: ErrorSeverity,   // Info | Warning | Error | Fatal
    file, line, timestamp,
    source: Option<Box<Error + Send + Sync>>,
}
```

工厂：`invalid_arg`、`not_found`、`io_error`…；`From<std::io::Error>` 自动映射。

### Result

`pub type Result<T, E = Error>` + `ResultExt` + `uix_try!` / `uix_try_std!` 宏。

### 约束

- 生产 `deny(clippy::unwrap_used, clippy::expect_used)`
- 平台错误统一映射为 `core::Error`，不泄漏 OS 细节到 ui

---

## 日志

`core/log/` — 全局日志基础设施。

| 层 | API |
|----|-----|
| 自由函数 | `trace_fn`, `debug_fn`, `info_fn`, `warn_fn`, `error_fn`, `fatal_fn`（`#[track_caller]`） |
| Logger | 多 sink 单例 |
| Sink | `ConsoleSink`, `FileSink`（轮转）, `CallbackSink` |

级别：Trace < Debug < Info < Warn < Error < Fatal。  
默认全局级别 **Warn**（Logger 初始化前）。

帧指标、启动失败、引擎回退等走日志；**不进 UI 热路径**。

---

## 诊断

`core/diagnostic/` — IO/网络等离线场景；UI 帧内保持轻量。

| 模块 | 职责 |
|------|------|
| **Collector** | 线程安全错误环形缓冲；dedup；Fatal → abort |
| **fatal** | `install_fatal_handler`, `dump_crash_report` → `uix_crash.log` |
| **recovery** | `retry`, `with_recovery`, RetryPolicy, CircuitBreaker |
| **middleware** | LogMiddleware, RetryMiddleware 管道 |
| **timestamp** | 毫秒时间戳（无 chrono 依赖） |

Fatal severity 收集 → crash log + `abort()`。

### 与错误 UI 的关系（#89）

裁决 [#89](../decisions.md#d89)：**默认 log**；可选 **Toast** 向用户展示非致命错误。

| 层 | 职责 |
|----|------|
| `core` | 记录、分类、Fatal → crash log；**不**渲染 UI |
| `ui` 反馈 | Toast 等（见 [component · feedback](component.md#内置-widget-目录)）承载可见提示 |

业务在捕获 `Error` 后自行决定 log-only 或触发 Toast overlay；core 诊断链路不参与 WidgetTree 热路径。

---

## core 模块图

```text
core/
├── geometry.rs       Point, Size, Rect, EdgeInsets
├── damage.rs         DirtyRegion, DamageRegion, PresentDamage
├── error/            Errc, Error, Result, macros
├── log/              Logger, Sink, levels
└── diagnostic/       Collector, recovery, fatal, timestamp
```

**零**平台依赖；native/draw/ui/app/data 均可 use。

---

## 使用约束汇总

| 规则 | 消费者 |
|------|--------|
| 几何类型只用 core | layout, hit test, dirty, present |
| PresentDamage 从 core 导出 | native::present, draw::pipeline |
| 禁止 unwrap/expect | 全库（测试除外） |
| 诊断不阻塞 UI 帧 | app 主循环 |

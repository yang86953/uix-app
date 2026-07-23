# 运行保障

[← 架构索引](../../架构.md)

> **接口**：声明 platform 系统中 **diagnostic — 诊断与运行保障**模块的内部设计。所属系统：`platform`。依赖：[系统列表](../系统列表.md)、`core` 系统。导出：运行保障公开用法 → [使用 · 运行保障](../../使用/运行保障.md)。

## 模块定位

diagnostic 模块（`src/core/diagnostic/`、`src/core/error/`、`src/core/log/`）负责**诊断与运行保障**：错误类型定义、运行时诊断收集、崩溃处理、恢复策略、受控崩溃与 tracing 日志基础设施。

系统名称统一为**运行保障（Runtime Assurance）**，公开面为 `uix::diagnostics`，不复用平行的 `stability`、`recovery` 或 `logging` 根模块。

## 组件清单

| 组件 | 类型 | 职责 |
|------|------|------|
| `Fatal` | struct | 不可恢复错误类型；携错误码、上下文、调用栈 |
| `CrashReport` | struct | 崩溃报告；序列化为可读文本，脱敏写入日志 |
| `ErrorReportStore` | struct | 有界错误存储；LRU 淘汰，安全预算保护 |
| `ErrorRecovery` | trait | 领域恢复登记；按 Errc 精确匹配，返回恢复策略 |

### 职责分界

| 职责 | 负责 | 不负责 |
|------|------|--------|
| 维稳 | ABI unwind 隔离、owner-thread 汇聚、有界 pending queue、报告容量、递归与错误风暴抑制 | 隐藏可返回的错误、吞掉失败、伪造成功 |
| 纠错 | 按精确 `Errc` 登记恢复处理器，在安全责任线程协调一次恢复尝试 | 内置通用 retry、退避、熔断、GPU 重建、业务补偿或任务运行时 |
| 记录 | tracing 唯一日志、面向人的 ErrorReport、脱敏、持久化写 | 远程上传、结构化事件 schema、实时告警或 metrics sink |

## 组件：Fatal

**接口**：不可恢复错误。携带错误码、上下文描述、调用栈。抛出后经 ABI unwind 隔离，由 owner-thread 汇聚后写入 CrashReport。

## 组件：ErrorReportStore

**接口**：有界错误存储。安全预算保护，LRU 淘汰旧条目。写入和读取均受容量上限约束，防止错误风暴撑爆内存。

## 组件：ErrorRecovery

**接口**：领域恢复登记的 trait。调用方按精确 `Errc` 注册恢复处理器，错误发生时框架在安全责任线程协调一次恢复尝试。不内置通用 retry、退避或熔断。

## 组件：CrashReport

**接口**：崩溃报告。从 Fatal 和 ErrorReportStore 聚合信息，序列化为可读文本。脱敏后写入 tracing 日志。不远程上传。

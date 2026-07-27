# diagnostics 模块

[← 架构索引](../../架构.md)

> **接口**：声明 platform 系统的目标 `diagnostics` 模块，权威持有 runtime-scoped 错误观察与恢复协调。依赖：[core/error](../core/error.md)。导出：`uix::diagnostics`。

> **当前实现线索**：公开实现位于 `src/diagnostics/`；`src/core/diagnostic/` 的历史内部机制不定义目标公开边界。

## 组件清单

| 组件 | 类型 | 职责 |
|---|---|---|
| `Diagnostics` | cloneable struct | 单 runtime 报告存储与恢复登记 |
| `DiagnosticsConfig` / `BacktracePolicy` | struct/enum | 容量、紧急目录和 backtrace 策略 |
| `ErrorReport` / `ReportId` | struct/value | 有界、脱敏的不可变观察 |
| `DiagnosticsSnapshot` | struct | 按 ReportId 排序的时点快照 |
| `RecoveryAction` / `RecoveryOutcome` | enum | 恢复处理器动作与结果 |
| `RecoverySubscription` | RAII struct | 精确 Errc 处理器登记 |

## 组件：Diagnostics

`report` 只在最终责任边界把 typed Error 转成报告；`on_error` 按精确 Errc 登记，`attempt_recovery` 在调用方选定的安全 owner thread 同步尝试。未处理结果保留原 Error。

## 组件：ErrorReport

报告限制总字节、分段、cause 数和 metadata，清理控制字符并按策略捕获 backtrace；达到容量淘汰旧报告，错误风暴不能无限增长内存。

## 模块不变量

不内建通用 retry、GPU 重建、业务补偿、远程上传或第二套日志系统；结构化事件经 tracing，具体恢复归资源所属模块。

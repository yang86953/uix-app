# error 模块

[← 返回架构索引](../../架构.md)

> **接口**：声明 core 系统的目标 `error` 模块及其组件契约。依赖：无。导出：所有系统统一的失败语义。

> **当前实现线索**：主要位于 `src/core/error/`。

## 组件清单

| 组件 | 类型 | 职责 |
|---|---|---|
| `Errc` | enum | 稳定错误分类 |
| `ErrorSeverity` | enum | 严重度 |
| `Error` | struct | 消息、位置、cause 与错误码 |
| `Result<T>` | type alias | UIX 统一结果类型 |
| `ResultExt` / `ResultErrorExt` / `ResultVoidExt` | trait | 保留类型的上下文转换 |

## 组件：Error

内层系统创建并传播 Error，拥有资源的领域决定是否可恢复，最终责任边界再交给 platform 的 Diagnostics 观察。错误不能在中间层被日志后吞掉，也不能转换成伪成功。

## 模块不变量

error 模块只定义失败值与传播辅助，不执行重试、上报、UI 提示、图形重建或进程退出。

# settings 模块

[← 返回架构索引](../../架构.md)

> **接口**：声明 data 系统的应用设置模型、编解码和可恢复持久化。依赖：[core/error](../core/error.md)。导出：`SettingsService` 供 [app/application](../app/application.md) 按需注册。
>
> **当前实现线索**：当前实现主要位于 `src/data/settings/`；重构后仍保持冷路径数据服务边界。

## 组件清单

| 组件 | 类型 | 职责 |
|---|---|---|
| `SettingsService` | struct | 共享设置状态、类型化读写、加载与原子保存 |
| scalar codec | 内部机制 | 通过 `Display` / `FromStr` 转换标量，不触发 I/O |
| serde codec | feature 扩展 | `settings-serde` 启用时把单个 key 编码为 JSON 字符串 |
| temp/backup sidecar | 文件协议 | 在中断后恢复旧文件或完成新文件安装 |

## 组件：SettingsService

`SettingsService::clone()` 共享同一锁保护状态，而不是复制一份独立设置。内存读写可以从持有服务的调用方进行；磁盘 load/save 仍是显式同步冷路径。

## 组件：持久化事务

```text
内存快照
  → 同目录临时文件写入并同步
  → 旧目标移动为 backup
  → 临时文件安装为目标
  → 清理 sidecar
```

加载时先检查 sidecar：

- 旧文件已经移走而新文件尚未安装：恢复 backup。
- 新文件已经提交：保留新文件并清理遗留 sidecar。
- 无法安全判定：返回 typed error，不静默丢弃设置。

同目录临时文件保证最终替换不跨文件系统；保存失败不得破坏调用前仍可恢复的版本。

## 与 app 的边界

- `App::settings(path)` opt-in 后，application 模块在进入运行模式前加载并把 `SettingsService` 注册进应用容器。
- data 不知道窗口、主题、组件或图形 backend；键名和值语义由应用拥有。
- DI 容器只负责生命周期和解析，不自动触发保存。

## 所有权、安全与生命周期

- application System 为每个应用创建并拥有一个 `SettingsService` 实例；Clone 只是同一实例的共享句柄，不形成第二个设置 owner。
- 配置路径、key 语义、保存时机、数据迁移和关闭前是否保存由应用拥有。Settings 不在 Drop、窗口关闭或进程退出时隐式提交。
- 文件协议不承诺加密、密钥管理、系统凭据库或多进程写合并。密码、令牌等秘密不得直接进入普通 Settings；应用还需保护包含相同值的 temp/backup sidecar。
- 加载或解析失败时保留 typed 原因，应用显式决定修复、迁移或重置；模块不得自动清空未知或损坏数据。

## 不变量

- 磁盘 I/O 不进入每帧热路径。
- codec 失败返回 typed error，不以默认值伪装成功。
- 设置文件不承担日志、错误报告、任务队列或数据库职责。
- Settings 锁只保护进程内共享状态，不得被表述为跨进程事务或并发写入协调。

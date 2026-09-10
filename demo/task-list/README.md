# 任务清单：UIX Lang 与 Rust 的完整应用路径

[← 返回 UIX Lang 教程](../../docs/uix-lang/指南/教程.md) · [Rust 教程](../../docs/使用/入门/教程.md)

> **角色**：一个有限、可运行应用的使用指南。UIX Lang 负责界面，Rust 拥有领域状态、保存和外部工作；不要求先学习 Scheme 或可移植模块。
>
> **许可**：本示例与框架统一使用 [MIT](LICENSE)，第三方材料保留各自许可。

## 1. 项目与依赖

示例位于公开 UIX 仓库的 `demo/task-list/`，UIX Lang 描述界面，Rust 负责领域状态、保存与后台任务。
它与全组件展示 `demo/uix-lang-demo/` 是不同应用。

| 文件 | 职责 |
|---|---|
| `src/main.rs` | 解析保存路径，运行应用，退出后取消并回收 worker |
| `src/lib.rs`、`src/domain.rs` | 示例入口导出、任务身份、标题校验与统计计算 |
| `src/app.rs` | 单窗口组合、Settings、业务投影与后台结果提交 |
| `src/main.uix` | 任务表单、稳定 key 列表、任务/统计两页与回调 |
| `tests/workflow.rs` | 表单、导航、保存失败和损坏文件保护的行为验证 |
| `LICENSE`、`THIRD_PARTY_NOTICES.md` | 示例自身的许可与第三方边界 |

本示例的 Cargo 清单通过同一源码树的 `path + version` 依赖框架：

```toml
[dependencies]
uix-app = { path = "../..", version = "0.0.8", default-features = false, features = ["vulkan", "settings-serde"] }
serde = { version = "1", features = ["derive"] }
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
```

请保留完整仓库结构和示例自己的锁文件；不能只导出此目录后声称它不依赖父目录源码。
无须私有 Git 或私有 Registry 授权，但需要 Rust stable、原生桌面与相应 Vulkan 环境。
从完整源码树构建：

```bash
git clone https://github.com/yang86953/uix-app.git
cd uix-app/demo/task-list
cargo build --locked
# 可选：示例行为测试；不是原生桌面或人工输入验收。
cargo test --locked --features test-harness
```

框架的四个 0.0.8 包已经发布到 crates.io；本示例 `publish = false`，本页不表示示例也发布为 Registry 包。

## 2. 启动与保存位置

在仓库的 `demo/task-list` 目录运行：

```bash
# 只创建本示例的临时数据目录，不使用真实业务配置。
data_dir="$(mktemp -d)"
cargo run --locked -- "$data_dir/tasks.json"
```

命令只接受一个**绝对、有效 UTF-8 的文件路径**；没有路径或传相对路径会明确拒绝。目录必须由调用者选择，建议先使用临时目录。后续再次运行使用同一个完整路径；不要在第二次启动时重新执行 `mktemp` 而误以为保存丢失。

```bash
cargo run --locked -- "$data_dir/tasks.json"
```

同一保存文件只由一个实例使用。示例不实现多进程锁、合并、自动保存、迁移工具或数据库，也不用于存储凭据。当前 GUI 入口为 `TaskApp::new(path)` → `application().run()` → `shutdown()`；不是 CLI 模式下等待 `on_start` 的用法。

## 3. 一次走完操作链

1. 新文件打开后进入“任务”页。空标题点击“添加”应显示校验失败，不能生成空任务。
2. 输入“学习框架”“保存数据”等合成标题，逐项添加；点击某行“切换完成”。行通过业务 ID 识别，删除其他行不会把操作转到另一项。
3. 切到“统计”，点击“开始统计”。结果由真实本地线程计算，再回到目标窗口提交；界面显示总数、完成数和标题字符数。空清单统计会显示明确失败，而非伪装为后台成功。
4. 点击“保存”，只有真实落盘成功才显示“已保存”并清除“未保存”提示。保存失败保留内存编辑和可重试状态。
5. 关闭窗口，再用同一文件路径启动**第二个进程**，核对清单及完成标记仍在。不能用第一次运行里的 `get_struct` 代替重新加载验证。
6. 再新增一项但不保存，关闭后重开；这项不会恢复。这是本示例明确的“仅显式保存”策略，不是承诺关闭时自动保存。

后台统计输入最多 128 项，可能很快完成；不保证肉眼能看到每次忙碌状态或成功抢在完成前取消。取消与旧结果保护由明确的所有权和实际行为验证支撑，不靠给生产 worker 增加假延时。

## 4. UIX Lang 与 Rust 如何衔接

`TaskList` Widget 使用 `State<String>`、`State<Vec<String>>`、`State<bool>` 等已登记 props 接收草稿、任务 ID 列表和显示状态；带参回调把标题或 ID 交给 Rust。`external` 声明行标题格式化等调用点可见的窄依赖，并不导入服务或赋予 UI 文件持久化能力。

Rust 持有真正的 `TaskList` 领域数据；UIX 的 ID 数组只是稳定列表投影，不是第二份可独立修改的业务真值。`TaskRow` 接收字符串 ID，事件通过拥有型参数调用 Rust 的切换/删除方法；闭包需克隆字符串是 Rust 所有权要求，不是另外一套 UIX 业务语法。

`main.uix` 中的 `Input`、`For`、两页结构及状态文本是真正的声明式界面。唯一窄 `KernelHost` 只连接公开 `WindowControl::Close`，没有把整页藏在 Rust View 里。普通横排使用 `Container direction="row"`；`Row` 是栅格组件。条件语法和句柄类型以[组件规范](../../docs/uix-lang/规范/组件.md)为准。

公开可观察目标包括 `draft`、`add`、`nav-tasks`、`nav-stats`、`save`、`start-stats`、`cancel-stats`、`status`、`stats-result`、`close`，以及按稳定业务 ID 形成的 `toggle-<id>`、`remove-<id>`。这些标识用于示例行为核对，不是直接访问框架私有树的许可。

## 5. 状态、错误与保存责任

| 状态/资源 | 所有者与提交边界 |
|---|---|
| `TaskList { schema, next_id, tasks }` | Rust 领域对象；schema 为 1，稳定 ID 删除后不复用 |
| 输入草稿、当前页面、显示消息 | Rust 组合根持有句柄，UIX 读取/绑定并调用窄回调；不自动持久化 |
| `SettingsService` | `TaskApp` 创建并显式 `load` 的唯一实例；不再同时启用另一份 `App.settings` |
| “未保存”状态 | 领域编辑后置为真；只有 `save()` 成功才清除，不把 `set_struct` 当作落盘 |
| 后台请求、取消令牌、线程句柄 | Rust 组合根；UIX 只触发开始/取消并呈现结果 |

任务标题先去首尾空白，再要求 1–80 个 Unicode 标量且无控制字符；最多 128 项。这是演示应用的有限数据合同，不是框架的全局容量或排版字素限制。

Settings 在 `task_list` key 中保存结构化清单；外层仍是 Settings 的字符串 KV 格式，不是把裸任务数组直接作为整个文件。加载先经过 Settings 格式/结构解码，再检查 schema、ID 唯一性与标题约束。加载失败会显示错误并禁用编辑/保存，保留原文件，不静默改成可保存的空清单。

`set_struct` 只更新 Settings 内存；后续 `save` 可能失败。失败不表示所有内存状态回滚，也不能显示“已保存”。示例保留领域编辑和脏状态供用户修复保存条件后重试；关闭仍按未保存策略处理。详细服务合同见[配置](../../docs/使用/框架设施/配置.md)。

## 6. 后台结果与关闭责任

统计使用单个有界本地 worker，不访问网络。正在计算或取消回收中的重复请求不启动第二个线程。离开统计页、编辑清单或主动取消都会使旧请求失效；worker 回投到 UI 后仍需同时核对请求代际、数据 revision、页面和关闭状态，不能仅凭结果已经算出就覆盖当前界面。

`post_to_ui` 返回 `()`，不是执行或呈现回执。以显示状态或明确的结果读回确认业务完成，详见[定时与异步](../../docs/使用/动画与异步/定时与异步.md)。

窗口关闭使框架丢弃不再可执行的窗口队列；它不会替应用强制停止或 join 外部线程。`App::run` 返回后，Rust `shutdown` 标记关闭、取消请求、取回线程并 join。UI 事件路径只回收已结束的线程，不等待仍在运行的 worker。未保存编辑不会因这一步而自动写盘。

## 7. 验证范围

本页说明当前源码的组织和使用方式。编译、行为测试、原生窗口呈现、人工键鼠输入、
跨平台验收与性能测量是不同证据；本次文档迁移未重跑这些验证，不把旧私有提交的测试
结论直接移植到当前公开版本。

切页或重建任务行后，自动化调用者应重新观察当前目标和呈现 revision，再执行依赖新结构的动作。
输入入队不表示呈现或保存已完成；探针与正式应用的 feature、构建 profile 和 root 也应分别记录。

# 扩展引擎取证与第一版合同

[← 返回架构索引](../架构.md) · [目标设计](extensions.md) · [实施计划](../动态扩展实施计划.md)

> **状态**：P0 取证产物与实施期第一版合同，P1（无界面最小闭环）、P2（动态 UI 与受控业务能力）、P3（热替换与状态迁移）、P4（示例与交付验证）已按此实现并通过公开 API 测试与真窗 Agent 验收；参考负载基线见 [UIX-PERF-032](../../性能/UIX-PERF-032.md)。引擎选型、兼容矩阵、缺口补齐方案、Cargo 能力名、依赖登记与资源上限初值在本文冻结；公开 API 字段以 `tests/extensions_public_api.rs` 与使用文档实际交付为准。
>
> **权威范围**：扩展语言引擎的证据与实施合同。语言选择、生命周期与失败语义由[目标设计](extensions.md)持有；阶段顺序与验收由[实施计划](../动态扩展实施计划.md)持有。

## 1. 引擎对照与选型结论

取证日期：2026-09-06。比较维度来自目标设计的引擎门槛：标准语义、可控执行、循环对象回收、宿主借用安全、崩溃边界、许可、构建成本（详见下表证据来源）。

| 候选 | 标准语义 | 可控执行 | 循环对象回收 | 依赖成本 | 结论 |
|---|---|---|---|---|---|
| Steel `steel-core` 0.8.3 | 自述 R5RS 为主、缺 let-syntax，R7RS "underway" | 未见步数/配额/取消机制 | 自带 GC | 必需依赖 40+ crate（crossbeam×4、im-rc、icu_casemap、serde、bincode、httparse、which、xdg 等，docs.rs 0.8.3 依赖清单） | 否决：依赖治理不可接受，标准与可控执行均需侵入改造 |
| Stak `stak-r7rs` | 定位 R7RS-small 子集（Chibi/Gauche/Guile 子集） | 未见配额/取消；宿主函数经设备机制注入 | Ribbit 堆 GC | 内核极小（约 1.5 KLOC Rust） | 否决：嵌入控制点不足，配额、取消、能力校验、宿主回调边界均需重写 |
| 小贝 `agent/src/scheme/` | 尾调用、卫生 `syntax-rules`（含椭圆/literals）、`call/cc` + `dynamic-wind`、record、四层数值塔；64 个单测含官方样例回归 | 燃料/帧深/reader 深/源码字节/分配数/字符串字节六类配额 + 协作取消（AtomicBool） | **无**：`Rc` + `RefCell`，环不回收；`equal?` 靠深度上限终止 | 纯 Rust 自研 6788 行（含测试 1263）；外部依赖仅 num 系与 thiserror | **选定**：以私有移植落地，缺口按下文清单补齐 |

选型依据：目标设计「优先评估小贝引擎能否提取为不依赖 Agent 的独立实现」。小贝引擎对上层依赖仅一个单文件 `CancellationToken`（纯 std），可独立移植；其配额扣费点（机器每步、reader 每字符、每值构造入口）、能力名校验、宿主函数不可重入边界与 UIX 的可控执行要求直接对齐。Steel/Stak 的标准或嵌入缺陷使改造成本不低于自研移植，且引入无法治理的依赖面。

移植不复制小贝的 Agent 业务层；`scheme` 目录整体作为 UIX 私有引擎模块重新落库，thiserror 依赖按本仓库习惯改为手写 `Display`/`Error` 实现。小贝仓库不改。

## 2. R7RS-small 兼容矩阵

基线：小贝引擎取证（`xiaobei` `0fa72644`）+ 本仓移植补齐项。分类：**必需语义**（(scheme base) 及开放库不可缺省）、**受限环境**（标准定义但宿主策略限制）、**拒绝**（明确报错的缺口）。矩阵随实现更新，测试出处为 `tests/extensions_public_api.rs` 的标准语义段落。

### 2.1 必需语义

| 语义 | 标准出处 | 小贝现状 | 移植后 |
|---|---|---|---|
| 词法作用域、闭包、`define`/`set!` | R7RS §3, §4.2 | 支持 | 保留 |
| 尾调用（含 `cond`/`when`/`do`/`and`/`or` 尾位置） | §3.5 | 显式帧栈、尾位不压帧 | 保留 |
| `syntax-rules` 卫生宏（椭圆、literals、`define-syntax`/`let-syntax`） | §4.3 | 重命名+回退方案 | 保留 |
| `call/cc`、`dynamic-wind`（可重复重入） | §6.10 | 帧栈+wind 链不可变快照 | 保留 |
| `define-record-type` | §5.5 | 支持 | 保留 |
| 四层数值塔（fixnum/bignum/rational/flonum、规范化折叠） | §6.2 | 支持（num 系） | 保留 |
| `do`、`case`、`when`/`unless`、命名 `let` | §4.2 | 支持 | 保留 |
| `define-library`/`import` 基础形式 | §5.6 | 仅裸库名 | 补 `only`/`except`/`prefix`/`rename` 修饰符 |
| `include`/`include-ci`/`cond-expand` | §4.1, §5.6 | `NotImplemented` | 补齐：经宿主注入的库解析器读取包内声明来源，不搜索用户目录 |
| **多值**：`values`、`call-with-values`、`let-values`、`let*-values`、`define-values` | §4.2, §6.10 | 缺失 | 补齐 |
| **异常**：`raise`、`raise-continuable`、`with-exception-handler`、`guard`（含 reraise）、`error`、error-object 系谓词 | §6.11 | 缺失 | 补齐；配额/取消/超时/内部错误不经 Scheme 异常通道，直接终止求值（宿主边界不可捕获）。已知限制：`with-exception-handler` 的 handler 与 guard 内的 `raise-continuable` 以子机器调用，handler 内 continuation 逃逸语义受限 |
| **`parameterize`/`make-parameter`** | §4.2 | 缺失 | 补齐（参数对象 + 动态绑定栈，与 `dynamic-wind` 交互按标准） |
| **lazy**：`delay`、`delay-force`、`force`、`make-promise`、`promise?`（(scheme lazy)） | §6.9 | 缺失 | 补齐 |
| `(scheme base)` 过程缺口：`string-map`、`string-for-each`、`vector-map`、`vector-for-each`、`eof-object` 系、`exact-integer?`、`exact-integer-sqrt`、`floor/`、`floor-quotient`、`floor-remainder`、`truncate/`、`truncate-quotient`、`truncate-remainder`、`square`、`numerator`、`denominator`、`rationalize`、`assert`、`boolean=?`、`list-copy`、`list-set!`、`string->vector`、`vector->string`、`string-copy!/fill!` 区间、`vector-copy/copy!/fill!` 区间 | §6 系 | 逐项缺失 | 补齐 |
| bytevector 系（`make-bytevector`、`bytevector-u8-ref/set!`、`bytevector-copy/append`、`u8-list->bytevector`、`utf8->string`、`string->utf8` 等） | §6.3, §6.9 | 缺失 | 补齐（owned 字节，受配额；`#u8(...)` 字面量支持） |
| quasiquote / unquote / unquote-splicing（嵌套层级） | §4.2.8 | 缺失 | 补齐（求值期展开） |
| `(scheme eval)`：`eval`、`interaction-environment` | §6.1 | 缺失 | 受限补齐：`interaction-environment` 返回 `#f` 标记，`eval` 在全局环境求值 |
| `(scheme char)` 缺口：`char-ready?`（内存端口恒真）、`digit-value` | §6.7 | 缺失 | 补齐；大小写映射按 Rust `char` 语义（全 Unicode） |
| 字符串端口与读写：`open-input-string`、`open-output-string`、`get-output-string`、`read-line`、`read-string`、`write-string`、`write-char`、`write-u8`、`write-bytevector`（内存端口限定） | §6.13 | 缺失 | 补齐为内存端口；文件端口列入受限环境 |
| `(scheme cxr)` 24 个组合访问器 | §6.4 | 逐项待核 | 补齐（机械展开） |

### 2.2 受限环境与拒绝项

| 项 | 处置 |
|---|---|
| `(scheme file)`、`current-input/output/error-port` 的文件形态 | 拒绝：`no-file-ports`；文件访问只经宿主业务端口 |
| `(scheme process-context)`、`(scheme time)`、`(scheme repl)`、`(scheme load)`、`(scheme eval)` | `eval` 以受限形态开放（同一配额与能力内，不新增权限）；其余拒绝 |
| `(scheme read)` 文件读取 | 拒绝文件来源；字符串端口读取开放 |
| `(scheme inexact)`/`(scheme complex)` | inexact 开放（f64）；`complex` 拒绝（首期无复数） |
| `include-declarations`、任意路径逃逸 | 拒绝；只接受包内声明来源 |

覆盖不足时对外称「R7RS-small 实现中的受限宿主环境」，不承诺完整标准兼容。字符串大小写不敏感比较采用简单 Unicode 折叠（非完整 case-fold）；`string-normalize-*` 拒绝（无 Unicode 规范化依赖）。

## 3. 移植缺口补齐设计

### 3.1 可回收堆（长期实例核心缺口）

小贝的 `Rc` 循环引用在实例销毁后仍不释放，一次性累计配额不能当长驻实例的终身额度。移植后采用自研标记-清扫：

- 每个堆分配（pair/vector/string/record/closure/env/continuation/大数）在构造入口登记到引擎堆表（`Weak` 槽 + 字节记账），配额扣费沿用小贝入口点。
- 根集 = 帧栈操作数 + 当前环境链 + 全局环境 + continuation 栈 + 求值机寄存器；宿主回调执行期间引擎不再运行（单实例串行求值），回调入参天然是根。
- 触发 = 累计分配达到阈值或存活字节达软上限，在 `tick()` 安全点执行；mark 阶段每个堆变体枚举其引用，sweep 阶段对不可达对象清空其 `RefCell` 内容使环断裂、引用计数自然归零；`Weak` 失效槽位回收。
- 三独立上限：单次分配字节、sweep 后存活字节、单次回收步数（回收自身计燃料与墙钟）。
- 跨边界数据只过 owned 值（见 §4.4）；宿主不持久持有引擎 `Value`，无外部根。

### 3.2 墙钟超时与取消

`tick()` 追加 `Instant` 截止检查，超时返回独立错误码；取消 token 移植为本模块私有 `AtomicBool` 句柄。二者与配额同为不可捕获的宿主边界。

### 3.3 语义补齐落点

多值、异常、`parameterize`、lazy、bytevector、字符串端口按 §2.1 清单落在求值机（语法形式）与原语表（过程）；库系统修饰符与 `include`/`cond-expand` 的来源解析经注入的 `LibraryResolver`（包内声明 + 标准库登记表，默认拒绝未登记来源）。`cond-expand` feature 标识符首期冻结为：`r7rs`、`uix-extension`、`else` 与已登记库名。

## 4. 第一版公开合同（实施期冻结）

### 4.1 构建能力与模块

- Cargo feature：**`extensions`**，默认关闭；不进入默认图、最小图与 demo 默认图（最小图编译失败为既有 `MenuBar` 问题，与本能力无关）。
- 模块：`uix::app::extensions`（app 私有 Module，引擎为其私有子模块）；能力合同测试按 `capability_compile_contract.rs` 惯例补 `extensions` 一对 doctest。
- 依赖登记（P4 实测）：新增可选依赖 `num-bigint`、`num-rational`、`num-integer`、`num-traits`；三口径基线复核不变（默认 75 / 最小 51 / demo 82，与基线一致），启用 `extensions` 后图为 78（净增 3：num-bigint、num-rational、num-integer）。替代方案论证见 §1（Steel 依赖面不可治理、自研大数正确性风险高于复用 num 系）。不引入 thiserror、GC 库或任何解释器外部依赖。

### 4.2 Rust 门面（P1 最小闭环）

```rust
// feature = "extensions"
ExtensionHost::new(ExtensionHostConfig) -> ExtensionHost       // 显式创建运行时
ExtensionHost::prepare(&ExtensionPackage) -> Result<PreparedExtension, ExtensionError>
ExtensionHost::activate(PreparedExtension) -> Result<ActivationReceipt, ExtensionError>
ExtensionHost::call_command(&ExtensionId, &str, &[ExtensionValue]) -> Result<CommandOutcome, ExtensionError>
ExtensionHost::list() -> Vec<ExtensionStatus>
ExtensionHost::deactivate(&ExtensionId) -> Result<TeardownReceipt, ExtensionError>  // P1 基础停止
```

- `ExtensionPackage`：清单 + `.scm` 源文件集。内存构造经 `from_parts`；外部目录在应用显式指定路径时经 `ExtensionPackage::read_from_directory` 读取并冻结（目录形态、上限与失败语义见使用文档「外部扩展包」节；库不扫描、不监听、不跟随包内链接，来源授权归应用）。多文件包经 `(include "…")` 解析冻结快照内来源。
- `ActivationReceipt`/`TeardownReceipt`：区分「已接收 / 已激活 / 已卸载」的可观察终态；P2 起扩展「已应用 / 已呈现」。
- `ExtensionStatus`（`list()`，同步宿主与 worker `ExtensionUiHandle::list()`）：来源版本、活动代、命令表与撤权标记。
- `UiProjector::consume_declaration_resets(&mut node)`（P4）：`(reset #t)` 在 Applied 处一次性执行并消耗，重投影幂等。
- 命令名空间：`扩展 ID + 局部名`；同 ID 重复激活按代际冲突拒绝。

### 4.3 包清单（P1 字段冻结）

实施采用 S 表达式清单（与扩展语言同一 reader，零新增解析依赖；原拟 TOML 因 `toml` crate 不在依赖图而调整）：

```scheme
(uix-extension
  (schema-version 1)              ; 固定为 1
  (id "text-tools")               ; 稳定身份：小写字母/数字/连字符/下划线，≤64 字节
  (version "0.1.0")               ; semver（x.y.z）
  (language r7rs-small)           ; 语言标识（符号或字符串）
  (entry "main.scm")              ; 包内相对路径，.scm 后缀
  (capabilities documents-query)  ; 声明宿主能力名；实际 = 声明 ∩ 宿主端口
  (state-schema-version 0))       ; P3 启用
```

未知字段、路径逃逸、重复身份、非 semver、入口缺失在包冻结或准备期拒绝。

### 4.4 类型映射（跨边界 owned 值）

| Rust `ExtensionValue` | Scheme | 限制 |
|---|---|---|
| `Bool` | boolean | — |
| `Int(i64)` | integer | 超出 i64 的精确整数以字符串拒绝并指明用宿主端口 |
| `Float(f64)` | flonum | NaN/Inf 拒绝 |
| `Text(String)` | string | 受字符串配额 |
| `Symbol(String)` | symbol | — |
| `List(Vec<ExtensionValue>)` | list | 深度与计数受限 |
| `Bytes(Vec<u8>)` | bytevector | 受字节配额 |
| `Null` | '() | — |

拒绝：有损窄化、循环结构、record/vector 以外的不透明对象（首期 vector 映射为 List）。闭包、continuation、环境、参数对象不跨边界。

### 4.5 错误分类（类型化结果）

`ExtensionError` 至少区分：`Package`（清单/结构）、`Parse`、`Unsupported`（标准支持缺口）、`Incompatible`、`CapabilityDenied`、`Argument`、`Quota`、`Cancelled`、`Timeout`、`StaleGeneration`、`Migration`、`HostFailure`、`EnginePoisoned`、`ShutdownIncomplete`。脚本域错误（`error`/`raise`）作为 `ScriptError` 附 error-object 信息返回调用方；未处置才走[诊断契约](../platform/diagnostics.md)。引擎 panic 一律 `EnginePoisoned`，该实例弃用不复用。

### 4.6 Scheme 宿主库

- `(uix extension)`：`extension-id`、`register-command!`（名、参数 schema 摘要、过程）等 P1 注册面。
- `(uix host)`：宿主业务端口（P1 为只读 `documents-query` 类端口，应用显式注入）。
- 标准库按 §2 矩阵开放；未实现库导入报 `Unsupported`。

## 5. UI 挂载位缺口清单（P2 前置）

| 缺口 | 现状 | P2 需要的形状 |
|---|---|---|
| `AppHandle::update_view` 返回 `()` | 只投递不回执 | 扩展专用提交 API 携带接收/应用回执；不改既有签名 |
| 无挂载位概念 | 整根替换 | 稳定挂载位 ID + 扩展提交仅限自有区域 |
| 动态 widget 登记 | `.uix` AOT 编译期 | 运行时受控 widget 白名单（容器/文本/输入/按钮/列表首期），与 AOT 共享语义定义而非共享编译器 |
| 事件回扩展队列 | 事件止于 Rust 回调 | 挂载位事件经既有捕获/冒泡后转 owned 事件入扩展队列，代际校验 |

## 6. 资源上限初值（P1 冻结，按参考负载修订）

| 项 | 初值 | 违反处置 |
|---|---|---|
| 源码总字节 | 256 KiB/包 | `Package` 拒绝 |
| reader 深度 / 求值帧深 | 1024 / 8192 | `Quota` |
| 燃料（求值步） | 5×10⁶/次调用 | `Quota` |
| 单次分配 / 字符串字节 | 1 MiB / 1 MiB | `Quota` |
| 存活字节（sweep 后） | 4 MiB/实例 | `Quota` |
| GC 触发阈值 | 4096 次分配 | — |
| 墙钟：prepare / 命令调用 | 5 s / 1 s | `Timeout` |
| 在途命令 / 注册命令 | 16 / 64 每实例 | `Quota` |

参考负载（实施计划 §测量）：无窗口纯逻辑扩展（P1 门禁）、含输入/列表/异步的交互扩展（P2 门禁）、多实例突发与连续替换（P3 门禁）；P1 交付时记录冷装载与命令端到端延迟，测量档案进[性能记录](../性能/README.md)。

## 7. 验证方式

- 标准语义经公开装载/调用入口提交规范程序验证，出处标 R7RS 章节；不为私有 reader、求值机或目录结构另设项目测试。
- 宿主边界（配额/取消/超时/能力/类型拒绝）用真实输入与公开失败结果验证，落在 `tests/extensions_public_api.rs`。
- 能力关闭侧编译失败 doctest 落在 `capability_compile_contract.rs`。
- 真窗行为经 `scripts/agent_client.py`（P2 起）；本阶段不涉窗。

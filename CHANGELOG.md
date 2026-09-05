# 变更记录

这里记录 UIX 闭源版本中已经发生、会影响使用方、交付物或验证方式的变化。变更条目不表示版本已发布，也不承担任务、负责人、阻塞和实时完成度管理；正式发布与交付事实见[交付与许可](docs/产品/交付与许可.md)。

## 0.0.8（进行中）

### 编辑器选区、键盘动作与定位容器

- Input 失焦保留逻辑选区；WrapSelection 包裹后保留内部选择，无选区时将光标放在成对标记之间，继续输入不会落到文末。
- 应用内自动化按所属窗口逻辑焦点执行。Agent 按键补齐 Down/Up，只要一段被消费或观察即成功；UIX Lang 键盘观察器保留冒泡事实，兼容原有输入行为。
- 树级事件分发汇总中间祖先的 Bubbled 结果，避免快捷键已执行却在冒泡末端被判为无人接收；公开事件注入回归区分已观察、已处理与无人处理。
- absolute/fixed 使用自然测量，避免 Flex grow 的零 basis 把自动尺寸容器压成零；固定尺寸、包含块与双边拉伸保持原契约。
- UIX Lang 的 top/right/bottom/left 数值属性映射到既有定位接口，允许状态或记录坐标驱动重排，绘制与命中同步。

### 动态文本、终端声明行为与验证范围修正

- 动态 Text 的行高、对齐、字体族、字重、装饰及盒样式进入实际绘制；
  文本测量按与绘制相同的主题优先级解析字号，窗口主题在布局前同步，
  字号令牌变化重新布局，色板变化保持只重绘。
- Terminal 接收公共尺寸和 Flex 布局声明；原位更新保留输入与历史。
  颜色及文本段变化按完整作者配置触发绘制，空段不再截断后续输出。
- 删除 29 个文档消费者中只核对手写围栏标记的运行测试，保留示例编译；
  动态文本内部换行单测移除，必要回归通过公开组件行为验证。
- 公开行为测试入口显式启用 `test-harness`；文档区分定向行为测试、
  示例编译与发布全量验收。跨平台 CI 跳过纯文档和仅测试改动，
  Linux/Windows 不再于同配置构建前重复运行编译检查。

### 修复：Tooltip 声明尺寸生效与容器内悬停目标重定向

- 背景：For 物化行内（目录树箭头、行内收藏）的 `<Tooltip>` 悬停无气泡、
  `@click` 移入 Tooltip 后点击整体失效。真窗口排查确认两个叠加根因，
  均与此前怀疑的「父布局片段过滤」无关（片段集合只有 table 子布局会
  写入，普通 For/虚拟滚动场景从不涉及）。
- 根因一：`ViewAdapter::apply_style` 是按具体组件类型的闭合分派链，
  Tooltip 不在链上，文档声明的 `width`/`height` 被静默丢弃，触发区停留
  主题默认 80×28；横向行内超宽命中框压过后继兄弟（后声明优先命中），
  命中与点击被兄弟抢走。修复：Tooltip 内核新增 `apply_view_layout_style`
  消费显式尺寸（与 Input/Select 同款窄入口），`apply_style` 链按 feedback
  feature 门控登记 Tooltip 分支；`sync_from` 携带尺寸，reconcile 不丢。
- 根因二：PointerMove 悬停快路径只验证「指针仍在当前悬停目标的 hit
  frame 内」就跳过重命中。容器型目标（侧栏、滚动视口等）的 frame 覆盖
  整片区域，重建时序可能让当初的命中停在祖先容器，此后指针在 frame 内
  的每次移动都不再重跑命中，更深的可交互后代（Tooltip、行内按钮）永远
  收不到 PointerEnter。修复：快路径收窄到「指针位置不存在更深命中」的
  目标（`hit_test_children=false` 或无子节点）；容器目标每步移动重跑
  完整命中，叶目标（按钮、滑块等高频悬停场景）保留快路径。
- 回归测试 `tooltip_for_row_hit_test_public_api.rs`：静态/For 行内/
  ScrollView+For 行内 Tooltip 的声明尺寸断言与点击命中、容器悬停目标
  重定向四项；分别验证过无对应修复时必败。

### 组件：新增终端组件族（terminal capability）

- 新增 `Terminal` 终端视图组件：上部为虚拟化滚动输出缓冲区，底部为固定
  提示符输入行；支持完整字符级编辑（方向键、Home/End、Backspace/Delete、
  光标处插入与粘贴，拒绝控制字符）、`↑`/`↓` 命令历史漫游（上限 100 条）、
  `Escape` 清空草稿；输出默认跟随底部，用户上滚即停止跟随、回底恢复。
- 公开模型：`TerminalLine`（着色文本段集合，`plain()` 输出纯文本）与
  `TerminalColor` 语义色板（Default/Muted/Primary/Success/Warning/Error/
  Info），色值全部经主题 token 解析；组件视觉几何由同目录 UIX 声明唯一
  拥有。
- 命令执行属于应用域：回车提交非空命令后组件清空草稿入历史、调用
  `on_command` 回调并发出 `submit` 语义事件；组件不执行命令、不回显、
  不解析 ANSI 转义序列，输出缓冲由应用拥有并自行截断。
- capability 门控贯通三层：主 crate `terminal` feature、uix-derive 转发、
  uix-lang-compiler 能力登记（`capability!` + `component!("Terminal", …)`）；
  关闭后公开门面、快照变体与声明式标签一并消失，`<Terminal data={…}
  prompt="…" />` 映射经编译期生成矩阵登记（可用组件 112、能力 10）。
- 快照与协调接线：`SnapshotFields::Terminal`（提示符、输出行纯文本、当前
  输入与历史）、有界无障碍摘要（最近 8 行非空输出与当前草稿）与
  `patch_as!(Terminal)` 原位同步；滚动沿用 `VirtualListScroll` 等高虚拟化
  （新增 `overscan_count` 只读取）。
- 文档与验证：新增使用文档[终端](docs/使用/数据展示/终端.md)（3 个
  `uix-compile` 围栏）、uix-lang 展示组件参考与组件速查条目；
  `tests/terminal_docs_public_api.rs` 从外部消费者视角编译全部围栏，
  编译器侧新增 Terminal codegen 契约测试（生成、缺省、拒绝路径）。

### 界面：动态文本在有限约束宽度内自动换行

- 背景：uix-lang `<Text>{插值}</Text>` 编译为 `DynamicLabel`，其测量按
  无限宽估算、绘制走单行 `draw_text` 定点路径；播放器等消费方标题超出
  可用宽度时只能横向溢出压到兄弟控件，没有任何换行途径（静态
  `Typography::Paragraph` 支持换行，动态文本此前是框架内唯一无法换行
  的文本入口）。
- `DynamicLabel::measure` 在布局约束给出有限宽度（或样式显式宽度）时
  改用与真实布局同一套 `estimate_text_metrics` 折行估算：行数随折行
  增长（行盒保持 1.5 倍字号契约），宽度在折行后取约束宽度；无显式
  高度时高度随行数增长。无限约束保持原单行内在尺寸。
- `DynamicLabel::render` 改走 `draw_text_wrapped`：以扣除 padding 后的
  frame 矩形为换行边界绘制，首行顶左位置与原单行绘制一致；文本不变
  时视觉无差异。
- 换行触发条件与 `Container::child_measure_constraints` 既有契约一致：
  只有直接父容器具备显式宽度（或有确定宽度的列）时子文本才拿到有限
  测量约束，宽度不定（无宽列、行内单元格）的动态文本维持单行溢出的
  原行为，既有布局零迁移。
- 新增单元测试覆盖宽松约束单行、有限约束折行（高度增长且宽度不越界）
  与显式宽高固定尺寸三条路径。
- 验证：45 个公开 API suite、90 项测试与诊断处置契约审计全部通过。

### Lang 编译：State 字段显式 `get()` 与裸字段读值同义归一（修复）

- `Widget` 内 `{field.get()}` 此前会把字段改写为句柄 `get()` 调用、再叠加作者
  显式 `.get()`，生成双重解引用并编译失败；现与裸 `{field}` 读值归一为单一
  `State::get`，方法链（如 `{field.get().len()}`）按公开 API 语义保留。
- 新增 codegen 契约测试锁定单层与嵌套条件分支的生成形状。

### Lang 绑定：键盘事件新增 `$event.mods` 修饰键载荷

- `@keyDown` / `@keyUp` 的载荷在既有 `$event.key` / `$event.code` 之外新增
  `$event.mods`，编译器把事件 `mods` 解构为稳定局部并改写成员访问；
  事件登记表同步收录 `mods` 字段，未知字段诊断的建议清单同步更新。
- `KeyMod` 新增 `has_ctrl` / `has_shift` / `has_alt` / `has_super` 便捷判断，
  与公开常量位语义一致，供受限表达式直接消费。

### 语义动作：`WrapSelection` 与应用内自动化入口

- `SemanticAction` 新增 `WrapSelection { prefix, suffix }`：对文本输入组件，
  有选区时用前缀与后缀包裹选中文本，无选区时在光标处插入成对标记；光标与
  选区按字符索引换算字节边界后替换，`Input` 的语义能力宣告同步收录。
- `TestApp::perform` 改走后台放行的语义入口：测试替身无窗口前台概念，文本
  合成输入不再被前台焦点策略拒绝；真实窗口的 Agent 授权链路行为不变。
- `AppHandle::perform_automation_action` 新增应用内自动化入口：按 automationId
  定位唯一节点并执行语义动作，结果经通道回传；不经过 Agent 策略与代际校验。

## 0.0.7（2026-09-04）

### VirtualScroll 行模板作用域完整化（UIX Lang 编译修复）

- `VirtualScroll` 的惰性行工厂现会在闭包内自包含建立行级 For 实例路径、
  逐迭代准备语句与事件捕获克隆；行根内嵌套 `<For>`、引用当前行字段的
  数据源及字符串字面量比较不再生成作用域外符号。
- 行身份按绝对索引生成，跨物化窗口重建保持稳定；准备语句引用的组件级
  声明在闭包外克隆遮蔽，不夺走组件体后续使用的所有权。
- `<For key={...}>` 的纯表达式子语言明确拒绝字符串字面量，需改用数据项
  的字符串字段；新增 codegen 契约测试，主演示同步覆盖嵌套 For 与字面量
  比较场景。

### 布局伸缩、双向滚动与可变行高收敛

- Container 与 Space 不再用组件视觉默认值覆盖子项公开的 `flexShrink`
  声明；无显式主轴尺寸且不参与 grow 的容器保持由内容撑开。
- ScrollView 同时求解两轴滚动条固定点，保留双向滚动内容的固定自然宽度，
  避免一侧沟槽触发另一轴溢出时漏判或把内容压入滚动条下方。
- `VirtualScroll::variable_height()` 改用已物化行子树的真实自然尺寸布局，
  固定行高模式继续避免无谓的子树重复测量；公开 API 测试覆盖四类回归。

### 诊断系统覆盖整改（框架侧 reporting 接入与死代码清理）

- 背景：诊断系统审计发现框架侧 reporting 未接入——生产代码
  `Diagnostics::report` 仅 pending drain 边界 1 处，`ReportOrigin::framework`
  标注 dead_code「供尚未接入的调用点使用」，GPU registry 的
  `PendingFailureQueue` 参数在三平台 factory 全部 `_pending` 丢弃；图形帧
  失败、窗口操作失败与字体初始化失败只走 tracing，宿主 `snapshot()` 完全
  不可见。另有 14 个零引用 `Errc` 变体、`WindowsTextInput::default()` 孤儿
  队列与若干错误吞没点。
- 框架侧 reporting 接入：检查式窗口操作失败（主窗、副窗与创建/清理边界
  共 15 处）与启动字体初始化失败经 `report_with_origin` +
  `ReportOrigin::framework("window"/"fonts", ...)` 进入框架报告；图形恢复
  序列放弃后的终态失败（`has_terminal_failure`）按窗口只报告一次
  （`framework("graphics", "terminal_failure")`），瞬态图形失败仍由
  RecoveryDriver 有界恢复收敛、不打扰报告存储（防恢复期风暴）；
  `ReportOrigin::framework` 摘除 dead_code。
- GPU registry 死参数链删除：`GraphicsContextFactory` 与
  `try_create_gpu_recipe_with_queue`、bootstrap、恢复 rebuilder、三平台
  factory 函数的 `PendingFailureQueue` 形参与传递全部移除——当前 GPU
  失败模型为同步 Result → RecoveryDriver → 终态报告，不存在回调投递面，
  保留死参数是误导；`create_preferred_engine` 的 `diagnostics` 形参随之
  移除（crate 内签名，非公开 API）。
- `Errc` 删除 14 个零构造零转换变体（`BadWeakPointer`、`FileBusy`、
  `ReadFailure`、`EndOfFile`、`NetworkError`、`ConnectionTimeout`、
  `DnsLookupFailed`、`ProtocolViolation`、`TlsError`、`ProtocolError`、
  `ChecksumMismatch`、`DeadlockDetected`、`FutureAlreadySatisfied`、
  `GdiOperationFailed`）；保留数值空洞不重排既有判别值。外部若引用这些
  变体需迁移到相邻语义（如 `IoError`、`Timeout`、`PlatformError`）。
- 吞没点整改：GPU 纹理销毁在错误清理路径的失败（10 处
  `let _ = destroy_texture(s)`）改为统一 `warn_destroy_texture(s)` 辅助——
  保留可观测 warn、不传播二次清理错误、不覆盖原始失败；表单
  `validate_pattern` 的非法正则不再静默降级为无规则，构造时告警一次。
  Vulkan device-wait 的 `Err(_) => {}` 核实为后续
  `accept_device_wait_for_shutdown` 统一转换（非吞没，未改）；
  `PixelSurface::new` panic 为既有文档化契约（固定尺寸 + `try_new`
  typed 出路，未改）。
- Wayland 未知子对象降级：`event_created_child` 遇到未登记接口/opcode
  时不再 panic 终止应用，改为 `UnhandledWaylandChild` noop 宿主吞掉该子
  对象事件并输出一次含接口名与 opcode 的告警。
- 孤儿队列清理：删除全仓库零调用的 `WindowsTextInput` `Default` 实现
  （其自建 `PendingFailureQueue` 脱离运行时诊断，错误永不排空）。
- demo 宿主最小消费：主演示经 `.diagnostics(DiagnosticsConfig)` 显式配置
  崩溃报告目录（系统临时目录 `uix-lang-demo-diagnostics`），`on_start`
  保存可 clone 诊断句柄，应用退出后输出报告摘要（total/evicted/retained）
  并原子写出复现清单——报告产出首次有真实宿主消费方。
- 文档：架构 diagnostics 文档补 `report_with_origin` 框架侧接线行；使用
  层运行保障文档补框架失败分支的报告语义（瞬态收敛、终态一次、origin
  目标区分）。
- 验证：`cargo test --features agent-control --test "*_public_api"` 全绿；
  主 crate 与 demo `cargo check` 通过；pending 队列的 `Overflowed`/
  `Closed` 状态丢弃核实为设计内语义（溢出经 `InsufficientResources`
  信号浮出、关闭为 teardown 拒绝），未改动。

### 协调与渲染热路径不变量 panic 全部转 typed 降级（失败隔离收尾）

- 背景：上一节整改后仍遗留五处「框架内部不变量」panic/expect——每帧协调
  热路径三处、每帧渲染热路径一处、应用 VirtualScroll 键工厂一处。按
  「失败隔离并继续」原则，这些路径在前提被打破时应降级并保留可观测
  错误，而不是以 panic 终止应用（闪退丢用户现场）。
- 渲染热路径（`draw/scene/layer_tree/render.rs`）：裁剪片段在数量确定后
  消失时不再 `expect` panic，改为返回 typed
  `InvalidState` 错误——该错误沿既有 `render_node` → `RenderOutcome::Failed`
  链进入 RecoveryDriver 有界恢复与终态框架报告，且绝不静默退化为未裁剪
  绘制。
- 协调热路径（`ui/coordination/adapter/mod.rs`）三处：Button 快照读取的
  downcast 失配（理论不可达）回退通用快照协调路径；Button patch 边界的
  downcast 失配保守视为配置已变化强制失效、实际同步交由既有替换路径；
  直接兄弟复用快路中后续兄弟被前序专项协调移除时，放弃快路并把未消费
  声明连同剩余迭代交回完整 keyed/unkeyed 协调（已协调前缀 key 幂等可
  再次匹配），三处均保留一次性 `tracing::error`。
- VirtualScroll 重复稳定键（`ui/coordination/render_handler.rs`）：应用
  键工厂对同一物化窗口返回重复键时，丢弃后续重复声明（该行本窗口跳过
  渲染）并输出只含冲突索引、不含业务标识的错误日志；不再以 panic 终止
  应用。首个键保持原有 keyed reconcile 与组件私有状态所有权语义。
- 验证：主 crate 与 demo 全 feature `cargo check` 通过；
  `cargo test --features agent-control --test "*_public_api"` 86 项全绿。

### 诊断覆盖第三轮：启动终态、独立 Window API 与 IME 全量接线

- 背景：第二轮复查确认主干闭合后，剩余缺口集中在三类——启动终态错误与
  fonts 的接入不对称、独立 `Window` API 整条路径无报告、IME 会话边界
  tracing-only；另有 `ReportOrigin::with_resource` 为唯一未接线的报告
  预留与若干无声明裸露点。本轮全部处置。
- 启动终态接线（8 处）：平台创建、主窗创建、自定义标题栏配置、图形
  初始化全失败（含 CPU 兜底失败）、设置加载、Agent 传输启动失败与副窗
  创建/身份校验失败，全部在终止启动的最终边界进入
  `ReportOrigin::framework("platform"/"window"/"graphics"/"settings"/"agent", ...)`
  框架报告；副窗两处携带 `with_resource("window", id)` 资源身份，
  `with_resource` 摘除 dead_code。Agent 传输专用错误收敛为 typed
  `IoError` 后上报。
- 独立 `Window` API 接线（6 处）：create（平台失败/show 失败/清理
  close）、raise、run close、Drop close 经自有 Diagnostics 句柄进入
  框架报告；`report_center_on_screen_result` 增加 Diagnostics 参数，
  非 `NotImplemented` 的真实居中失败在三个入口（主窗/副窗/独立窗口）
  统一上报。
- 事件终态与 IME 接线：主循环 window action 失败（2 处）、副窗 action
  失败（2 处）、首帧后 deferred show/raise 失败（窗口可能持续不可见）
  进入框架报告；`sync_window_text_input` 增加 Diagnostics 参数，IME
  会话 start/select_target/stop 失败上报
  （`framework("ime", ...)`，五个调用点同步），光标矩形高频更新失败
  保留日志不进报告（防 IME 会话期风暴）。
- 设计边界显式化（保留日志、加注释声明）：GPU adapter Drop 期主动
  retain/forget 的泄漏事实（Vulkan/D3D12）与 teardown 恢复失败——
  adapter 层不持有 Diagnostics 句柄且 Drop 期报告存储随进程消亡，经
  `tracing::error` 观察；widget_runtime 剪贴板失败——UI 树层不反向
  依赖 app 组装根诊断服务；帧通知取消/请求失败——高频瞬态且 fallback
  保持武装；`AppHandle::request_activate` 执行期拒绝——文档声明的
  平台策略行为。
- 零散清理：`tree_events` 两处 `unreachable!()` 补文案（外层事件组分派
  结构性保证）；字体路径/映射探测链（5 处 `.ok()`）与 Wayland 阴影
  缓冲失败补 debug/warn 级错误细节（公开 Option 契约不变）；Picture
  编码器构造失败补 debug 细节；widget_handle 语义重入降级、Windows
  IME 注册表读取、icon 字体 OnceCell 竞争、Vulkan device-wait catch-all
  空臂补设计注释。
- 验证：主 crate 与 demo 全 feature `cargo check` 通过；86 项公开 API
  测试全绿。

### 错误处置判断机制（类型层强制：决策矩阵 + 显式处置入口）

- 背景：三轮覆盖整改中「某错误该进报告、保留日志还是 panic」始终依赖
  人工逐点裁定。本轮把该判断机制化：处置类别编码为类型化入口，决策
  规则固化为架构文档的权威矩阵，新错误处理点不显式选择处置方式就无法
  表达瞬态观察。
- 新增公开 `Diagnostics::observe_transient(target, reason, detail) -> bool`：
  瞬态失败（自愈重试、fallback 保持武装、高频平台噪声）的显式观察入口。
  内置新私有 Module `TransientObservationModule`（`src/diagnostics/transient.rs`）
  按 `(target, reason)` 冷却去重——首条立即发射结构化事件（target
  `uix::diagnostics`，携带 `transient_target`/`reason`/`suppressed_in_window`/
  `detail`），30 秒窗口内重复观察抑制并计数，窗口结束后的下一条携带累计
  数。键为静态字符串对、编译期有界，不违反错误风暴内存上界；不进入报告
  存储。返回值公开去重行为供测试锁定。
- 新增 crate 内 `diagnostics::observe_boundary_error(layer, error)`：无诊断
  句柄层（UI 树、adapter teardown 等 SMC 边界层）的显式边界观察入口，
  取代裸 `tracing::error!`，失败事实携带层标识结构化记录。
- 新增公开 `uix_contract_violation!` 宏：契约破坏 panic 的统一前缀入口
  （`[uix-contract]`），便于日志与崩溃报告检索；既有带文档声明的契约
  panic 保持不动。
- 架构文档新增「错误处置决策矩阵」：按终态性、回调边界、恢复所有者、
  瞬态性、层边界的顺序判定六个处置类别（终态报告/回调投递/恢复/瞬态
  观察/边界观察/契约 panic）及对应类型化入口；明确新错误处理点不得以
  裸 `tracing::error!`/`warn!` 表达任何处置类别。组件清单、SMC 落地边界
  与公开契约表同步；使用层运行保障文档补瞬态观察语义。
- 存量迁移：`WindowDriver` 现持有诊断句柄（构造时注入），帧通知
  cancel/request/presented、pre-present show、native frame cancellation、
  resize 收敛与图形帧瞬态 episode 共 8 处改经 `observe_transient`（获得
  去重）；指针光标更新失败（原逐事件 warn）与 IME 光标矩形更新失败经
  `observe_transient`；`AppHandle::request_activate` 执行期拒绝经
  `observe_transient`；widget_runtime 剪贴板 ×2、Vulkan/D3D12 adapter
  Drop retain ×2 与 Windows DPI teardown 共 5 处改经
  `observe_boundary_error`。
- 新增公开 API 测试锁定去重语义：同键首条发射、窗口内抑制、不同
  target/reason 键独立、瞬态观察不产生留存报告。
- 验证：主 crate 与 demo 全 feature `cargo check` 通过；87 项公开 API
  测试全绿。

### 报告即时通知与瞬态量级可见（发生即感知）

- 背景：诊断覆盖与处置判断机制就位后，「马上知道」还差两块——报告产生
  后宿主只能装 tracing subscriber（全量日志）或事后轮询 `snapshot()`，
  没有「一产生就通知我」的订阅通道；被冷却抑制的瞬态观察只有事件窗口，
  快照无法回看观察通道的量级。
- 新增公开 `Diagnostics::on_report(handler) -> ReportSubscription`：每份
  报告入库的同时（应用 `report` 或框架内部上报），处理器在 report 调用
  线程同步收到该 `ErrorReport`——宿主不依赖 tracing subscriber 也能在
  错误发生的第一时间感知。RAII 句柄 drop 即注销；处理器内部的嵌套上报
  照常入库但不再分发（thread-local 通知递归抑制）；处理器 panic 被隔离
  为 2^n 限流 emergency 输出，不影响报告与其他订阅者。新增私有 Component
  `reporting/notify.rs`（`ReportNotifier`/`ReportSubscription`，SMC：注册/
  注销/分发与发射、存储互不访问）。
- `DiagnosticsSnapshot` 新增瞬态量级字段与 getter：
  `total_transient_observations()`（累计观察次数，含被抑制的重复）与
  `suppressed_transient_observations()`（被冷却窗口抑制的次数）——宿主
  可随时回看观察通道活动；`TransientObservationModule` 新增对应原子计数。
- demo 宿主接入即时订阅：主演示 `on_start` 注册 `on_report`，任何框架或
  应用报告产生的同一时刻实时输出 code/severity/origin/operation/summary，
  运行中错误无需事后翻快照即可见。
- 文档：架构文档公开契约表补 `on_report` 行、SMC 边界表补 notify
  Component、决策矩阵补即时性双通道说明；使用层运行保障文档补订阅示例
  与处理器契约（同步线程、防递归、panic 隔离）。
- 新增公开 API 测试：即时同步通知与分类/来源字段、RAII 注销后停止通知、
  嵌套上报恰好分发一次（无递归）且全部入库、瞬态计数（累计/抑制/不进
  报告存储）。
- 验证：主 crate 全 feature `cargo check` 通过；89 项公开 API 测试全绿。

### 诊断处置契约静态审计（防止绕过诊断系统的错误报告）

- 背景：处置判断机制（决策矩阵 + 类型化入口）就位后，其强制力仍依赖约定
  ——新代码可能退回裸 `tracing::error!`、无声明 panic 或静默吞没。本轮把
  矩阵编码为可执行检查，接入验证流程形成硬门禁。
- 新增 `scripts/diagnostics_audit.py`：扫描 `src/` 生产代码（自动剔除
  `#[cfg(test)]` 与测试/验收 feature 门控块），执行三条规则——R1 错误值
  （短述/错误绑定）的裸 `tracing::error!/warn!` 必须同分支已有诊断通道
  调用或带 `diagnostics-exempt: <理由>` 豁免标记；R2 生产路径
  `panic!/unreachable!/expect` 必须带邻近契约注释，`todo!/unimplemented!`
  一律违规；R3 空体 `Err(_) =>` 吞没必须带理由注释。另输出通道统计仪表
  （report_with_origin / observe_transient / observe_boundary_error /
  panic 家族调用点计数）。违规时非零退出码失败；命令接入 AGENTS.md 构建
  与测试节，并约定改动 src/ 错误处理路径后必须重跑且违规清零后提交。
- 首跑暴露 55 处存量违规，全部按矩阵语义处置而非豁免：GPU adapter
  teardown/降级链（Metal/EGL/WGL/D3D11/D3D12/Vulkan/线程绑定/渲染会话）
  22 处迁移 `observe_boundary_error`（HW→WARP、flip→legacy、Gdi→空
  presenter、字体路径缺失等降级事实获得带层标识的结构化记录）；UI 层
  （自动化快照导出/关闭、feedback 租约更新/挂载 ×4、Lucide 字体、表单
  非法正则）8 处迁移边界观察；主题查询/publish 失败迁移
  `observe_transient`（app 层有句柄）、主题订阅失败升级为终态报告
  （`framework("theme_bus", "subscribe")`，与注释声明的「中止启动并报告」
  对齐）；agent 线程基础设施 4 处与 io/正则错误收敛为 typed
  `IoError/FormatError` 后走边界观察；7 处 panic/expect 补契约注释；
  1 处配置提示加豁免标记。
- 审计终态：`src/` 无绕过诊断系统的错误报告点；通道统计为
  report_with_origin 28、observe_transient 18、observe_boundary_error 49、
  生产 panic 家族 38（全部带契约声明）。
- 文档：架构文档决策矩阵补「静态强制」段（工具规则、豁免标记、退出码
  语义）。
- 验证：主 crate 与 demo 全 feature `cargo check` 通过；89 项公开 API
  测试全绿；`python3 scripts/diagnostics_audit.py --verbose` 违规为零。

### 未处置错误落地检测与编译期丢弃警告

- 背景：静态审计只能覆盖仓库内 `src/`；使用框架的应用程序绕过诊断系统
  丢弃错误（`let _ =`、match 后丢弃）时框架无从知晓。本轮补齐编译期
  警告与运行时落地检测（处置强度经取舍：未按规范以警告提示为主，
  不阻断构建；运行时检测与静态审计兜底）。
- 编译期警告：`Error` 标注 `#[must_use]`（文案指明必须经诊断通道处置），
  框架或下游应用把 `Error` 作为表达式语句直接丢弃即产生编译警告；
  需要更强约束的宿主可自行 `deny(unused_must_use)` 升级为错误。存量
  代码零违规（前序审计清零的直接收益）。
- 运行时落地检测（全覆盖，含应用）：`Error` 内增处置标记（原子布尔）；
  `report_with_origin`、`observe_boundary_error`、新增公开
  `observe_transient_error`（瞬态观察的错误值变体，等价 observe_transient
  并完成处置标记）、pending 入队与 `attempt_recovery` 在消费时递归标记
  全原因链；`with_source`/`with_appended_source` 附加的源错误随父错误
  承担（错误链作为一个观测单位，父未处置时只有父进入观测）。`Clone`
  不复制标记——副本是独立的未处置实例。
- Drop 时仍未处置的错误进入新增 `core::error::unhandled` 模块的全局
  有界去重观测：按 `(code, 消息前缀 64 字节)` 每键一条 debug 事件
  （target `uix::diagnostics`）、键数硬上限 256（超出折叠为溢出键），
  动态消息无法突破内存上界；公开
  `uix::core::unhandled_error_summary() -> (累计次数, 去重键数)` 供宿主
  与测试检视。应用吞掉框架错误不再是静默行为。
- 存量迁移：13 处瞬态观察调用点迁移 `observe_transient_error`（帧通知、
  resize、图形帧 episode、指针光标、IME 光标、主题查询/publish、激活
  拒绝、pre-present show）；删除失去调用者的
  `graphics_failure_diagnostic` 辅助。
- 新增公开 API 测试：未处置丢弃计数、报告/边界/瞬态观察完成处置后
  不计数（含原因链随链）、副本独立承担观测责任（全局状态在专用锁内
  串行断言）。
- 文档：架构文档决策矩阵的强制机制升格为四层（编译期 must_use 警告、
  静态审计、运行时落地检测、类型入口）；使用层运行保障补
  `unhandled_error_summary` 检视说明。
- 验证：主 crate 与 demo 全 feature `cargo check` 通过；90 项公开 API
  测试全绿；诊断契约审计违规为零。

## 0.0.6（2026-09-02）

### WindowControl 图标前景色定制（深色标题栏对比度适配）

- 背景：`window_controls` 组合的三个图标颜色固定为主题文本角色
  （`ColorValue::Neutral(NeutralRole::Text)`），浅色标题栏表面正确；
  消费方在深色标题栏（如深绿品牌栏）上使用自定义标题栏时，深色图标
  在深色表面对比度不足（实测约 1.4:1），且 UIX 样式类不级联、组件未
  暴露图标颜色入口，消费方无公开途径适配。
- 新增公开 `window_controls_with_icon_color(show_minimize, show_maximize,
  show_close, icon_color: ColorValue)` 组合构造器（prelude 与
  `ui::window_controls_with_icon_color` 双路径导出）；声明层同目录 UIX
  的三个 Icon 前景改为注入参数。既有 `window_controls` 签名与默认
  主题文本色外观完全不变（委托新入口）。
- UIX 声明层 `<WindowControl>` 新增专有属性 `iconColor`：接受颜色
  字面量或字符串表达式（`impl Into<ColorValue>`，含 `&str`/`String`）；
  未声明时保持既有 `window_controls` 生成路径与主题文本色，声明时生成
  `window_controls_with_icon_color(..., (#color).into())`。窗口动作、
  无障碍语义与 hover/focus/active 热区外观继续由运行时 `window_chrome`
  模块拥有。
- 文档：通用组件参考第 10 节属性表补 `iconColor`；多窗口使用文档
  `multi-window-control` 围栏示例补深色表面用法。
- 版本对齐：uix-derive、uix-lang-compiler、uix-lang-cli、uix-lang-lsp
  与 uix-lang-gate-consumer 随主 crate 同步升至 0.0.6。
- 验证：新增 WindowControl codegen 契约测试（默认/定制入口分叉、
  `.into()` 转换、子节点与未登记属性拒绝路径）；`multi-window-control`
  公开 API 编译消费者补深色表面示例；框架全量 46 个测试套件全绿；
  消费方（markdown-project-manager）深绿标题栏真窗实测图标前景
  #E7F2ED 呈现清晰（见消费方 0.8 版界面文档验收记录）。

## 0.0.5（2026-09-02）

### MenuBar 横向菜单栏组件（新增导航 capability 公开面）

- 背景：导航组件族只有垂直 `Menu` 与单触发器 `Dropdown`，桌面应用顶部
  的「文件 / 视图」式横向菜单栏没有公开承载，消费方只能用多个 Dropdown
  自行拼装且无法获得互斥切换与统一键盘语义。
- 新增 `MenuBar`（feature `navigation`）：横向顶级入口条 + 每入口下拉
  弹层全部由组件内核自绘。数据模型 `MenuBarMenu { key, label, items }`
  与 `MenuBarItem { key, label, icon, disabled, divider }` 走 keyed 门禁
  （入口 key 全栏唯一、条目 key 菜单内唯一，被拒身份经 `diagnostics()`
  暴露）。
- 交互契约：点击入口开/关，已开状态下点击或悬停其他入口直接切换；
  `Enter`/`Space` 打开或选择，`Up`/`Down` 循环高亮，`Left`/`Right` 切换
  相邻入口，`Escape` 与外部点击关闭；选择发布条目稳定 key 的
  `SemanticKind::Change`。UIX 声明层 `<MenuBar menus={...} @change=...>`
  的 `$event.value` 物化为拥有型 `String`（与 Dropdown 的借用载荷不同，
  可直接写入 String 状态）。弹层登记 `OverlayKind::Popover`，
  `dismiss_on_outside` 关闭与 `SnapshotFields::MenuBar` /
  无障碍快照同步提供。
- 样式桥：入口前景 `color` 与悬停/打开背景
  （`backgroundColor:hover` / `backgroundColor:active`）接入统一样式，
  支持深色标题栏适配；弹层视觉保持组件私有。View 显式 `flexGrow` /
  `flexShrink` 覆盖同步登记。
- v1 边界：嵌套子菜单与 UIX 声明层分隔线构造不在首版
  （`MenuBarItem::divider()` 仅 Rust API）。
- 版本对齐：uix-derive、uix-lang-compiler、uix-lang-cli、uix-lang-lsp
  与 uix-lang-gate-consumer 随主 crate 同步升至 0.0.5，修复首个 v0.0.5
  tag 只改主 crate 版本导致外部 git 依赖无法解析 `uix-derive ^0.0.5`
  的发版缺陷（tag 已删除重打）。
- 修复（随首个 v0.0.5 tag 后补齐）：入口行与已开弹层内的
  `PointerUp` 由内核消费——Agent `click_at` 成对派发按下与抬起，
  此前抬起未处理导致整次点击被误报 `not_interactable`（按下已生效）。
- 验证：编译器 561 项单测（含 MenuBar codegen 生成/拒绝路径与 schema
  计数 111/33）全绿；公开 API 测试 45 文件全绿；demo 导航页新增面板经
  Agent 通道真窗实测——语义树 `expanded` 状态、弹层像素呈现（浮起白板
  与对照区区分）与选择后 `@change` 状态回写闭环。已知观察：Agent
  `click_at` 对自绘弹层坐标偶发 `not_interactable` 误报但事件已送达
  （状态断言证明），交互路径不受影响，待后续 Agent 协议可交互性判定
  修订时一并核对。

## 0.0.4（2026-09-02）

### Agent 协议官方 Rust 客户端（新增公开面）

- 背景：`uix.agent.v1` 此前只承诺线协议与 Python 参考客户端
  （`scripts/agent_client.py`）；Rust 宿主与集成测试需要自行拼装
  JSON Lines、按平台连接端点并手写发现目录定位，消费成本高且易漂移。
- 新增 `uix::app::agent_client::AgentBridgeClient`（feature
  `agent-control`）：与 Python 客户端同源同语义的公开 Rust 消费面。
  能力包括发现文件定位（`discovery_file`）、按进程连接并完成 `hello`
  限额协商（`connect_to_process` / `connect`）、窗口枚举与等待
  （`list_windows` / `wait_for_window`）、语义快照与谓词轮询
  （`snapshot` / `wait_for_snapshot`）、受控动作便捷入口
  （`perform_invoke` / `perform_set_value` / `perform_raw`）与呈现等待
  （`wait_until_presented`）。
- 边界：服务端 IPC 组装仍为私有边界（SMC-06 不变）；报文逐条校验
  `schema` 与 `request_id` 回显，请求/响应受协商限额约束，超限即失败。
- 验证：发现目录布局、发现文件命名与限额协商单测通过；由消费方
  belldandy-agent 桌面集成测试在 Linux 上完成五场景全链路实测
  （发现、连接、语义树、审批决策、冲突保护）。

## 0.0.3（2026-09-01）

### Agent 语义快照声明坐标空间（修复 bounds 与截屏对照错位）

- 症状：AI 使用方读取语义快照 `visible_bounds` 后与 `screenshot` 像素直接
  对照定位部件，在显示缩放（device pixel ratio ≠ 1）环境下出现与纵坐标
  成正比的位置偏差；语义 focus / 点击的动作点因此不中实际部件。
- 根因：`frame` / `visible_bounds` 始终是 logical 客户区坐标，`screenshot`
  始终是物理像素，但快照协议没有声明坐标空间与换算比例。
- 修复：`AgentWindowState` 与 `WindowSemanticSnapshot` 携带渲染 engine 的
  `device_pixel_ratio`；`snapshot` 回复顶层新增同名字段，Agent 文档补充
  坐标空间与换算约定。
- 验证：demo + Agent 通道实测初始页、页面切换、内容滚动与窗口最大化，
  `visible_bounds` 与截屏部件位置一致；DPR 字段正确输出。

### 渲染 CPU/GPU 双路径重复实现收敛

- 对角线性渐变统一为 CPU 局部空间公式，GPU shader 由 lowering 传入局部
  矩形宽高；任意仿射下 CPU/GPU 语义同源。
- 圆角半径规范化推广为 CPU/GPU 全链共享规则；极端半径输入下两个媒体
  轮廓一致，常规输入像素不变。
- 高斯核、离屏 SlotPool、字形 atlas 分配、shader SDF 数学、字形覆盖量化
  与跨帧 coverage atlas 等实现收敛为共享组件；Vulkan 19 个 SPIR-V 变体
  重编并通过 `spirv-val`。

### ContextMenu 零消费者不再弹默认占位覆盖层（交互修复）

- 指针右键命中没有 ContextMenu 消费者的节点时，不再创建框架默认占位
  覆盖层；应用未声明菜单即保持无副作用。
- 已声明 ContextMenu 的语义路由与事件消费保持不变。

### Slider 受控值变更使用节点级窄 Paint 失效（响应式优化）

- `Slider` 的受控值变化改走节点级 Paint 失效，不再升级为整树结构重建；
  滑块几何与语义值仍在同一收敛帧更新。

### Agent 滚动在边界无位移时保持成功语义（自动化修复）

- 语义滚动动作到达内容边界且没有实际位移时，不再误报 `internal`；已处理
  的边界动作稳定返回成功，未找到或不可交互仍保留原类型化失败。
- 新增公开 API 回归覆盖滚动边界、可滚动路径与失败语义。

### 子树作用域 scoped：结构变化不再强制整树重建（框架能力）

- 背景：根 View 构建期读取的 `State` 一律登记为整树 reconcile 依赖，
  任何结构变化都会重跑整个根闭包并全树 diff；子树级变化（如歌词行
  高亮、队列切换）在重界面上会形成周期性整树重排卡顿。
- 新增公开组合器 `scoped(build)`：闭包挂载时执行一次，闭包内读取的
  `State` 登记为该子树节点的局部结构依赖；变化时只重跑本闭包并按
  `reconcile_existing` 语义原位协调该子树。外层根重建照常重跑作用域
  闭包；同帧根协调会吸收并作废作用域请求。
- 实现复用既有嵌套捕获帧、节点租约与组件状态事务；新增树级作用域
  请求队列（合并去重）与帧循环消费，节点移除即随租约解绑。
- 约束：闭包须为可重复纯构建；不得直接返回另一个 `scoped` 节点
  （显式拒绝，需嵌套时中间包一层容器）；高频值仍应走 canvas/
  map_text/组件绑定的窄失效通道。
- 文档与测试：`docs/使用/入门/响应式基础.md` 新增 scoped 一节
  （围栏 scoped-subtree），reactive/view 架构契约同步；内部测试覆盖
  作用域失效不升级整树、根协调覆盖作用域、租约重装语义。

### UIX Lang 补齐消费者侧登记缺口（语言能力）

- 背景：桌面消费者（小贝）按 UIX Lang 迁移界面时实证出五项登记缺口，
  按适配规则形成产品需求，本批统一补齐。
- `Input` 新增 `rows`（最小可见行数，隐含启用多行模式）与 `maxLength`
  （后续用户输入的最大字符数）属性，均接受 `usize` 整数字面量或受限
  表达式，映射公开 `rows`/`max_length` 构建器。
- `Select` 与 `Checkbox` 新增 `@change` 事件：选中值或勾选状态提交后
  触发，`$event` / `$event.value` 是当前值的文本借用（Checkbox 为
  `"true"` / `"false"`），映射公开 `on_change_fn` 入口。
- `Tag` 新增 `semanticColor` 属性：绑定 `TagColor` 表达式走公开预设
  语义色入口，与 `color`（具体色）互斥，同时声明得到编译期诊断。
- 同步 `docs/uix-lang/参考`（组件属性表、示例围栏与事件登记表）与
  编译器生成快照测试。

### TestApp 公开系统事件注入（测试能力）

- 症状：test-harness 的 `TestApp` 只暴露语义动作与三个硬编码便捷事件，
  文件拖放、指针序列等交互在无窗口测试中无法端到端驱动。
- 改进：新增 `TestApp::dispatch_system_event(&SystemEvent)`，前后各执行
  一次收敛并返回树级 `EventResult`；坐标为逻辑客户区坐标，与语义快照
  同空间。视口变化仍应使用 `resize()`（裸 `Resize` 不更新根 frame），
  拖拽手势应由 PointerDown → PointerMove → PointerUp 序列合成。
- 新增使用文档 `docs/使用/框架设施/测试驱动.md` 与公开 API 测试
  `tests/test_harness_event_injection_public_api.rs`（FileDrop → Upload
  队列端到端）。

### 调试 HUD 支持折叠与拖动（可用性改进）

- 症状：统一调试模式的帧 HUD 固定锚在窗口右上角且不可收起，遮挡其下方
  的应用内容，悬停检查等调试场景需要的信息反而被 HUD 自身挡住。
- 改进：新增逐窗口 `DebugHudState`（`uix::draw::debug`）。开启调试模式后，
  主窗口 HUD 标题行右侧出现折叠按钮（`-`/`+`），折叠为紧凑药丸（仅显示
  「UIX DEBUG ~fps」）；按住标题行或药丸本体可拖动，位置逐窗口记忆并始终
  钳制在窗口内。落在折叠按钮与标题条上的指针事件由 HUD 消费，不再穿透到
  应用组件；展开面板正文区保持点击穿透。副窗口与未接交互的路径保持固定
  右上角形态。
- 关联：`FrameRenderInput` 新增 `hud` 字段；`Ctrl+Shift+D` 与调试模式语义
  不变。

### 发布状态

- 本版本于 2026-09-01 通过私有 Gitea Release 发布，附带从同一精确 tag
  确定性构建并独立校验的 Linux x64 与 Windows x64 内部制品。
- `v0.0.3` 精确指向提交 `33f8fbccdaac94544e4b3e950d219e1aff44fae1`；
  自 `v0.0.2` 起累计 13 个提交，发布前 45 个公开 API suite、85 项测试全部
  通过。
- Linux x64 制品为 34,661,176 字节，SHA-256 为
  `d616b9b10091e2ad8735b67df27bf923f95231e2cdfb8ef06e20a3ef1181a2e0`；
  Windows x64 制品为 34,584,126 字节，SHA-256 为
  `6542b76f377c818034b75caaab8c97975bbf69ba9eaf6ac2ccf394bd29b76d5e`。
  两个平台均完成连续两次相同摘要的候选构建、上传后回下载和独立校验。
- Linux 制品在 Linux x64 宿主原生链接；Windows 制品通过冻结的
  `cargo-xwin 0.23.1` 与 LLVM 交叉链接为 PE/COFF。交叉链接不替代当前
  提交在真实 Windows 主机上的窗口、GPU、输入与 Agent 运行验收。
- macOS 不在 `0.0.3` 发布范围内。

## 0.0.2（2026-08-31）

> 发布序列重整：0.0.2~0.0.8 的碎片版本标签与 Release 已收敛删除，本条目
> （原 0.0.8 草稿）作为收敛后的正式 v0.0.2。以下 0.0.3~0.0.7 历史小节保留
> 作为当时交付事实的记录，其对应标签已不存在。

### 2026-08-30 Input 聚焦边框局部重绘残留修复

- 症状：文本输入框聚焦瞬间边框宽度不一致——上边 2px、其余三边 1px。真窗
  截图逐像素取证：聚焦首帧上边为整数对齐的 2px 新聚焦边框，其余三边为上
  一帧 1px 旧边框残影（外侧伴约 9% 亚像素光晕）；随光标闪烁等后续重绘自
  愈，稳态四边均匀 2px 正常。
- 根因：Input 的失效矩形取 `dirty_rect(frame)` = frame 本身（组件未声明
  `draw_margin`），而聚焦边框以 frame 边界为中心线描边，2px 线宽向外溢出
  1px 加 AA 过渡；聚焦换宽触发的局部重绘中 clear 与 clip 均以 frame 为
  界，外溢带既不被清除也不被重绘，残留上一帧旧边框。各边 clear/clip 取整
  方向差异使顶边外带恰好被新绘制覆盖、其余三边残留，呈现为宽度不一致。
- 修复：Input 实现 `draw_margin`（= max(borderWidth, focusBorderWidth)/2
  + 1px AA 余量），失效矩形覆盖边框全部绘制像素；与 0.0.7 已统一的
  dirty_rect 溢出复绘门控配合，clear、clip 与绘制三者边界保持一致。
- 验证：真窗 Agent 桥聚焦前后连拍 20 帧逐像素对比——修复后聚焦首帧四边
  即为均匀 2px 纯色、无光晕残留，与稳态一致；新增公开 API 回归测试
  `input_focus_border_draw_margin_public_api`（textarea/单行 dirty_rect
  外扩契约 + 真实聚焦路径 frame 包含断言）全绿；workspace 53 个测试目标
  全部通过。

## 0.0.7（2026-08-30，已删除碎片标签）

### 2026-08-30 UIX Lang 独立语言服务器（uix-lang-ls）

- 新增工作区成员 `uix-lang-lsp`：UIX Lang 语言服务器独立实现（stdio
  LSP，二进制 `uix-lang-ls`）；原 `uix-lang-cli` 内嵌的最小 LSP 移除，
  `uix lsp` 子命令委托同一实现，两个入口共享一套会话与特性代码。
- 语言事实不复制：诊断、schema、格式化、源码图与语义 IR 全部经共享
  `uix-lang-compiler` 公开命令；`CompilerSession` 阶段缓存继续承担
  跨请求复用，语义与失败语义与 `uix check` 一致。
- 相比原内嵌实现的增量：上下文感知补全（标签 / 属性 / `@` 事件 / 样式
  属性 / `#token` / 样式类 / 顶层指令）、语义 IR 悬停（组件、事件载荷、
  State 句柄位、capability 门禁）、导入闭包内的定义跳转与全词边界引用、
  文档大纲、`workspace/didChangeWatchedFiles` 缓存失效后立即复核仍打开
  的根，以及修复编辑器位置换算（原 `word_at` 将 UTF-16 列当字节偏移，
  中文等非 ASCII 行定位错误）。
- 新增使用文档 [语言服务器](docs/uix-lang/指南/语言服务器.md)：启动方式、
  编辑器接入、能力清单、可见失败与安全边界。
- 新增 Zed 编辑器扩展 `editors/zed/`：注册 `.uix` 文件类型并通过
  `zed_extension_api` 适配 `uix-lang-ls`（服务器二进制经用户 settings
  `lsp.binary` 提供）；已在 Zed 1.18 端到端实测（扩展加载、`.uix` 语言
  关联、语言服务器拉起）。
- Zed 扩展新增 tree-sitter 语法高亮（`grammars/uix`）：覆盖标签、属性、
  `@` 事件、字符串、数字、注释、`@import/@export/@theme/@keyframes`
  指令、样式属性、伪类与 `#token` 主题引用；以真实 `.uix` 样例
  （demo 全部页面、全部 widget 声明、导入 fixtures）回归零解析错误，
  并在 Zed 1.18 截图验收高亮渲染。

### 2026-08-30 布局收敛与动态文本修复（uix-app#91）

- 修复数据更新后 For 重建卡片内 `dynamic_label` 文本不渲染的核心缺陷
  （admin/uix-app#91）：布局收敛循环存在亚像素浮点噪声驱动的伪变化，
  每帧打满收敛上限退出，最后一轮容器移动失去后续子孙 arrange，子树
  frame 与父 clip 脱节，文本被自身容器裁剪不可见。
- `normalize_layout_rect` 新增千分之一像素亚像素吸附，消除求和顺序噪声
  导致的伪布局变化，收敛循环可真实达成稳定。
- 收敛循环退出（含打满上限）后补一轮自上而下 arrange，保证容器移动
  总有配套的子节点位置发布；防御未来新增移动路径再次产生父子脱节。
- 移除 0.0.6 平台 facade 重构后遗留的 native 测试替身死代码
  （`native::test_harness`，其引用的系统服务 trait 已不存在，且无测试
  消费方）；`test-harness` feature 现在只提供 UI 层 `TestApp`。
- 新增公开回归测试 `for_dynamic_label_update_public_api`：覆盖同 key
  数据更新、整组替换与混合追加三种 For 场景的动态文本可见性。
- UIX Lang 随根 crate 升级到 `0.0.7`，没有新增语言语法、组件登记或生成映射。

## 0.0.6（2026-08-30）

### 2026-08-30 Agent 后台输入契约

- Agent 语义动作新增专用后台输入入口：连接、调用者与动作策略门禁通过后，
  `set_value`、`insert_text`、切换、步进和选择动作可在目标窗口未取得操作系统焦点时
  走受控的 Agent 事件分发；普通应用输入仍保留前台焦点门禁。
- 修复前协议协商会声明 `actions_without_focus=true`，但受控 `Input` 的合成键盘与
  `TextInput` 仍走普通分发，最终把 `NotHandled` 错误折叠成
  `internal command failure`。修复保持 Agent 传输、策略与 UI 语义三个边界独立。
- UIX Lang 随根 crate 升级到 `0.0.6`，没有新增语言语法、组件登记或生成映射。

### 验证边界

- 39 个非空公开 API suite、71 项测试与 `agent-control` 构建通过，无新增 warning/error；
  另有 1 个公开目标命中 0 项，不计为通过项。
- UIX OS 工作空间在四个均未聚焦的真实 Wayland surface 上，通过 UIX Agent 连续完成
  中文 `set_value`、状态快照、提交和结果呈现；宿主未提供 Agent1 服务时明确显示
  `ServiceUnknown`，不伪装执行或自动重试。该运行证据不替代其他平台验收。

### 发布状态

- 本版本已于 2026-08-30 通过私有 Gitea Release 发布。`v0.0.6` 精确指向提交
  `27201f8e915f9218ad915c260bb57125067bfdba`。
- Linux x64 内部制品已从干净 worktree 确定性构建并完成构建器校验、独立校验、
  Release 上传和重新下载逐字节核对；容器大小为 34,616,249 字节，SHA-256 为
  `f43e700cad4e4c3aa1602ef4c65e05146969ce6cfd3bd4ad79b1db0139e22898`。
  Windows x64 制品仍须在对应环境从同一 tag 构建、校验和追加。

## 0.0.5（2026-08-30）

### 2026-08-30 Input 布局契约

- `Input` 现在和其他 View 组件一样消费公开 DSL 的显式 `width` / `height` 与
  `flex_grow` / `flex_shrink`；尺寸只进入布局边界，编辑、IME 与视觉状态仍由 `Input`
  私有持有。修复前 `.width(...)` 与 `.flex_grow(...)` 会停留在 View 样式、实际输入框
  始终使用固有宽度。
- 新增外部消费者布局回归，分别锁定显式 180px 宽度和 320px 行内扣除 40px
  同级空间后的 280px 弹性宽度。UIX Lang 随根 crate 升级到 `0.0.5`，没有新增
  语法、组件登记或生成映射。
- 内部发布构建器现在可以从不跟踪 `Cargo.lock` 的干净源码开始：根工作区与 Demo 工作区
  分别解析一次锁定图，后续 metadata、构建和 package 全部使用 `--locked`；只回收
  当次生成的忽略锁文件，不覆盖使用方已有本地锁定图。Linux 与 Windows 构建入口保持同一语义。

### 发布状态

- 本版本已于 2026-08-30 通过私有 Gitea Release 发布。`v0.0.5` 精确指向提交
  `5c73a0d85a18a9c85f48392e45497bc5f83016ac`；发布前 41 个非空公开 API suite、81 项测试全部通过，
  无新增 warning/error。
- 本次 Release 附带从干净源码确定性构建并独立校验的 Linux x64 内部制品，容器大小为 34,614,651 字节，
  SHA-256 为 `b4a6b3b839903230e2c73703bd6178d2b12baef62665fefd5c383329275ffdd1`；从 Release
  重新下载后摘要一致。Windows x64 制品仍须在对应环境从同一 tag 构建、校验和追加。

## 0.0.4（2026-08-30）

### 2026-08-30 字体可靠性

- 系统主字体与 CJK 回退现在必须通过当前 `TextBackend` 的真实布局和可见栅格探针；只有 cmap
  映射、却不能生成 coverage 或 outline 的候选会被卸载并继续下一项，不再让整窗文字静默消失。
- Linux/fontconfig 发现返回有界、有序候选，移除把所有 `OTTO` 静态 CFF OTF 误判成 CFF2
  可变字体的格式猜测；字体格式能力由统一字体服务以真实后端结果判定。应用显式加载的图标与符号
  字体不参与正文探针，保持原有窄用途。
- 更新已锁定的低层依赖下限并迁移 `syn 3`；工作区不再跟踪环境相关的 `Cargo.lock`，内部候选仍由
  构建脚本在干净源码上解析、冻结并校验实际依赖图。UIX Lang 随根 crate 升级到 `0.0.4`，没有
  新增语法、组件登记或生成映射。

### 发布状态

- 本版本已于 2026-08-30 通过私有 Gitea Release 发布。`v0.0.4` 精确指向提交
  `e055eb99f275a005f8894eac10961ce147b7a626`；发布前 40 个公开 API suite、80 项测试全部通过，
  无新增 warning/error。
- 本次 Release 附带从干净 `main` 确定性构建并独立校验的 Linux x64 内部制品，容器 SHA-256 为
  `f7a8d1b142d7faef819ab2b529c41e4c8fd48ad933cfb50b4d37685bb0d4de34`；从 Release 重新下载后
  摘要一致。Windows x64 制品仍须在对应环境从同一 tag 构建、校验和追加。

## 0.0.3（2026-08-30）

### 2026-08-30 窗口生命周期

- `AppHandle` 新增 `is_open` 瞬时会话快照与 `request_activate` owner-thread 激活请求。激活只经逐窗
  main-thread queue 到达原生窗口，不公开 platform backend、surface 或句柄；关闭先发生时稳定返回
  `InvalidState`，请求不会按标题或创建顺序重定向到其他窗口。
- 修复 `WindowControl` 的 Agent / 无障碍 `Invoke`：语义入口现在复用控件持有的类型化窗口动作映射，
  直接进入当前 `WidgetTree` 的窗口动作队列，不再通过合成键盘事件推断最小化、最大化/还原或关闭。
  平台最终状态仍须由窗口生命周期事实确认，不能把动作已入队当作窗口管理器已经执行。
- 多窗口公开文档与外部消费者编译门禁同步覆盖存活观测、关闭竞态和激活请求。UIX Lang 版本随根
  crate 升级到 `0.0.3`，本版本没有新增语言语法、组件登记或生成映射。

### 发布状态

- 本版本已于 2026-08-30 通过私有 Gitea Release 发布。`v0.0.3` 精确指向提交
  `b1fb789224a9aebfcd446587083c23c5e2ca5611`；发布前的外部 UIX OS 真实 Agent 门禁已完成工作空间
  最大化、精确还原、关闭终态、Dock 新身份重开及重复打开不产生重复窗口的验证。
- 本次 Release 附带从干净 `main` 确定性构建并独立校验的 Linux x64 内部制品，容器 SHA-256 为
  `f760b0bb8c903d465b284a61821bdb31aef252fc14eabfb0ac2ecf601e52f4f2`。Windows x64 制品仍须在
  对应环境从同一 tag 构建、校验和追加；现有 Linux 门禁不替代 Windows 真实运行验收。

## 0.0.2（2026-08-30）

### 2026-08-30 交叉编译

- 新增统一桌面目标交叉编译入口与 Gitea Actions 门禁：根库和带 Agent 能力的主演示可检查 Linux x64、Windows x64、Apple Silicon macOS 与 Intel macOS；Linux runner 真实链接 Linux/Windows，macOS runner 持有 Apple SDK 时链接两种 macOS 架构。Windows 非原生宿主固定使用 `cargo-xwin 0.23.1` 与 MSVC x64 target；编译成功不替代目标平台真实窗口、输入、Agent 与 GPU 验收，macOS 仍不改写为 `0.0.2` 当前交付平台。

### 2026-08-29 Agent 控制协议

- 修复 Agent 截屏协议的帧上限矛盾：此前 `hello` 宣称 PNG 可达 32 MiB，但所有 JSON 回包统一受 4 MiB 上限约束，较大的合法截图在 base64 膨胀后会丢失原 `request_id` 并误报 `internal`。现拆分 4 MiB 请求正文上限与约 42.7 MiB 响应上限，`hello.limits` 新增 `max_request_bytes` / `max_response_bytes`（旧 `max_message_bytes` 保留为请求上限别名）；响应超限降级仍回显原请求 ID。仓库客户端同步执行长度、schema、请求 ID 与截屏载荷校验，避免错配回包或无界累积。
- 降低连续 Agent 操作的客户端开销：`scripts/agent_client.py session` 在同一已认证连接上按 stdin/stdout JSON Lines 转发多次请求，只在首行发布一次 `hello`；单次命令改为静默握手并只输出目标回包，不再为每步重复输出完整能力目录。Linux/Wayland release 主演示复测：单次 `list_windows` 中位时延由约 37 ms 降到 17.6 ms、stdout 由 1876 字节降到 392 字节；持久会话内 50 次同请求 p50/p95 为 0.046/0.070 ms，20 次首页↔语言能力页切换到真实呈现完成为 15.0/16.9 ms。本批针对已测得的客户端重启与重复输出开销，不改 UI settle 或呈现语义。
- 新增每用户 `uix.agent.hub.v1` 多应用连接：启用控制面的 UIX 进程以随机 `instance_id` 向固定本地 Hub 长期登记，AI 必须先 `list_apps` 再显式 `attach(instance_id)`，不再按最新 discovery 猜测目标；应用退出或重启后旧绑定终止，不自动切到同名实例。仓库客户端支持 `apps` 与 `--instance`，可信项目配置新增不依赖 MCP SDK 的 STDIO MCP；连接器为每个绑定实例分离 control / wait / media 长期连接，`uix_interact` 合并动作、呈现等待与快照，截屏以权限 0600 临时文件返回，旧直连 discovery 暂作迁移兼容。
- Agent 动作改为显式窗口定向而非前台焦点定向：应用失焦、被遮挡、最小化或隐藏时，语义、指针、按键和窗口动作继续在目标窗口 UI turn 内完成，后台声明协调、布局和语义修订不依赖 surface；普通操作系统输入仍遵守焦点门禁。无可呈现 surface 时截屏与 presented wait 继续返回 `not_presentable`，MCP `uix_interact` 保留动作成功事实并回退语义快照，禁止自动重放或伪造缓存画面。

### 2026-08-29 桌面窗口能力

- 新增公开 `WindowSurfaceRole` 与 `DesktopLayerConfig`，主窗口和次窗口可声明 Wayland background/bottom/top/overlay layer、四边锚点、独占区及键盘交互；普通窗口继续使用 `xdg_toplevel`。桌面角色只在合成器提供 `zwlr_layer_shell_v1` v4 时建立，其他平台稳定拒绝，不静默退化为普通窗口。

### 发布状态

- 本版本于 2026-08-30 通过私有 Gitea Release 发布，本次先附带经过确定性构建与独立校验的 Linux x64 制品；Windows Vulkan、D3D11 与最终 surface 的真实主机验收已完成，但 Windows x64 制品将在 Windows 环境完成最终打包后追加。仅限内部授权使用。

## 0.0.1（2026-08-29）

### 首发能力

- Windows 与 Linux 均纳入 `0.0.1` 首发平台范围；两平台统一以 Vulkan GPU-native swapchain 为首选，Windows 保留 D3D11、Linux/Wayland 保留 EGL OpenGL ES 兼容回退。
- 提供 Rust 声明式 View、State / Computed、应用壳、本地 Settings 与桌面组件体系。
- `demo/` 精简为只包含 `uix-lang-demo` 的独立工作区；主演示以 uix-lang 声明 12 个页面、全部已登记组件、展示数据、界面状态与局部交互，Rust 薄入口只保留应用生命周期、平台资源和 Form 业务提交回调。
- 编译产物配置入仓：根包与 demo 工作区显式声明 release/dev/test profile（体积优先 `opt-level="s"`、fat LTO、符号表全剥离、`codegen-units=1`），不再依赖机器级 cargo 配置；release 的过程宏与构建脚本单独提速编译。
- 默认 feature 集同时启用 `vulkan`、`d3d11` 与 `opengles`；自动选择按 registry 优先级使用 Vulkan（100），Windows 依次保留 D3D11（30）与 OpenGL ES（10），Linux 保留 OpenGL ES（10），有界 GPU 恢复耗尽后再整体回退 Software。
- 支持多窗口、IME、主题、定时与动画、浮层、结构化语义快照及显式启用的本机 Agent 开发预览。
- 提供唯一的 `uix-lang-demo` 全组件演示；原 CLI 与 8 个独立真窗 demo 已移除，对应组件统一在主演示中以 uix-lang 声明。
- uix-lang 能力扩展：组件私有 state 支持类型注解（`u32` / `usize` / `f32` / `i32`）；受控组件与双向绑定属性（`open` / `value` / `checked` / `current` 等句柄位）可直接绑定组件私有 state；数据类属性按值 clone 消费，同一绑定可被多个标签复用；`uix-lang-demo` 对齐 API GUI Demo 全部已注册标签，并按页拆分为 12 个声明式页面组件。
- uix-lang 图表声明补齐类型化气泡数据、基础坐标图多系列、共同高级配置、热力图自定义色阶、四类高级坐标图参考线、仪表盘格式化器与自定义混合系列；集合与回调在生成代码中保持精确公开类型并复用既有运行时绘制契约。
- uix-lang Tabs 登记 `tabPosition` 与 `scrollable`，支持四向标签栏和沿主轴溢出滚动；生成层只传递声明配置，面板子树、具体几何与滚动偏移仍由运行时 Tabs 独占。
- uix-lang Steps 登记 `clickable` 与 `dot`，可声明用户切换策略和圆点样式，同时保持调用方 `State<usize>` 为当前步骤的唯一业务状态来源。
- uix-lang Pagination 登记 `showTotal`、`showSizeChanger`、`simple`、`showJumper` 与 `pageSizeOptions`，显式显示策略可覆盖绑定默认，同时保留分页运行时的交互与归一化所有权。
- uix-lang Anchor 登记 `showInk`，可声明活动锚点指示线绘制策略，同时保持运行时选择/位置缓存与应用滚动所有权边界不变。
- uix-lang Breadcrumb 登记 `separator` 与 `maxItems`，可声明路径分隔文本和折叠阈值，同时保持运行时选择、溢出菜单与测量/命中生命周期不变。
- uix-lang Menu 登记 `mode="inline"`；内联模式固定垂直并始终展开完整递归子树，即使同时声明 `collapsible` 也不会改写调用方 `openKeys`。
- 所有产品与 Demo 图标统一走 `Icon` 组件管线；内置紧凑槽位复用 `Icon::paint_in_frame`，不再以 Unicode、ASCII、本地化字符或独立手绘形状充当图标。
- 捆绑字体固定为 `lucide-static` 1.17.0，并随内部包保留 ISC 与 Feather 派生图标 MIT 声明。

### 2026-08-14 功能扩展

- Windows 系统通知接通：完成通知身份（AUMID）与发送契约（`NotificationService`），并提供真发送运行时验收；Linux（notify-send）与 macOS（osascript）经平台命令发送。
- 富文本内联图片完整生命周期：解析边界、图片资源后台解码轮询、绘制与无障碍快照事实，随 RichText 一起交付。
- 浮层背景模糊：Modal / Drawer 等浮层接入背景模糊策略与区域失效、快照重建，含真窗验收。
- Popconfirm 组合触发器：多触发器组合支持（父级测量、键盘释放捕获与回归覆盖）。

### 2026-08-15 功能扩展

- RichText 主题分隔线与内联图片能力统一进入 uix-lang 全组件主演示，不再维护独立真窗 demo。

### 2026-08-29 缺陷修复

- 修复 Linux/Wayland 客户端窗口阴影四角与参考外观的偏差（上一条目的后续修正）：角部 tile 原取「双带剖面的加权混合」，底角沿对角线在 d=2-3 处隆起至 21.6 再骤降（Windows 参考为 16.9 起单调衰减、淡尾延展到 40px），顶角淡尾只到 8px（参考约 20px），底角圆弧处缺口补画 26% 较真实场偏深呈暗楔形。现角部内部改取模型场本身（两条轴尾部相乘，沿对角单调衰减、淡尾长度与 Windows 一致），接缝 4px 内以 max 组合由对应边带项钉住（接缝处偏差 ≤1.5 alpha 点），缺口补画峰值改取模型场在圆弧对角中点处的取值（底角 26%→17.8%）。KWin（Plasma 6）真窗最小化差分法复测：底角对角线 27.8/19.6/13.0/11.4/9.4/6.7/4.7/3.1/1.6/0.8 单调衰减对参考 16.9/16.1/14.9/13.3/10.6/7.8/5.5/3.9/2.4/1.2（形态一致、无隆起），顶角 d≥3 与参考逐点重合，四边边带不受影响；公开 API 测试 77 用例全绿。已知边界不变：角点 1-2px 处仍受 9-patch 常量边带接缝值约束较参考偏深。
- 修复 Linux/Wayland 客户端窗口阴影与 Windows 平台外观不一致：Linux 侧 `org_kde_kwin_shadow` 使用按 GTK/Firefox 校准的均匀 32px 对称二次衰减剖面（峰值 0.33、无方向），Windows 侧是 DWM 系统有向投影（源矩形下移：顶部短而弱、底部长而强），同一应用在两平台呈现两种阴影；DWM 阴影不可参数化，统一基准取 Windows 实测外观。现以 Windows 11 100% DPI 参考窗（1200×800）截图提取四边剖面，拟合「窗口矩形水平不动、垂直下移的高斯矩形源」可分离投影场（振幅 0.315、σ=18.5px、源上边下移 9.5px、下边外扩 26.5px，四边边缘峰值 9.6/15.9/29.1 对参考 9.9/15.9/28.7），新增跨平台共享 `shadow_profile` 模块作为唯一外观事实源；阴影 tile 改为逐边偏移（顶 33、侧 43、底 69 物理像素，随输出缩放换算，修复旧 tile 不随 scale 缩放的潜在失配）并按该场生成；同时修正 KWin 九宫格纹理朝向约定——patch 纹理起点对应远离窗口一侧，顶/左/底与角部 tile 内容按此反向生成（右带右锚定天然满足），此前朝向不匹配时边带呈外缘亮、贴窗暗的反转。圆角缺口补画从单一 (峰值, 距离) 升级为逐角峰值（sampled ABI 32 字节内重排出 vec4 逐角事实，尺寸不变），四后端（Vulkan/OpenGL ES/D3D/Metal）着色器按 fragment 所在象限取值，Vulkan SPIR-V 同步重编译；发布链路改为 `set_client_shadow_fill`（逐角 alpha 与逻辑衰减距离，快照按 scale 换算）。KWin（Plasma 6）真窗验收以最小化差分法逐像素测量：四边边缘峰值 9.6/14.9/15.7/26.7 对参考 9.9/15.9/15.9/28.7、外扩 33/43/43/69 与参考一致、向下偏置成立，最大化阴影关闭、还原后完整恢复，minimize→restore 多轮不丢；公开 API 测试 77 用例全绿。已知边界：9-patch 边带只能承载一维剖面，边带近角区间无法跟随 DWM 真实场沿边变化，单显示器 scale=1 环境验证，多 scale 换算已实现未实测。
- 修复 Windows 自定义标题栏窗口最大化后内容下方露出黑色横条：`ShowWindow(SW_MAXIMIZE)` 对保留 `WS_THICKFRAME` 的无边框窗口会把外窗四边各扩一个缩放边框（约 8px），上/左/右超出屏幕不可见，底部边框却落在任务栏上缘的屏幕可见区内；`WM_NCCALCSIZE` 又把客户区对齐回显示器工作区，这条底部非客户区没有任何绘制方（无系统标题栏的窗口 DWM 不绘制非客户区），呈现为窗口内容下方一条黑色横条。现最大化 `WM_SIZE(SIZE_MAXIMIZED)` 时把外窗矩形钉到所在显示器工作区（外窗=客户区=工作区，非客户区不再露出）；仅对扩展客户区窗口生效（保留系统标题栏的窗口不钉，避免系统边框退回屏内），外窗已与工作区一致时跳过且客户区尺寸未变不会重入 `WM_SIZE`，还原与窗口 placement 语义不变。`x86_64-pc-windows-msvc` 交叉编译通过，公开 API 测试 40 文件全绿；最大化/还原、拖顶 Aero Snap、多显示器工作区待 Windows 真机复验。

### 2026-08-28 工程收敛（重复机制治理）

- 删除未文档化遗留机制：`uix::ui` 不再重导出 legacy `StateManager` / `TextManager`（状态与样式请使用响应式 `state` 与 `theme::style`，编译器生成的 `WidgetStateStore` 契约不变）；通知组件不再提供 `replace_from_service` / `notify_error_from_service` / `notify_result_error_from_service` 与 `NotificationSource` 协议（文档化的 `AppHandle::notify_error` 与 `replace_from_toasts` 保留）；`PlatformSystem` 端口收窄，移除 `file_dialog` / `keyboard` / `timer` / `notification` / `console` / `file_system` 访问器（功能由 `Platform` 门面提供；文件对话框、系统通知、剪贴板、显示器的公开门面契约不变）。
- 系统信息单一事实源：`ISystemInfo` 的 OS 与内存采集改为委托公开硬件 Provider；macOS 经内部系统信息路径此前返回空版本号与硬编码内存占位值，现返回真实 sysctl 数据。
- Windows 图形错误分类归一：D3D12 复用与 D3D11 共享的 HRESULT 分类，遮挡、桌面访问丢失与显示模式切换恢复为 typed `GraphicsOccluded` / `GraphicsSurfaceLost`（此前落入通用 `PlatformError`，恢复语义丢失）。
- GDI 呈现器像素缓冲校验对齐：短缓冲从静默截断改为与其他 CPU presenter 一致的 typed `InvalidArgument` 错误。
- CPU 参考执行光栅收敛：矩形描边与直角填充统一走 `SoftwareRasterizer`，SrcOver 与 Additive 两种混合模式的抗锯齿语义强一致；微工具群（`fade_token_color`、`paint_elided_text`、`byte_index_for_char`、`expand_rect`、`union_frame_rect`）各自归一为单一实现。
- 主/副窗装配收口：DI 回退解析、应用根工厂包装与会话资源接线收敛为 `window_assembly` 唯一原语；副窗聚焦初值同步树内投影，`next_deadline` 改用注入时钟；调度队列释放收敛为 `queues::release_window_scheduling_resources` 单一序列。
- 浮层几何归一：tooltip / popover / popconfirm 三套 placement 机制与 10 处输入类弹层矩形解析收敛到 `widgets::overlay` 共享模块（`OverlayPlacement` trait + resolve 骨架 + 下拉五件套原语），公开 placement 枚举与各组件 resolve 入口签名不变。
- 双向绑定样板收敛：25 个受控组件的依赖捕获与「相等则不写」写回统一到 `widgets::binding` 原语；各组件的越界 clamp / 取整规则保持原地组件语义。

### 2026-08-15 工程与验证范围

- `cargo test` 只编译运行公开 API 契约测试（`app/core/data/diagnostics/draw/native/ui` 的 `tests/*_public_api.rs`）：移除 38 个非 API 集成测试（`flex_overflow_*`、`uix_lang_*_windows`、`usage_*`、`semantic_actions`、`layout_invariants`、`tessellator_contract` 等）与未被引用的 `tests/unit` 死代码，同步清理 `platform_notification_runtime` 的 `[test]` 表声明；设备丢失/表面丢失真窗注入验收由 demo 的 `--test-graphics-recovery` 页面承担，真窗视觉与通知运行时验收随精简移除。

### 2026-08-17 工程与验证范围

- ui System 的私有组件运行时 Module 从 `component` 更名为 `widget_runtime`，同步迁移内部限定路径与架构文档；公开 `component!` 宏、Component 层类型和 ui 根门面保持不变。
- 盒阴影定义、Rust 样式链与 UIX Lang `boxShadow` 支持可选正负 spread；既有四参数构造器与四段语言语法继续保持零 spread。

### 2026-08-27 工程与验证范围

- 测试面收敛为公开 API 契约：只保留 `tests/*_public_api.rs`（38 个消费者目标），移除非 API 集成测试（分配合同、渲染合同、profile 与回归二进制）、Python 合同脚本与整层 `tests/unit` 单元测试；src 内对应的 `#[path]` 单元挂载全部解除，内联单元测试保留。
- 寄存在 `tests/support` 的特性门控支撑代码回归 crate 语义：`draw` / `native` / `ui` test harness 原样保留供 `test-harness` feature 与公开 API 测试使用；GPU parity harness 从 `tests/unit` 迁移至 `tests/support/native/gpu_parity/` 并保持原挂载语义。
- Cargo.toml 移除已删除目标的 `[[test]]` 声明（4 个 GPU parity 目标与动画分配合同目标）；相关性能决策记录中引用已删套件的命令按登记时点存档处理，不作为现行入口（见性能决策记录索引的使用限制）。
- 项目动态待办停用 Gitea Issue 管理：完成度与任务判定以仓库内文档（docs/）和实际代码为准；既有文字中对 Gitea Issue 的引用按撰写时点存档处理，不作为现行入口。

### 2026-08-27 uix-lang 语言与工具链

- `<Widget>` 成员支持 `@props` / `@state` / `@computed` / `@actions` 块级声明，与字符串属性形式解析等价；同一成员双写两种形式被编译期拒绝。
- 组件专属状态值类型（`CascaderValue`、`Vec<UploadFile>`）从核心白名单迁入 `UiProjectionSchema` 按拥有组件登记并挂接 Cargo capability 门禁；未启用对应 feature 时在生成 Rust 前给出定向诊断（UIX2000 / UIX2001），不再延迟到 rustc 报公开符号缺失。
- `uix scaffold visual <rust文件> <StructName>` 从同模块 Rust 结构体生成可编译的 `<Visual>` 声明骨架：已知标量类型给占位初值，复合类型输出 TODO 提示，消除 `.uix` 与 Rust 双侧手写字段搬运。
- 公开宏文件入口补全消费者证据：`uix!` 递归导入闭包、`uix_app!` 未运行 builder 与 `uix_items!` Visual 静态项由新增的 `tests/uix_lang_public_api.rs` 消费并断言单一静态地址；`uix scaffold visual` 骨架输出增加锁定单测。
- 新增消费者层门禁证据：`tests/uix_lang_gate_public_api.rs` 锁定公开 check 入口的稳定诊断码、阶段、行列与文案快照；工作区新成员 `uix-lang-gate-consumer` 在真实消费者构建中复现双写拒绝与 `CascaderValue` 值类型门禁的 rustc 级定向诊断（输出行携带码、阶段与主 SourceSpan）。

### 2026-08-27 缺陷修复

- Linux/Wayland 客户端窗口阴影从不显示：KWin 只在 `wl_surface.commit` 应用 pending 的 `org_kde_kwin_shadow` 状态并向渲染端广播 `shadowChanged`，shadow 协议自身的 commit 只更新纹理与偏移；原实现在首次 surface commit 映射窗口时阴影对象仍为空、之后才附加纹理做协议级 commit，`shadowChanged` 早已在空状态下消费，渲染端永远拿不到有效阴影。现改为每次启用阴影时重建 shadow 对象，在同一次 `wl_surface.commit` 前完成八块纹理、偏移与协议 commit，关闭阴影保持 `unset` + commit；KWin（Plasma 6）真窗验收阴影正常呈现。
- Linux/Wayland 动画不生效：程序化 `set_size` 与 `resize_notify` 复用同一入口并无条件回发 `WindowResize`，driver 消费后又调用 `resize_notify` 再次进入该入口，形成每帧同尺寸 resize 自激循环；每圈都把 surface 标记为代次变化并重置动画时钟基准，导致动画 `dt` 恒为 0（画面停在关键帧起点）、native frame callback 因代次失配永不被接受、每帧全帧重绘。现按 surface 事实代次去重，尺寸与 scale 均未变化时不再发布 resize（与 configure/scale 路径既有去重约定一致）；Windows 系统驱动 resize 无此回发因此不受影响，两平台动画调度语义恢复一致。
- 声明式透明度动画在文本内容上不生效：View 统一样式与动画绑定写入的 `style.opacity` 只被各组件在背景/边框绘制路径自行消费，文本等其余内容完全丢失衰减。现把节点声明透明度沿声明树进入运行时节点，并在 scene 节点合成通道与 enter/leave 过渡透明度统一叠乘（LayerTree 已有画布级联语义），组件内部不再重复应用；颜色类关键帧/过渡动画由此在同一 Drawing 语义下跨 Windows 与 Linux 表现一致。
- 未聚焦窗口忽略全部指针输入导致悬停与点击类界面反馈不生效：后台或被窗口管理器延迟聚焦启动的窗口上，指针移动、悬停与点击在树层被整体丢弃，表现为按钮悬停变色、按压反馈等动画「不出来」。现按主流桌面语义修正为指针输入（移动/悬停/点击/滚轮）不要求窗口焦点（Wayland 指针输入本就独立于键盘焦点），键盘、IME、剪贴板与文件拖放仍保留焦点要求。

### 2026-08-27 Agent 控制能力

- 窗口动作补齐与语义修正：`pointer_move` 不再把「无组件继续消费移动」误报为 `not_interactable`（悬停事实更新即是动作效果）；新增 `activate_window` 动作（Wayland 复用既有 xdg-activation 激活契约）。
- 语义快照节点新增 `hovered` 事实，`list_windows` 与 `snapshot` 新增窗口 `focused` 状态字段，自动化可直接断言悬停目标与窗口焦点，不再依赖截图猜测界面状态。
- Agent 端点新增 `screenshot` 请求：命令在窗口 UI turn 强制出帧，由 owner-thread 回读边界截取下一次真实呈现帧，以 base64 PNG（物理分辨率）返回，载荷上限 32 MiB；`hello.capabilities.screenshot` 与 `limits.max_screenshot_bytes` 随握手发布，软件回退路径按 `unsupported_action` 明确失败。仓库客户端 `scripts/agent_client.py` 新增 `screenshot` 命令，AI 操作的像素级验证不再依赖系统截屏工具。

### 2026-08-28 缺陷修复

- Linux/Wayland 程序化窗口尺寸变化后阴影错位与呈现冻结（`resize_window` / `set_size` 后窗口停留旧尺寸、顶部与左侧外圈阴影消失而底部右侧保留，最小化还原后窗口不再显示或冻结在旧帧）：事件循环共享层 `wait_event` / `wait_timeout` 在排干已入队 UI 事件后仍无条件进入原生阻塞等待，而主线程 UI turn 内入队的 `UiEvent::resize`（程序化 set_size、unminimize 恢复）没有伴随原生唤醒，事件滞留到下一个外部唤醒（输入、Agent 请求）才被消费；滞留期间客户端已向 compositor 声明新 `xdg window geometry` 与输入区域，swapchain 与呈现 buffer 却停在旧尺寸，KWin 按两者不一致的几何放置阴影 tile 与窗口内容，呈现为部分边阴影被窗口内容覆盖的错位与整窗冻结。现 `wait_event` / `wait_timeout` 排干队列取到任何事件时立即返回处理，程序化 resize 当轮完成 swapchain 重建、geometry 同步与新帧呈现。同时 xdg_shell 不通知 unminimize：主动最小化后的第一个 configure（任务栏恢复、还原请求）此前因模式与尺寸均无变化而不产生任何窗口事件，客户端无从感知窗口重新参与布局，窗口停留在最小化前的最后 buffer；现以最小化记账在下一个 configure 投递 `WindowRestore` 恢复事件并同步共享窗口记账，驱动完整重呈现。KWin（Plasma 6）真窗验收：程序化 resize 即时生效且四边外圈阴影对称完整、空闲后不漂移，最小化还原后窗口以当前尺寸重新呈现。已知遗留（同日晚间复核关闭）：maximize → restore 后曾观察 KWin 不恢复还原几何、窗口以最大化尺寸拉伸显示旧 buffer（当时客户端 ack / geometry / buffer / commit 协议流经 `WAYLAND_DEBUG` 验证合规，判定待 compositor 交互层排查）；单实例环境经 agent `restore_window`、应用标题栏按钮切换与最大化→还原快速连发三类路径复测，KWin `frameGeometry` 与客户端记账逐帧一致、正确还原，未再复现——原观察与并行会话多实例叠加时段重合，属观察污染，按不可复现关闭。

- 局部脏区重绘擦除溢出布局 frame 的覆盖视觉（徽标角标顶部出现一条水平缺口，如开关 Collapse 后下方按钮右上角的计数圆点缺角）：`Collapse` 按契约把 `dirty_rect` 上报为全部面板最大展开的区域（用于清理展开切换残留），该区域底边可以越过自身 frame 落进后续兄弟内容；脏区重绘会在区域内以不透明背景覆盖全部像素后再按节点门控复绘，而 `needs_paint` / `needs_paint_in_viewport` 对非脏节点只按布局 frame 判相交，Badge 角标、阴影等溢出 frame 的视觉恰好落在区域内、frame 却不相交时，节点被跳过，被背景覆盖的溢出像素无人恢复，呈现为紧贴损伤底边的一条水平缺角。现统一两类门控的相交判定：任意节点的可见绘制范围一律使用完整 `dirty_rect`（默认实现即 frame 加 `draw_margin`，仅溢出型组件更大；实现返回空矩形时按 frame 兜底），溢出像素落入脏区时节点随区域一起复绘，非脏节点不再因 frame 不相交而漏画自身的溢出视觉。验证：`cargo test --features agent-control` 公开 API 测试全绿；主演示操作回路（展开/收起、按住头部、悬停、滚动、视口裁切、往返复原）中徽标圆点与整帧逐像素完整，局部脏区行为无回归。

- Linux/Wayland 客户端装饰窗口圆角缺口透出桌面背景：`org_kde_kwin_shadow` 阴影环只覆盖窗口矩形之外，而最终合成着色器按 10 逻辑像素圆角把窗口四角内容挖成透明，缺口处（圆角之外、窗口矩形之内）KWin 不会补画阴影，浅色背景下四个角呈现白色「缺口」。现把客户端阴影环事实（峰值不透明度与外扩距离，随 `WaylandSurfaceMetrics` 与窗口装饰启停同步发布）传入最终 sampled 合成 ABI，着色器在圆角缺口内按同一二次衰减把外圈阴影连续延伸为预乘黑色补画；合成 damage 规划在补画参数生效且脏矩形触及任一角点缺口方块时升级为完整合成，避免局部 `Load` + SrcOver 让补画 alpha 逐帧累积。Vulkan / OpenGL ES / D3D / Metal 四后端 sampled 着色器源与 SPIR-V 同步更新；KWin（Plasma 6）真窗验收四角阴影连续、`screenshot` 回读角点 alpha 与理论衰减一致、局部脏区多次呈现后不累积、最大化自动恢复方形。阴影 tile 参数同步按 GTK/Firefox 客户端装饰阴影剖面校准为外扩 32 物理像素、峰值 0.33 的二次衰减（半衰减约 9px、尾部平滑延伸到 32px 之外），替代原 24/0.28 组合紧贴窗口的硬黑边观感。

- Linux/Wayland 客户端装饰阴影四角出现矩形色块台阶（窗口外圈阴影在四角附近可见硬边矩形明暗分块，角部区域比相邻边带深，接缝处呈楼梯状断层）：八块阴影 tile 的 alpha 衰减场不连续——边带 tile（1px 维度）按「到窗口轮廓的垂直距离」取二次衰减剖面，角部 tile（32×32）却按「到 tile 外角的径向距离 hypot/√2」取值，两场只在角对角线上重合；KWin 按 9-patch 原样拼贴，接缝行/列上角部可比相邻边带强 12 个 alpha 点以上（实测 16px 距离处角侧 20.6% 对 边侧 8.3%），围绕四角呈现矩形色块。现角部 tile 改为以「到窗口角点的欧氏距离」驱动同一 32px 二次衰减剖面（alpha 只依赖到窗口轮廓的真实距离），角↔边接缝两侧 alpha 连续（真窗实测残差 ≤0.3 个 alpha 点，修复前台阶 12.4 点），对角线方向按物理距离自然变浅、外角平滑归零。KWin（Plasma 6）真窗验收：四角放大无可见分块，四边剖面与设计衰减一致（4px 处约 24%、31px 处归零）且四角对称；程序化 resize、minimize→compositor 还原、maximize→restore（agent `restore_window`、应用标题栏按钮切换、快速连发三类路径）后 KWin `frameGeometry` 与客户端记账逐帧一致、阴影四边四角对称连续。

- 切换页面卡顿（主演示点击侧栏切换页面时窗口可感知停顿，动画期间整窗以不受 vsync 约束的全速构建帧，桌面负载重叠时单帧最高拉长到约 620 ms、切换请求延迟最高 615 ms）：帧尾续帧路径把任何残留 paint 失效都武装为「立即帧」，而 `FrameScheduler::request_frame` 对已武装请求只接受「更早 deadline 收紧」——动画 tick（涟漪、悬停过渡等）每帧产生的新失效把本应按 cadence 到期的动画帧反复收紧为「立即」，形成帧构建约 2 ms 的全速链（动画期 shaping 调用每秒 3400+），直到动画结束；空闲循环探测 `has_frame_work` / `next_deadline` 的同型武装让仅改帧尾无法断链。现 `WindowDriver::arm_visual_request` 按「是否仍有已注册动画」与「是否允许收紧」分流：帧尾有动画时走 `request_animation_frame` 按 cadence 续帧、空闲探测在已有请求时不再重复武装，只有帧入口的外部唤醒（输入、定时器、Agent 命令）保留立即收紧，输入仍同轮渲染。验收（KWin/Plasma 6 真窗，agent 通道冷/温、间隔/无间隔连发三类协议）：切换延迟 23.7～47 ms 与修复前一致，动画期帧回到 vsync 节奏（17～33 ms）且约 1 秒内收敛、空闲安静，48 次无间隔连发 >25 ms 慢帧由 20 降为 6，通用/反馈/覆盖清单三页真窗截屏与基线逐位一致（0.000% 像素差），公开 API 测试 77 用例全绿；测量登记见 [UIX-PERF-030](docs/性能/UIX-PERF-030.md)。已知边界：调试 HUD 的自持渲染与永续动画（loading 旋转器）按显示节奏占帧属既有设计/组件生命周期问题，不在本次范围。

### 修复与平台完善

- OpenGL ES 后端按渲染目标修正纵向行序：`opengles` feature 下 Wayland EGL 界面不再上下颠倒、Windows WGL 回读行序规范化，并完成 Linux EGL GUI 路径首次构建验证。
- Wayland 接通自定义标题栏装饰模式：GUI Demo 跨平台启用自定义标题栏，Wayland 通过 xdg-decoration 在客户端与服务端装饰间切换。
- Wayland 程序化 `set_size` 现在同代更新 logical/drawable surface metrics，并向唯一窗口事件管线
  发布定向 resize；可调整大小的顶层窗口不再只更新 geometry/约束却继续呈现旧客户区 buffer。
- Agent 主窗口关闭会在图形与原生资源清理前发布精确 generation 的 closed 事实；终态 wait 回复
  获得有界传输排空窗口，调用方可在 `close_window` 后可靠取得关闭证明而不被 teardown 抢先断流。
- 应用窗口默认启用文件拖放：Windows 通过 `WM_DROPFILES` 交付本地路径，Wayland 完成 data offer、`text/uri-list` 非阻塞读取、逐窗路由及 checked teardown；稳定缺少能力的平台继续创建无拖放窗口。
- macOS 修复 platform 拆分遗留属性：恢复 `MacosAppEvent` 的 `Debug` / `Clone` 派生，并补充源码契约防止回归。
- Windows D3D12 完成生产装配：显式 `d3d12` feature 现在交付完整 GPU-native thin RHI 并注册为优先级 40 的候选；Vulkan 仍保持最高优先级，真实 Windows 公开运行继续单独验收。
- macOS Metal 完成生产装配：显式 `metal` feature 现在通过 CAMetalLayer 交付 GPU-native thin RHI、12 类固定 pipeline、Retina 物理 drawable、失败帧回滚与 swapchain present，并注册为低于 Vulkan 的优先级 90 候选；真实 macOS 运行继续单独验收。
- `Platform::gpu_adapters` 在 D3D11、D3D12、Vulkan 与 Metal 上返回对应原生 API 的 owned 设备描述；OpenGL ES 因缺少无上下文的可移植适配器枚举协议，继续返回明确的 typed `NotImplemented`。

### 发布状态

- 版权归 `yangyanhui` 所有；版本按专有闭源、仅限内部授权使用的方式交付，不对外分发源码或二进制制品。
- 已定义的 Windows 内部交付载体为 Windows 11 24H2 x64 ZIP：release Demo、内部 `.crate`、UIX 专有许可、Lucide / Feather 第三方声明、说明文件、运行时 Demo 图片与 SHA-256 清单；release 使用 Vulkan 首选、D3D11 兼容回退矩阵，并保留 Software 整体回退。
- 已定义 Linux x64 内部候选包：以确定性 `tar.gz` 交付 Vulkan 首选、Wayland/EGL OpenGL ES 兼容回退的主演示、内部 `.crate`、许可与说明载荷，并由独立校验器冻结条目顺序、epoch 时间、数字所有者、权限、gzip header 与逐项 SHA-256。
- `0.0.1` 正式制品只经私有 Gitea Release 分发，不引入发布签名私钥或自动更新器；升级使用已校验的并行版本目录，失败时回切上一份制品与 Settings 备份。
- 项目动态事实源已迁移到 Gitea；任务、负责人、阻塞、环境矩阵与带时点的验证结果统一由 [0.0.1 产品完成父 Issue #1](http://100.79.245.29:3000/admin/uix-app/issues/1) 及其子 Issue 持有。
- 本版本已于 2026-08-29 通过私有 Gitea Release 发布；按所有者明确决定，本次 Release 只附带已完成确定性构建与独立校验的 Linux x64 制品，Windows x64 制品未随本次发布提供，真实 Windows Vulkan / D3D11 运行验收仍未完成。

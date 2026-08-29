# 变更记录

这里记录 UIX 闭源版本中已经发生、会影响使用方、交付物或验证方式的变化。变更条目不表示版本已发布，也不承担任务、负责人、阻塞和实时完成度管理；`0.0.1` 的发布与交付事实见[交付与许可](docs/产品/交付与许可.md)。

## 0.0.1（未发布）

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
- 本版本仍未发布；正式发布状态、平台矩阵与带时点的验证结果以 Gitea 发布任务为准。

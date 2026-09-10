# Agent 独立后台操作面

[← Agent 控制](Agent控制.md)

## 契约与边界

启用 `agent-control` 后，AI 只控制独立后台操作面，不再控制用户的可见窗口。
后台操作面具有专用 UI 线程、`WidgetTree`、组件状态存储、导航、焦点、指针/拖拽、选区、撤销栈、私有剪贴板和 CPU 离屏视口。它复用正式 `WindowSession` / `WindowDriver` / `Renderer`，不是 `TestApp`、网页壳、隐藏窗口或系统输入注入。

- 发现与鉴权仍使用 `uix.agent.hub.v1` / `uix.agent.v1`。`list_windows` 中只有后台视口，`visible=false`、`focused=false`；节点 `focused` / `hovered` 是 AI 私有交互状态，不是桌面焦点。
- `hello.capabilities.background_control.isolated_workspace=true` 才证明服务端承诺隔离。旧应用没有该能力时，连接器拒绝控制，不自动激活、隐藏、最小化或改绑前台窗口。
- 业务数据不自动复制，也不自动隔离。应用必须单独构造后台导航、草稿、选择 State，仅显式共享业务服务或领域 State；共享更新仍按业务版本、冲突和授权规则仲裁。不得把前台 AppHandle、导航 State 或操纵桌面的回调捕获到后台根中。框架不是 Rust 回调沙箱，不能撤销应用自行调用的 OS、文件、网络或播放副作用。
- 本版每进程一个后台操作面。多个 AI 客户端共享此操作面，动作仍按队列和 `expected_revision` 仲裁；不承诺每个客户端独立草稿。
- 视口 DPR 为 1，单轴最大 4096、最多 8,388,608 像素；`resize_window` 只改变内存视口。`close_window` 只关闭后台操作面，不关闭应用的可见窗口。
- 激活、移动、最大化、最小化、还原和系统菜单没有后台含义，不能获得桌面访问能力。错误不会触发前台降级。

## 可见应用接入与迁移

```rust
use uix_app::prelude::*;

App::new()
    .root(|| label("用户窗口"))
    .enable_agent_control()
    .agent_root(|| label("AI 独立后台视图").automation_id("agent-home"))
    .run();
```

`agent_root` 不等于将前台根捕获的 State 再 clone 一份。应分别建立界面局部状态；`.uix` 中按组件状态存储声明的局部状态由各自树持有。共享业务服务可以分别被两个根显式捕获。

这是有意的兼容收紧：仅调用 `enable_agent_control()` 而未设置 `agent_root` 时，`run()` 在创建原生窗口之前返回 1，并经诊断报告配置错误，不能偷偷继续驱动用户窗口。未启用开关时，`agent_root` 不创建线程或端点；未编译 feature 时方法不可用。应用原有只读、保护、拒绝和确认策略转交后台操作面。

主演示使用 `demo/uix-lang-demo/src/agent.uix` 作为专用后台示例，不是前台镜像。它独立演示导航、草稿、提交结果与确认；不复用主窗口终端或原生标题栏，也不默认把后台演示数据同步到用户界面。

## 无窗口宿主与 Rust 客户端

无需启动图形桌面或 `App::run()`，也可显式启动后台操作面：

```rust
use uix_app::app::agent_workspace::AgentWorkspace;
use uix_app::prelude::*;

let workspace = AgentWorkspace::new(640, 480, || {
    button("后台任务").automation_id("task")
}).spawn()?;
let mut client = workspace.client()?;
assert_eq!(client.capabilities()["background_control"]["isolated_workspace"], true);
let windows = client.list_windows()?;
let view = windows[0];
let snapshot = client.snapshot(view.window_id)?;
let pixels = client.request(serde_json::json!({
    "type": "screenshot", "window_id": view.window_id
}))?;
assert_eq!(pixels["ok"], true);
workspace.close()?;
Ok::<(), Box<dyn std::error::Error>>(())
```

`AgentWorkspace` 的 `title`、`theme`、`font_bundle` 设置后台资源。显式 FontBundle 优先；未配置时通过平台字体发现装载正文与 CJK 回退，不创建显示连接或前台窗口。找不到可加载正文时返回 typed 错误，不使用 Lucide 图标字体充当正文。框架不再内置 WenKai；字体与测试 fixture 许可见[字体材料](../../../assets/fonts/README.md)。CPU 与 GPU 可能有栅格化差异，不承诺跨后端截图字节一致。

`spawn()` 成功表示首帧已经真实离屏绘制且本机端点就绪；非法尺寸、重复启动、字体错误或端点失败返回 typed `Error`。`workspace.client()` 只连接此租约的端点并检查发现文件身份，不扫描“最新实例”。`AgentBridgeClient::capabilities()` 返回握手能力；`request(JSON object)` 原样交付业务响应信封并校验传输 schema / request_id，非 object 在本地拒绝，不自动重试动作。

仓库提供可直接运行的无桌面宿主（两分钟自动回收，也可通过 `close_window` 提前结束）：

```sh
cargo run --features agent-control --example agent_workspace
python3 scripts/agent_client.py apps
python3 scripts/agent_client.py --instance <本示例的实例ID> list_windows
python3 scripts/agent_client.py --instance <本示例的实例ID> screenshot /tmp/agent-workspace.png
```

先按示例输出的 pid 和应用名从 `apps` 确认实例，再显式绑定。预期只有 `visible=false` 的后台视口、PNG 是该后台页面；旧应用缺少隔离能力时以 `background_control_required` 拒绝。示例未注入确认回调，受保护操作的确认会明确失败，不弹用户窗口。

根视口的 `window_id` 可以为 **0**，必须使用枚举结果而非假定从 1 开始。MCP 的 schema 与参数验证均接受 0；`confirm_id` 是协议返回的正整数，不转换成字符串。

## 观察与截图

普通语义、指针和键盘动作只写后台树。`snapshot` / `screenshot` 不依赖 DISPLAY、Wayland surface、GPU、桌面可见性或用户前台状态。`screenshot` 是本次队列请求之后真实 CPU 渲染生成的 PNG；`presented_revision` 在后台表示已完成离屏帧，不是“已显示到用户屏幕”。不读取桌面、不用缓存截图冒充当前画面。截图会包含后台页面实际显示的数据，应按敏感附件处理。

**已知画质问题（2026-09-09，源码 `fc92df41`）**：CPU 后台视图在 `View.scale(0.8)` 下的小字号文字可出现采样断笔，恢复 `1.0` 后消失。本次最小场景中缩放后的命中、选择与语义仍正常；这不代表画质通过，也不能将后台截图问题外推为原生 Vulkan 同样失败。问题尚未修复；复现输入与原图见 TASK-112（内部历史记录，不作为公开版本说明）。该结论针对应用内视图缩放，不是系统 DPI 或显示缩放验收。

`wait`、代际、修订、策略、确认、取消与 `outcome_unknown` 继续遵守 Agent 主文档。后台化不改变业务提交含义，不自动重放超时或结果未知的动作。

## 共享服务、确认与关闭

- `AgentWorkspace::read_only` / `protect` / `deny_action` / `require_confirm` 与 App 上的同名 Agent 策略等价。确认不能覆盖拒绝。
- `confirm_with` 在后台 UI turn 交付一次性确认意图。回调必须快速返回；建议投递非打断式通知，等待用户主动查看，不自动弹前台模态框或夺焦点。宿主随后调用 `workspace.resolve_confirmation(window_id, confirm_id, allow)`。App 集成的既有 `AppHandle::resolve_agent_confirmation` 只转交后台确认路由。
- 后台业务线程更新 State 后，调用 `workspace.wake()` 通知此 owner；或用 `workspace.post_to_ui` 在后台 owner 执行短任务。`workspace.poster()` 返回可克隆的 `AgentWorkspacePoster`（`wake` / `post_to_ui`，能力与 handle 一致、不转移租约与关闭责任），供扩展 worker 线程等长期持有方把结果转交后台 owner——例如动态扩展的 UI 声明 sink。禁止在 UI 回调中阻塞等待客户端响应。空闲无轮询；动画和定时器沿正式驱动的截止时间调度。
- 页面切换或组件卸载不自动取消外部 worker；应用持有的 State 与 poster 可以继续存活。需要丢弃离页后的结果时，由应用在回投执行处检查 owner/页面代际。`close()` 成功返回后，该操作面的迟到回投安全跳过并释放闭包捕获；不能把请求返回 `()` 当作已执行或持久化回执。
- `close()` / Drop 请求停止；窗口注册、命令、IPC、私有剪贴板与像素资源随 owner 释放。等待线程最多两秒，超时后另有控制面有界排空；阻塞 Rust 回调不能被安全强杀，超时明确失败，不声称回调已撤销。实际线程结束前不允许重用工作面租约。

## 与软件动态扩展的集成

后台操作面可以承载动态扩展面板：应用为后台单独创建 `ExtensionHost`（独立挂载位）并 `spawn_worker`，声明经 `with_ui_sink` 回调应用为 owned 快照后用 `poster.wake()` 唤醒后台 owner 重投影。前台用户界面与后台操作面各自拥有投影器与交互草稿库，只共享应用显式注入的领域端口（查询、受控提交与版本仲裁）。完整业务场景（文本处理工作台：读取 → 输入 → 处理 → 提交闭环、热替换、撤权与停用）见 [软件动态扩展 · 后台操作面接入](软件动态扩展.md#后台操作面接入独立-ui-worker-与共享领域端口)与 `examples/text_workbench.rs`；公开行为验证为 `tests/text_workbench_public_api.rs`。

## 公开 API 验证

`tests/agent_workspace_public_api.rs` 通过公开 `AgentWorkspace`、`AgentBridgeClient` 和 `TestApp` 消费者验证后台输入/导航与用户树隔离、显式业务共享、真实离屏 PNG、视口操作、鉴权/修订与策略边界、确认和关闭。TestApp 只作为用户输入侧验收载体，生产后台运行时不依赖它。
Linux/Unix 下同一测试文件还以 Python 3 启动官方 STDIO MCP，验证根视口 0 和数值确认 ID 的公开能力目录；不导入私有 Python 函数或控制用户实例。

```sh
cargo test --features agent-control,test-harness --test agent_workspace_public_api
```

无显示服务运行同一测试可以证明后台链路不需要桌面；不等价于完成所有 OS 上的真窗、媒体或第三方应用回调验收。

## 连续控制与手动测量

复用 workspace.client() 建立的同一实例连接，顺序动作逐条检查响应并等待 settle；稳定业务步骤可在一次宿主执行里连续调用，阶段末统一读取所需语义结果，避免每个按键都触发模型往返或截图。长等待与大截图可使用同一工作面的独立连接，不把并行传输当作事务。

Rust 客户端对分帧、I/O、schema/request_id 关联错误停止后续派发，返回错误后的再调用为 connection_unusable；可信 outcome_unknown 仍原样返回其业务信封，但同样不再允许在该客户端上发送新请求。应丢弃该客户端并在核实的同一实例上进行只读核查，不能用重连重做刚才的 mutation。普通可信业务拒绝或发送前参数过大不会损坏连接；权限仍由应用策略逐动作决定。

公开协议的响应限额包含换行；客户端按连续缓冲块有界解析大型 UTF-8/JSON 响应。相关外部消费者回归为 `tests/agent_client_public_api.rs`，只验证公开接口行为，不将微小耗时设为项目测试门槛。

手动连续性示例（不会创建原生窗口或访问桌面剪贴板）：

```sh
cargo build --locked --features agent-control --example agent_control_probe
# 先创建源码 target 下的报告父目录；文件必须尚不存在。
target/debug/examples/agent_control_probe 1800 target/agent-control-soak.json
```

示例在自建工作面循环填写私有草稿、完整 Ctrl+A/C/V、业务提交和语义回读，记录 1..1800 秒的事务样本、p50/p95、完成数与关闭结果；约 10 Hz 受控负载的空闲间隔不计入事务耗时，也不是 UI readiness 的固定 sleep。任何内容/提交计数/代际异常立即停止，不自动重派。它没有模型/宿主工具往返、人工基线或 Portal/EIS 输入，因此不能证明任意应用已达到人工速度或物理按键绝无残留。运行方应保持宿主存活并设置有界外部超时，用户取消时只停止这份自建工作面。

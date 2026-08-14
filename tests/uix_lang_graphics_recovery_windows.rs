// 仅在 Windows 同时启用 Agent 与故障注入能力时编译真实窗口测试。
#![cfg(all(windows, feature = "agent-control", feature = "test-harness"))]
// 允许断言型集成测试使用场景化 expect 诊断。
#![allow(
    clippy::expect_used,
    reason = "真实窗口协议测试必须在首个不满足的验收条件处给出场景诊断"
)]

// 接入只负责主演示子进程与 discovery 生命周期的测试支撑。
#[path = "support/uix_lang_graphics_recovery/process.rs"]
mod process;
// 接入只负责公开 uix.agent.v1 请求响应的测试 Adapter。
#[path = "support/uix_lang_graphics_recovery/protocol.rs"]
mod protocol;

// 导入主演示进程 fixture。
use process::DemoProcess;
// 导入公开协议客户端与逐窗稳定引用。
use protocol::{AgentClient, AgentWindow};
// 导入 JSON 值以检查语义快照。
use serde_json::Value;

// 保存状态文本的稳定自动化标识。
const RECOVERY_STATUS_ID: &str = "runtime-graphics-recovery-status";
// 保存设备丢失操作的稳定自动化标识。
const INJECT_DEVICE_LOST_ID: &str = "runtime-inject-device-lost";
// 保存表面丢失操作的稳定自动化标识。
const INJECT_SURFACE_LOST_ID: &str = "runtime-inject-surface-lost";
// 保存恢复后交互操作的稳定自动化标识。
const ASSERT_RECOVERED_ID: &str = "runtime-assert-recovered-interaction";
// 保存等待注入的稳定可访问文本。
const WAITING_STATUS: &str = "图形恢复测试：等待注入";
// 保存故障已注入的稳定可访问文本。
const INJECTED_STATUS: &str = "图形恢复测试：已注入，等待恢复后交互";
// 保存恢复后交互成功的稳定可访问文本。
const VERIFIED_STATUS: &str = "图形恢复测试：恢复后交互成功";

// 保存一条独立真实窗口恢复场景的验收证据。
struct RecoveryEvidence {
    // 保存恢复与后续交互各自完成呈现的 revision。
    revisions: (u64, u64),
    // 保存本场景唯一窗口的 generation。
    generation: u64,
}

// 验证两类 typed 图形故障各自在全新 D3D11 真实窗口中完成恢复。
#[test]
#[ignore = "requires an interactive Windows desktop and a D3D11 driver"]
fn real_uix_lang_demo_recovers_device_and_surface_loss() {
    // 以独立图形 owner 验证设备丢失，避免恢复后的 backend 代际污染下一类故障。
    let device = run_recovery_case(
        INJECT_DEVICE_LOST_ID,
        "ID3D11Device::GetDeviceRemovedReason",
        "DeviceLost",
    );
    // 以另一独立图形 owner 验证表面丢失确实到达 D3D11 RHI acquire 边界。
    let surface = run_recovery_case(
        INJECT_SURFACE_LOST_ID,
        "D3d11 RHI test surface lost",
        "SurfaceLost",
    );
    // 输出不含 token 的确定验收摘要供测试日志保存。
    println!(
        "uix-lang graphics recovery evidence: device={:?}/generation={}; surface={:?}/generation={}",
        device.revisions, device.generation, surface.revisions, surface.generation
    );
}

// 在独立 D3D11 子进程中完成一类故障的恢复与证据收集。
fn run_recovery_case(
    // 接收本场景故障按钮的稳定 automation ID。
    injection_id: &str,
    // 接收必须出现在原生日志中的 lower-boundary 标记。
    boundary_marker: &str,
    // 接收失败诊断使用的故障名称。
    fault_name: &str,
) -> RecoveryEvidence {
    // 启动同时打开 Agent 与 test-harness 双门禁的主演示子进程。
    let mut demo = DemoProcess::spawn();
    // 等待进程发布同用户本机 discovery 描述符。
    let endpoint = demo.wait_for_endpoint();
    // 连接命名管道并保留测试 fixture 的子进程存活检查。
    let stream = demo.connect(&endpoint.endpoint);
    // 以 descriptor token 完成首条 hello 鉴权。
    let mut client = AgentClient::handshake(stream, &endpoint.token);
    // 当前专用验收模式必须只登记一个窗口。
    let window = client.list_single_window();
    // 首个语义 revision 必须完成真实 present 后才读取页面。
    client.wait_for_presented(window, 1);

    // 读取故障注入前的专用页面语义树。
    let initial = client.snapshot(window);
    // 初始页面必须明确等待注入。
    assert_status(&initial, WAITING_STATUS);
    // 未注入前必须禁止恢复后交互。
    assert_disabled(&initial, ASSERT_RECOVERED_ID, true);

    // 完成本场景的 typed 故障、真实恢复呈现与后续交互。
    let revisions = run_recovery_round(&mut client, window, injection_id);

    // 关闭协议连接，允许输出读取线程在子进程结束后排空。
    drop(client);
    // 只终止本测试创建的子进程并收集已经写出的图形证据。
    let output = demo.stop_and_collect();
    // 启动必须命中生产 D3D11 retained RHI recipe。
    assert!(
        output.contains(
            "Graphics bootstrap: selected recipe backend=d3d11; raster=gpu_native; present=swapchain"
        ),
        "真实窗口未选择 D3D11 GPU recipe；output={output}"
    );
    // 本场景故障必须实际进入指定的 D3D11 native lower boundary。
    assert!(
        output.contains(boundary_marker),
        "真实窗口未经过 D3D11 {fault_name} 边界；output={output}"
    );
    // 启动阶段不得因 D3D11 探测失败直接退到整机 CPU renderer。
    assert!(
        !output.contains("fallback=software_cpu"),
        "真实窗口启动不应退化为整机 CPU fallback；output={output}"
    );
    // 返回公开协议与原生日志共同验证过的场景证据。
    RecoveryEvidence {
        // 保存两个严格等待 presented 的 revision。
        revisions,
        // 保存本场景窗口的稳定 generation。
        generation: window.generation,
    }
}

// 在同一窗口完成一次注入、恢复呈现和后续交互。
fn run_recovery_round(
    // 接收公开协议 Adapter。
    client: &mut AgentClient,
    // 接收当前窗口稳定身份与 generation。
    window: AgentWindow,
    // 接收本轮故障按钮的稳定 automation ID。
    injection_id: &str,
) -> (u64, u64) {
    // 通过语义动作安排下一次真实绘制的 typed 故障。
    let injected_revision = client.invoke(window, injection_id);
    // 等待故障 teardown/rebuild 后提交对应 revision。
    client.wait_for_presented(window, injected_revision);
    // 读取已经完成恢复呈现的语义树。
    let recovered = client.snapshot(window);
    // 恢复后页面必须等待后续交互确认。
    assert_status(&recovered, INJECTED_STATUS);
    // 只有真实恢复呈现后才允许后续交互。
    assert_disabled(&recovered, ASSERT_RECOVERED_ID, false);

    // 通过同一公开语义管线执行恢复后的用户动作。
    let verified_revision = client.invoke(window, ASSERT_RECOVERED_ID);
    // 等待后续交互的新 revision 完成真实 present。
    client.wait_for_presented(window, verified_revision);
    // 读取本轮最终语义状态。
    let verified = client.snapshot(window);
    // 最终必须留下恢复后仍可交互的稳定证据。
    assert_status(&verified, VERIFIED_STATUS);
    // 返回两次单调 revision 供最终摘要核对。
    (injected_revision, verified_revision)
}

// 断言语义树中的图形恢复状态文本。
fn assert_status(snapshot: &Value, expected: &str) {
    // 按稳定 automation ID 定位状态节点。
    let node = node_by_automation_id(snapshot, RECOVERY_STATUS_ID);
    // 可访问名称必须与当前状态精确一致。
    assert_eq!(node["name"], expected);
}

// 断言指定按钮的禁用状态。
fn assert_disabled(snapshot: &Value, automation_id: &str, expected: bool) {
    // 按稳定 automation ID 定位按钮节点。
    let node = node_by_automation_id(snapshot, automation_id);
    // 状态对象必须提供布尔 disabled 字段。
    assert_eq!(node["state"]["disabled"], expected);
}

// 按稳定 automation ID 查找唯一语义节点。
fn node_by_automation_id<'a>(snapshot: &'a Value, automation_id: &str) -> &'a Value {
    // 读取扁平语义节点数组。
    snapshot["nodes"]
        // 快照必须包含节点数组。
        .as_array()
        // 缺失数组立即给出协议形状诊断。
        .expect("snapshot nodes")
        // 遍历当前窗口 generation 的全部节点。
        .iter()
        // 只接受精确稳定标识。
        .find(|node| node["automation_id"] == automation_id)
        // 缺失节点时报告具体 automation ID。
        .unwrap_or_else(|| panic!("missing automation node {automation_id}"))
}

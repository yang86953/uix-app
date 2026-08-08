use super::*;

// D3D11 真实窗口必须呈现经过旋转的大字号 MSDF 水印场景。
#[test]
#[ignore = "requires an interactive Windows desktop and D3D11 driver"]
fn real_d3d11_demo_presents_large_rotated_msdf_watermark_case() {
    // 运行默认 D3D11 生产路径的专项场景。
    run_large_msdf_case(DEFAULT_D3D11_GRAPHICS);
}

// OpenGL ES 真实窗口必须复用同一通用 MSDF lowering 场景。
#[cfg(feature = "opengles")]
#[test]
#[ignore = "requires an interactive Windows desktop and OpenGL ES driver"]
fn real_opengles_demo_presents_large_rotated_msdf_watermark_case() {
    // 运行显式 OpenGL ES 生产路径的专项场景。
    run_large_msdf_case(FORCED_OPENGLES_GRAPHICS);
}

// 在指定原生 adapter 上定位 Watermark 组件并等待其真实 present。
fn run_large_msdf_case(graphics: GraphicsExpectation) {
    // GUI 场景必须串行运行，避免多个窗口竞争同一个桌面 surface。
    let _guard = REAL_GUI_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    // 直接进入组件测试页，避免其它页面遮蔽大字号场景。
    // 大字号字形测试通过专用组件测试页面启动。
    let mut demo = DemoProcess::spawn_with_args(graphics, &["--test-components"]);
    // 等待 demo 写出 agent discovery descriptor。
    let descriptor = demo.wait_for_descriptor();
    // 读取 agent pipe endpoint。
    let endpoint = descriptor["endpoint"]
        .as_str()
        .expect("descriptor endpoint");
    // 读取 agent hello token。
    let token = descriptor["token"].as_str().expect("descriptor token");
    // 建立当前 demo 的 Windows named pipe 连接。
    let stream = connect(endpoint, &mut demo.child);
    // 使用有缓冲的 JSONL 连接发送验收请求。
    let mut connection = BufReader::new(stream);

    // 完成 agent 协议握手。
    let hello = exchange(
        &mut connection,
        json!({
            "schema": "uix.agent.v1",
            "request_id": "msdf-visual-hello",
            "type": "hello",
            "token": token,
            "client": { "name": "uix-msdf-visual-acceptance" },
        }),
    );
    // 握手失败不得继续把窗口状态当成视觉证据。
    assert_success(&hello, "msdf-visual-hello");

    // 查询当前唯一主窗口及其 surface generation。
    let listed = exchange(
        &mut connection,
        json!({
            "schema": "uix.agent.v1",
            "request_id": "msdf-visual-list",
            "type": "list_windows",
        }),
    );
    // 列表请求必须成功。
    assert_success(&listed, "msdf-visual-list");
    // 读取唯一 demo 窗口。
    let windows = listed["windows"].as_array().expect("listed windows");
    // 组件测试场景不应偷偷创建额外窗口。
    assert_eq!(windows.len(), 1);
    // 读取窗口身份。
    let window_id = windows[0]["window_id"].as_u64().expect("window id");
    // 读取当前 surface generation。
    let generation = windows[0]["generation"].as_u64().expect("generation");
    // 首帧必须已经完成一次最终 present。
    wait_for_presented(
        &mut connection,
        window_id,
        generation,
        1,
        "msdf-visual-first-present",
    );

    // 逐页前进，直到定位显式的大字号旋转 Watermark 场景。
    let mut watermark_found = false;
    for index in 0..COMPONENT_VISUAL_CASE_COUNT {
        // 获取当前组件测试页的语义快照。
        let snapshot_response = exchange(
            &mut connection,
            json!({
                "schema": "uix.agent.v1",
                "request_id": format!("msdf-visual-snapshot-{index:03}"),
                "type": "snapshot",
                "window_id": window_id,
            }),
        );
        // 快照失败不能被误判成组件缺失。
        assert_success(
            &snapshot_response,
            &format!("msdf-visual-snapshot-{index:03}"),
        );
        // 读取当前组件名称。
        let snapshot = &snapshot_response["snapshot"];
        // Watermark 名称必须来自真实组件树，而非测试常量。
        let current_name = node_by_automation_id(snapshot, "component-qa-current")["name"]
            .as_str()
            .expect("component name");
        // 到达 Watermark 后验证目标节点可见并停止翻页。
        if current_name == "Watermark" {
            // 组件 target 必须拥有正数的真实可见范围。
            let target = node_by_automation_id(snapshot, "component-qa-target");
            // 读取目标节点可见 bounds。
            let bounds = &target["visible_bounds"];
            // 专项场景必须确实进入可绘制区域。
            assert!(
                bounds["w"].as_f64().unwrap_or_default() > 0.0
                    && bounds["h"].as_f64().unwrap_or_default() > 0.0,
                "large MSDF Watermark target must be visible: {bounds}"
            );
            // 记录已经定位到目标组件。
            watermark_found = true;
            // 不再改变组件状态，保留当前大字号场景等待 present。
            break;
        }
        // 最后一页没有 Watermark 时应由最终断言报告错误。
        if index + 1 == COMPONENT_VISUAL_CASE_COUNT {
            break;
        }
        // 通过 agent invoke 进入下一个组件场景。
        let next = invoke_until_presentable(
            &demo,
            &mut connection,
            window_id,
            generation,
            "component-qa-next",
        );
        // 读取翻页后的新 revision。
        let revision = next["revision"].as_u64().expect("next revision");
        // 等待大字号目标所在的后续 present 完成。
        wait_for_presented(
            &mut connection,
            window_id,
            generation,
            revision,
            &format!("msdf-visual-next-{index:03}"),
        );
    }
    // 若组件顺序或 manifest 被错误修改，专项验收必须失败。
    assert!(
        watermark_found,
        "component visual manifest must contain Watermark"
    );

    // 关闭 agent 连接并收集 demo 的原生 adapter 日志。
    drop(connection);
    let output = demo.close_and_wait();
    // 日志必须确认场景确实运行在期望的原生 adapter 上。
    if let Some(marker) = graphics.adapter_marker {
        assert!(
            output.contains(marker),
            "MSDF visual case must reach adapter marker {marker}; output={output}"
        );
    }
    // 输出专项证据边界，明确它验证的是 present 与组件定位。
    println!(
        "large rotated MSDF case presented: adapter={}; case=Watermark; pixel_capture=manual-WGC-required",
        graphics.evidence_label
    );
}

// 等待指定 agent revision 完成真正的 surface present。
fn wait_for_presented(
    connection: &mut BufReader<File>,
    window_id: u64,
    generation: u64,
    revision: u64,
    request_id: &str,
) {
    // 发送带 generation 的 present wait 请求。
    let response = exchange(
        connection,
        json!({
            "schema": "uix.agent.v1",
            "request_id": request_id,
            "type": "wait",
            "window_id": window_id,
            "generation": generation,
            "presented_revision": revision,
            "timeout_ms": PRESENT_TIMEOUT_MS,
        }),
    );
    // wait 失败必须保留具体请求上下文。
    assert_success(&response, request_id);
    // 只有 presented 才算完成 MSDF 真窗执行边界。
    assert_eq!(response["outcome"], "presented");
}

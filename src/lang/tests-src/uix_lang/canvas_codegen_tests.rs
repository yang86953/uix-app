// 集中验证 Canvas 组件的专有属性、必填集合与拒绝路径。

// 引入共享文档解析入口。
use super::parser::parse_document;
// 引入共享 View 生成入口。
use super::widget_codegen::generate_document_view;

// 验证 Canvas 映射公开 canvas 组合器并保留三个专有属性。
#[test]
fn generates_canvas_with_required_attributes() {
    // 解析带完整专有属性的画布文档。
    let document = parse_document(
        // width/height 用数值字面量，paint 引用 Rust 绘制函数。
        r#"<Canvas width="590" height="34" paint={draw_waveform} flexGrow={1} />"#,
    )
    // 合法画布文档必须解析成功。
    .expect("画布文档应解析成功");
    // 生成完整令牌。
    let tokens = generate_document_view(&document)
        // 合法画布应生成成功。
        .expect("画布组件应生成成功")
        // 转换为文本。
        .to_string();
    // 应调用公开 canvas 组合器。
    assert!(tokens.contains(":: uix_app :: prelude :: canvas"));
    // 宽度与高度应作为 f32 参数传入。
    assert!(tokens.contains("590"));
    assert!(tokens.contains("34"));
    // 绘制函数应保留调用方名称。
    assert!(tokens.contains("draw_waveform"));
    // 公共样式应继续经统一装饰链应用。
    assert!(tokens.contains("flex_grow"));
}

// 验证 paint 支持 Widget 体内经 external 声明的绘制函数。
#[test]
fn generates_canvas_paint_from_widget_external() {
    // 解析组件内画布文档。
    let document = parse_document(
        // external 显式声明绘制函数。
        r#"
        <Widget name="WaveBlock" external="draw_waveform">
          <Canvas width="200" height="20" paint={draw_waveform} />
        </Widget>
        <WaveBlock />
        "#,
    )
    // 合法组件文档必须解析成功。
    .expect("组件画布文档应解析成功");
    // 生成完整令牌。
    let tokens = generate_document_view(&document)
        // 合法组件画布应生成成功。
        .expect("组件画布应生成成功")
        // 转换为文本。
        .to_string();
    // 绘制函数应保留 external 名称。
    assert!(tokens.contains("draw_waveform"));
    // 应调用公开 canvas 组合器。
    assert!(tokens.contains(":: uix_app :: prelude :: canvas"));
}

// 验证缺失必填属性与声明子节点都在生成前诊断。
#[test]
fn rejects_incomplete_canvas_shapes() {
    // 解析缺少 paint 的画布。
    let missing_paint = parse_document(r#"<Canvas width="10" height="10" />"#)
        // 声明阶段仍可解析。
        .expect("缺属性文档应可解析");
    // 生成阶段必须拒绝缺失属性。
    let error = generate_document_view(&missing_paint)
        // 必填属性缺失必须失败。
        .expect_err("缺失 paint 必须失败");
    // 诊断应列出完整必填集合。
    assert!(error.message.contains("width、height 与 paint"));
    // 解析带子节点的画布。
    let with_child = parse_document(r#"<Canvas width="10" height="10" paint={f}><Text>x</Text></Canvas>"#)
        // 声明阶段仍可解析。
        .expect("子节点文档应可解析");
    // 生成阶段必须拒绝声明子节点。
    let child_error = generate_document_view(&with_child)
        // 叶子组件不得接收子节点。
        .expect_err("Canvas 子节点必须失败");
    // 诊断应说明叶子边界。
    assert!(child_error.message.contains("不接受子节点"));
    // 解析 paint 用字面量的画布。
    let literal_paint = parse_document(r#"<Canvas width="10" height="10" paint="red" />"#)
        // 声明阶段仍可解析。
        .expect("字面量文档应可解析");
    // 生成阶段必须拒绝字面量绘制回调。
    let literal_error = generate_document_view(&literal_paint)
        // 字面量不能拥有绘制行为。
        .expect_err("字面量 paint 必须失败");
    // 诊断应指向表达式要求。
    assert!(literal_error.message.contains("Rust 绘制函数表达式"));
}

// 引入文档解析与核心 View 生成入口。
use super::{Diagnostic, generate_view, parse_document};

// 解析单根文档并生成稳定令牌文本。
fn generate(source: &str) -> Result<String, Diagnostic> {
    // 解析完整 UIX 文档。
    let document = parse_document(source)?;
    // 生成根 View 令牌并规范为空白稳定文本。
    generate_view(&document.root).map(|tokens| tokens.to_string())
}

// 验证 click 只公开坐标字段并在 Rust 生成前拒绝未知字段。
#[test]
fn validates_click_payload_fields() {
    // 同时读取两个已登记坐标字段。
    let tokens = generate(
        // 构造点击处理器。
        r#"<Button @click="handle($event.x, $event.y)">Go</Button>"#,
    )
    // 已登记字段必须生成成功。
    .expect("click x/y 应通过登记校验");
    // 坐标必须映射到公开 ClickEvent.pos。
    assert!(tokens.contains("pos . x") && tokens.contains("pos . y"));
    // 未知点击字段必须在 UIX 生成期失败。
    let unknown = generate(
        // 构造未登记字段。
        r#"<Button @click="handle($event.value)">Go</Button>"#,
    )
    // 提取预期诊断。
    .expect_err("click value 未登记时必须失败");
    // 诊断必须包含事件名、未知字段和合法字段。
    assert!(
        unknown.message.contains("@click")
            // 点名未知字段。
            && unknown.message.contains("value")
            // 修复建议列出坐标。
            && unknown.suggestion.contains("$event.x")
    );
}

// 验证 change 的 value 字段投影到现有实际载荷。
#[test]
fn validates_change_value_payload() {
    // 构造 Input 值变化处理器。
    let tokens = generate(
        // 使用规范 value 字段。
        r#"<Input value={name} @change="handle($event.value)" />"#,
    )
    // 已登记 value 必须生成成功。
    .expect("change value 应通过登记校验");
    // 生成物必须继续使用现有 Change 注册入口。
    assert!(tokens.contains("on_change_fn") && tokens.contains("handle"));
    // 未知 change 坐标必须失败。
    let unknown = generate(
        // 构造错误字段。
        r#"<Input value={name} @change="handle($event.x)" />"#,
    )
    // 提取预期诊断。
    .expect_err("change x 未登记时必须失败");
    // 修复建议必须只列出 value。
    assert!(unknown.message.contains("@change") && unknown.suggestion.contains("$event.value"));
}

// 验证键盘事件公开 key/code 且不会吞掉组件事件。
#[test]
fn generates_registered_keyboard_payload_fields() {
    // 构造按键按下处理器。
    let tokens = generate(
        // 同时读取逻辑键和稳定代码文本。
        r#"<Button @keyDown="handle($event.key, $event.code)">Go</Button>"#,
    )
    // 键盘登记字段必须生成成功。
    .expect("keyDown key/code 应生成成功");
    // 生成物必须筛选 KeyDown 并继续返回 NotHandled。
    assert!(
        tokens.contains("on_key") && tokens.contains("KeyDown") && tokens.contains("NotHandled")
    );
    // code 使用公开 KeyCode 的稳定调试文本。
    assert!(tokens.contains("format !") && tokens.contains("key_payload"));
    // mods 已随修饰键载荷登记，读取后必须生成稳定的 KeyMod 局部。
    let mods = generate(
        // 构造读取修饰键载荷载荷的处理器。
        r#"<Button @keyDown="handle($event.mods)">Go</Button>"#,
    )
    // 登记字段必须生成成功。
    .expect("keyDown mods 应生成成功");
    // 生成物必须解构 mods 并建立稳定修饰键载荷局部。
    assert!(
        mods.contains("mods : __uix_mods") && mods.contains("__uix_mods_payload")
    );
    // 未知键盘字段仍然必须失败。
    let unknown = generate(
        // 构造未登记的坐标字段。
        r#"<Button @keyDown="handle($event.x)">Go</Button>"#,
    )
    // 提取预期诊断。
    .expect_err("keyDown 未登记字段必须失败");
    // 修复建议必须列出 key、code 与 mods。
    assert!(
        unknown.suggestion.contains("$event.key")
            && unknown.suggestion.contains("$event.code")
            && unknown.suggestion.contains("$event.mods")
    );
}

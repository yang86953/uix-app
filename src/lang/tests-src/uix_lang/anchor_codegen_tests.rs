// 引入解析与核心生成入口。
use super::{Diagnostic, generate_view, parse_document};

// 生成测试源码的稳定令牌快照。
fn generate(source: &str) -> Result<String, Diagnostic> {
    // 先解析完整 UIX 文档。
    let document = parse_document(source)?;
    // 再生成公开 Rust View 表达式。
    generate_view(&document.root).map(|tokens| tokens.to_string())
}

// 验证 Anchor 类型化条目、定位偏移、指示线、href 事件与公共属性生成。
#[test]
// 声明完整 Anchor 生成测试。
fn generates_anchor_contract() {
    // 生成覆盖类型化数据、像素偏移、动态指示线、事件载荷和公共属性的锚点导航。
    let snapshot = generate(
        r#"<Anchor items={anchor_items} offsetTop="24px" showInk={show_anchor_ink} @change="record_anchor_change($event)" width="240px" automationId="settings-anchor" />"#,
    )
    // 合法 Anchor 必须成功生成。
    .expect("文档属性应映射到公开 Anchor API");
    // 数据表达式必须通过拥有权 IntoIterator 统一收集。
    assert!(snapshot.contains("IntoIterator :: into_iter ((anchor_items) . clone ())"));
    // 收集目标必须锁定公开 AnchorItem 类型。
    assert!(snapshot.contains("Vec < :: uix_app :: prelude :: AnchorItem >"));
    // 组件必须通过公开构造器取得条目所有权。
    assert!(snapshot.contains("Anchor :: new"));
    // 像素偏移必须映射到公开 target_offset 构建器。
    assert!(snapshot.contains("target_offset (24.0)"));
    // 动态指示线策略必须映射到公开 show_ink 构建器。
    assert!(snapshot.contains("show_ink (show_anchor_ink)"));
    // href Change 事实必须接入公开文本处理器。
    assert!(snapshot.contains("on_change_fn") && snapshot.contains("record_anchor_change"));
    // Anchor 必须生成叶 View。
    assert!(snapshot.contains("ViewNode :: leaf"));
    // 公共宽度与自动化标识仍由公共属性层消费。
    assert!(snapshot.contains("width (240.0)") && snapshot.contains("automation_id"));
}

// 验证 Anchor 动态偏移仍交由 Rust 类型系统检查。
#[test]
// 声明 Anchor 动态数值生成测试。
fn generates_dynamic_anchor_offset() {
    // 使用受限表达式提供运行时偏移。
    let snapshot = generate(r#"<Anchor items={anchor_items} offsetTop={header_height} showInk />"#)
        // 合法数值表达式必须成功生成。
        .expect("动态 offsetTop 应映射到公开 Anchor API");
    // 表达式必须原样进入 target_offset 类型检查位置。
    assert!(snapshot.contains("target_offset (header_height)"));
    // 布尔简写必须映射为 true 指示线策略。
    assert!(snapshot.contains("show_ink (true)"));
}

// 验证 Anchor 必需数据与数值诊断。
#[test]
// 声明 Anchor 数据错误测试。
fn rejects_invalid_anchor_data() {
    // 缺失 items 时没有滚动目标数据来源。
    let missing = generate(r#"<Anchor />"#)
        // 缺失必需属性必须失败。
        .expect_err("缺少 items 必须被拒绝");
    // 诊断必须点名 items。
    assert!(missing.message.contains("items"));
    // 字符串不能伪装成类型化 AnchorItem 集合。
    let literal = generate(r#"<Anchor items="overview" />"#)
        // 非表达式 items 必须失败。
        .expect_err("字符串 items 必须被拒绝");
    // 诊断必须说明 AnchorItem 集合要求。
    assert!(literal.message.contains("AnchorItem"));
    // 非数值偏移不能进入运行时定位契约。
    let offset = generate(r#"<Anchor items={anchor_items} offsetTop="large" />"#)
        // 非法长度必须失败。
        .expect_err("非数值 offsetTop 必须被拒绝");
    // 诊断必须点名数值要求。
    assert!(offset.message.contains("数值"));
    // 非布尔指示线配置不能进入公开绘制策略。
    let ink = generate(r#"<Anchor items={anchor_items} showInk="yes" />"#)
        // 非布尔字面量必须失败。
        .expect_err("非法 showInk 必须被拒绝");
    // 诊断必须明确布尔值要求。
    assert!(ink.message.contains("布尔值"));
}

// 验证 Anchor 叶节点、属性与事件边界。
#[test]
// 声明 Anchor 形状错误测试。
fn rejects_invalid_anchor_shape_attributes_and_events() {
    // Anchor 自身绘制链接列表，不能接受 View 子树。
    let child = generate(r#"<Anchor items={anchor_items}><Text>非法</Text></Anchor>"#)
        // 嵌套元素必须失败。
        .expect_err("Anchor 子节点必须被拒绝");
    // 诊断必须点明叶组件边界。
    assert!(child.message.contains("不接受子节点"));
    // activeHref 尚未登记为 UIX 文档属性。
    let attribute = generate(r#"<Anchor items={anchor_items} activeHref={active_href} />"#)
        // 文档外属性必须失败。
        .expect_err("未知 Anchor 属性必须被拒绝");
    // 诊断必须包含具体未知属性名。
    assert!(attribute.message.contains("activeHref"));
    // @select 不是 Anchor 登记的 href 事实事件。
    let event =
        generate(r#"<Anchor items={anchor_items} @select="record_anchor_change($event)" />"#)
            // 未登记事件必须失败。
            .expect_err("未知 Anchor 事件必须被拒绝");
    // 诊断必须包含具体未知事件名。
    assert!(event.message.contains("@select"));
}

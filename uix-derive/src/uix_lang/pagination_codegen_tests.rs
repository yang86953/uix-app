// 引入解析与核心生成入口。
use super::{Diagnostic, generate_view, parse_document};

// 生成测试源码的稳定令牌快照。
fn generate(source: &str) -> Result<String, Diagnostic> {
    // 先解析完整 UIX 文档。
    let document = parse_document(source)?;
    // 再生成公开 Rust View 表达式。
    generate_view(&document.root).map(|tokens| tokens.to_string())
}

// 验证 Pagination total、双状态、事件与公共属性生成。
#[test]
// 声明完整 Pagination 生成测试。
fn generates_bound_pagination_contract() {
    // 生成覆盖动态 total、双状态、Change 与公共属性的分页器。
    let snapshot = generate(
        r#"<Pagination current={page} pageSize={page_size} total={total} @change="record_page($event)" width="640px" automationId="orders-pagination" />"#,
    )
    // 合法 Pagination 必须成功生成。
    .expect("文档属性应映射到公开 Pagination API");
    // 构造器必须接收动态 total 与文档 pageSize 默认值。
    assert!(snapshot.contains("Pagination :: new (total , 10_usize)"));
    // pageSize 必须先绑定到 State<usize>。
    let page_size = snapshot
        // 查找公开 pageSize 状态入口。
        .find("page_size_state (& (page_size))")
        // 缺失表示状态句柄未保留。
        .expect("应生成 pageSize 状态绑定");
    // pageSize 绑定必须启用文档承诺的条数切换入口。
    assert!(snapshot.contains("show_size_changer (true)"));
    // current 必须在 pageSize 后绑定。
    let current = snapshot
        // 查找公开 current 状态入口。
        .find("current_state (& (page))")
        // 缺失表示状态句柄未保留。
        .expect("应生成 current 状态绑定");
    // 绑定顺序必须保证 current 使用最新 pageSize 范围。
    assert!(page_size < current);
    // Change 处理器必须接收现有文本载荷。
    assert!(snapshot.contains("on_change_fn") && snapshot.contains("record_page"));
    // 公共宽度与自动化标识仍由公共属性层消费。
    assert!(snapshot.contains("width (640.0)") && snapshot.contains("automation_id"));
}

// 验证 Pagination 文档默认值与非受控最小形状。
#[test]
// 声明 Pagination 默认值测试。
fn generates_pagination_defaults() {
    // 生成无专有属性的最小分页器。
    let snapshot = generate(r#"<Pagination />"#)
        // 文档默认值必须足以构造组件。
        .expect("Pagination 默认属性应生成");
    // total 与 pageSize 默认值必须显式固定。
    assert!(snapshot.contains("Pagination :: new (0usize , 10_usize)"));
    // 未绑定 pageSize 时不应伪造条数切换能力。
    assert!(!snapshot.contains("show_size_changer"));
    // 未绑定 current 时保留运行时非受控状态。
    assert!(!snapshot.contains("current_state"));
}

// 验证 Pagination 整数和 State 属性诊断。
#[test]
// 声明 Pagination 值形状错误测试。
fn rejects_invalid_pagination_values() {
    // 小数 total 不能调用 usize 构造器。
    let fractional = generate(r#"<Pagination total="2.5" />"#)
        // 非整数 total 必须失败。
        .expect_err("小数 total 必须被拒绝");
    // 诊断必须明确 usize。
    assert!(fractional.message.contains("usize"));
    // 字面 current 不能提供 State 所有权。
    let current = generate(r#"<Pagination current="2" />"#)
        // 字面 current 必须失败。
        .expect_err("字面 current 必须被拒绝");
    // 诊断必须明确 State<usize>。
    assert!(current.message.contains("State<usize>"));
    // 字面 pageSize 不能提供 State 所有权。
    let page_size = generate(r#"<Pagination pageSize="20" />"#)
        // 字面 pageSize 必须失败。
        .expect_err("字面 pageSize 必须被拒绝");
    // 诊断必须明确 State<usize>。
    assert!(page_size.message.contains("State<usize>"));
}

// 验证 Pagination 叶节点与属性边界。
#[test]
// 声明 Pagination 形状错误测试。
fn rejects_invalid_pagination_shape_and_attributes() {
    // Pagination 自身绘制控件，不能接受 View 子树。
    let child = generate(r#"<Pagination><Text>非法</Text></Pagination>"#)
        // 嵌套元素必须失败。
        .expect_err("Pagination 子节点必须被拒绝");
    // 诊断必须点明叶组件边界。
    assert!(child.message.contains("不接受子节点"));
    // 未登记属性不能被公共映射静默忽略。
    let unknown = generate(r#"<Pagination showTotal="false" />"#)
        // 文档外属性必须失败。
        .expect_err("未知 Pagination 属性必须被拒绝");
    // 诊断必须包含具体未知属性名。
    assert!(unknown.message.contains("showTotal"));
}

// 引入文档生成、record 生成与解析入口。
use super::{Diagnostic, generate_document_view, generate_record_items, parse_document};

// 解析完整文档并生成 record 与 View 的稳定令牌文本。
fn generate(source: &str) -> Result<String, Diagnostic> {
    // 解析 record、组件与调用根。
    let document = parse_document(source)?;
    // 生成模块级 record 结构体。
    let records = generate_record_items(&document)?;
    // 生成组件展开后的 View。
    let view = generate_document_view(&document)?;
    // 合并两部分令牌以便断言完整类型闭环。
    Ok(format!("{} {}", records, view))
}

// 验证 Vec 字符串/数值/record、Some 与嵌套 record 递归生成。
#[test]
fn generates_extended_typed_state_values() {
    // 构造覆盖全部新增类型形状的文档。
    let source = r#"
        <Record name="Address" fields="city: String" />
        <Record name="Profile" fields="name: String, address: Address, tags: Vec<String>, scores: Vec<number>, addresses: Vec<Address>, alias: Option<String>" />
        <Widget name="Typed" state="names: Vec<String> = ['甲', '乙'], scores: Vec<number> = [1, 2.5], addresses: Vec<Address> = [{ city: '上海' }, { city: '杭州' }], selected: Option<String> = Some('active'), profile: Profile = { name: '用户', address: { city: '北京' }, tags: ['核心', '稳定'], scores: [3, 4.5], addresses: [{ city: '深圳' }], alias: Some('owner') }">
            <Text>{profile.name}</Text>
        </Widget>
        <Typed />
    "#;
    // 生成全部类型化初始值。
    let tokens = generate(source).expect("扩展类型白名单与递归初始值应生成成功");
    // Vec<number> 必须映射为 f64 向量类型。
    assert!(tokens.contains("Vec < f64 >") && tokens.contains("as f64"));
    // Vec<Record> 必须引用模块级 Address 类型。
    assert!(tokens.contains("Vec < Address >") && tokens.contains("Address {"));
    // 字符串数组项目必须经过组件拥有型字符串包装。
    assert!(tokens.contains("owned_string") && tokens.contains("\"甲\""));
    // Some 字符串必须生成拥有型 Option<String>。
    assert!(tokens.contains("Option :: Some") && tokens.contains("\"active\""));
    // 嵌套 record 字段必须递归物化而非保留对象伪语法。
    assert!(tokens.contains("Profile {") && tokens.contains("city :"));
}

// 验证 Vec<Record> 元素引用必须指向已声明 record。
#[test]
fn rejects_unknown_record_vector_element_type() {
    // 构造引用未声明 record 的集合状态。
    let error = parse_document(
        // 使用 Missing 作为 Vec 元素类型。
        r#"<Widget name="Bad" state="items: Vec<Missing> = []"><Text>A</Text></Widget><Bad />"#,
    )
    // 提取预期引用诊断。
    .expect_err("Vec<Missing> 必须在解析期失败");
    // 诊断必须点名缺失 record。
    assert!(error.message.contains("Missing") && error.message.contains("未在当前文档声明"));
}

// 验证 Some 和 record 集合元素形状在宏生成期拒绝。
#[test]
fn rejects_invalid_some_and_record_vector_values() {
    // Some 只接受字符串字面量。
    let some = generate(
        // 构造数值 Some。
        r#"<Widget name="Bad" state="selected: Option<String> = Some(1)"><Text>A</Text></Widget><Bad />"#,
    )
    // 提取预期可空字符串诊断。
    .expect_err("Some(number) 必须失败");
    // 诊断必须列出 None 与 Some('text')。
    assert!(some.message.contains("None 或 Some('text')"));
    // Vec<Record> 项必须是匹配字段的对象。
    let record = generate(
        // 构造字符串替代 record 对象。
        r#"<Record name="Item" fields="name: String" /><Widget name="Bad" state="items: Vec<Item> = ['x']"><Text>A</Text></Widget><Bad />"#,
    )
    // 提取预期结构形状诊断。
    .expect_err("Vec<Record> 非对象元素必须失败");
    // 诊断必须说明对象字面量要求。
    assert!(record.message.contains("对象字面量"));
}

// 验证嵌套 record 的未知或缺失字段不会被静默接受。
#[test]
fn rejects_invalid_nested_record_fields() {
    // 嵌套 Address 缺少 city 字段。
    let missing = generate(
        // 构造缺字段嵌套对象。
        r#"<Record name="Address" fields="city: String" /><Record name="Profile" fields="address: Address" /><Widget name="Bad" state="profile: Profile = { address: {} }"><Text>A</Text></Widget><Bad />"#,
    )
    // 提取预期缺字段诊断。
    .expect_err("嵌套 record 缺字段必须失败");
    // 诊断必须点名 city。
    assert!(missing.message.contains("缺少字段 city"));
    // 嵌套 Address 出现未知字段。
    let unknown = generate(
        // 构造额外 zip 字段。
        r#"<Record name="Address" fields="city: String" /><Record name="Profile" fields="address: Address" /><Widget name="Bad" state="profile: Profile = { address: { city: 'x', zip: 'y' } }"><Text>A</Text></Widget><Bad />"#,
    )
    // 提取预期未知字段诊断。
    .expect_err("嵌套 record 未知字段必须失败");
    // 诊断必须点名 zip。
    assert!(unknown.message.contains("字段 zip"));
}

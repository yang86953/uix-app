// 引入组件感知生成入口与文档解析器。
use super::{generate_document_view, parse_document};

// 验证默认与具名 Slot 在模板位置内联调用方作用域内容。
#[test]
fn expands_default_and_named_slots_in_caller_scope() {
    // 解析拥有组件私有状态的调用方与双插槽容器。
    let document = parse_document(
        // 插槽插值只能读取 Host 的 caller，不能读取 Panel 私有作用域。
        r#"
        <Widget name="Panel" props="title: String">
          <Column>
            <Text>{title}</Text>
            <Slot />
            <Container><Slot name="footer" /></Container>
          </Column>
        </Widget>
        <Widget name="Host" state="caller: '调用方'">
          <Panel title="标题">
            <Text>{caller}</Text>
            <Button slot="footer">操作 {caller}</Button>
          </Panel>
        </Widget>
        <Host />
        "#,
    )
    // 合法插槽文档必须解析成功。
    .expect("默认与具名 Slot 文档应解析成功");
    // 生成完整组件感知 View 令牌。
    let tokens = generate_document_view(&document)
        // 调用方状态必须在投影前完成绑定改写。
        .expect("默认与具名 Slot 应展开成功")
        // 转成稳定文本检查投影结果。
        .to_string();
    // 默认插槽文本必须进入最终 View。
    assert!(tokens.contains("调用方"));
    // 具名插槽按钮必须进入 footer 模板位置。
    assert!(tokens.contains("操作"));
    // 归位属性与 Slot 占位不得泄漏到核心 View 生成代码。
    assert!(!tokens.contains("Slot") && !tokens.contains("slot ="));
}

// 验证调用方不提供内容时插槽为空且模板其余内容保留。
#[test]
fn allows_empty_slot_projection() {
    // 解析包含空默认插槽的稳定容器组件。
    let document = parse_document(
        // 自闭合调用不提供任何投影内容。
        r#"<Widget name="Panel"><Container><Text>标题</Text><Slot /></Container></Widget><Panel />"#,
    )
    // 空插槽调用必须解析成功。
    .expect("空 Slot 文档应解析成功");
    // 生成组件模板其余内容。
    let tokens = generate_document_view(&document)
        // 空插槽不能触发缺失子节点诊断。
        .expect("空 Slot 投影应生成成功")
        // 转成稳定文本检查标题保留。
        .to_string();
    // 非插槽模板内容必须继续生成。
    assert!(tokens.contains("标题"));
}

// 验证 For 内纯插槽组件沿用调用方循环词法绑定。
#[test]
fn expands_pure_slot_widget_inside_for() {
    // 解析由 For 局部 item 驱动的纯插槽行组件。
    let document = parse_document(
        // Row 没有 props/state，因此保持既有 For 内允许边界。
        r#"
        <Widget name="Row"><Container><Slot /></Container></Widget>
        <Column>
          <For {item} in {items}>
            <Row><Text>{item}</Text></Row>
          </For>
        </Column>
        "#,
    )
    // 合法 For 插槽文档必须解析成功。
    .expect("For 内纯 Slot 组件应解析成功");
    // 生成调用方循环绑定与投影内容。
    let tokens = generate_document_view(&document)
        // Row 不得被误判为需要逐实例状态存储。
        .expect("For 内纯 Slot 组件应展开成功")
        // 转成稳定文本检查循环绑定。
        .to_string();
    // 调用方 item 必须留在 For 生成闭包作用域中。
    assert!(tokens.contains("item"));
}

// 验证外层组件可把自己的 Slot 投影继续传递给嵌套插槽组件。
#[test]
fn forwards_slot_projection_through_nested_widgets() {
    // 解析 Inner 容器、Outer 转发模板与拥有调用方状态的 Host。
    let document = parse_document(
        // Outer 的 Slot 作为 Inner 的默认插槽子节点继续转发。
        r#"
        <Widget name="Inner"><Container><Slot /></Container></Widget>
        <Widget name="Outer"><Inner><Slot /></Inner></Widget>
        <Widget name="Host" state="label: '嵌套内容'"><Outer><Text>{label}</Text></Outer></Widget>
        <Host />
        "#,
    )
    // 合法嵌套转发文档必须解析成功。
    .expect("嵌套 Slot 转发文档应解析成功");
    // 展开两层插槽组件。
    let tokens = generate_document_view(&document)
        // 调用方状态绑定必须穿过两层模板。
        .expect("嵌套 Slot 转发应展开成功")
        // 转成稳定文本检查最终内容。
        .to_string();
    // 最内层 Container 必须收到 Host 作用域展开的文本。
    assert!(tokens.contains("嵌套内容"));
    // 两层 Slot 占位均不得泄漏到最终令牌。
    assert!(!tokens.contains("Slot"));
}

// 验证模板插槽名称唯一且 name 只能使用字符串字面量。
#[test]
fn rejects_invalid_slot_declarations() {
    // 解析重复默认插槽。
    let duplicate_default = parse_document(
        // 同一组件模板声明两个默认占位。
        r#"<Widget name="Bad"><Container><Slot /><Slot /></Container></Widget><Bad />"#,
    )
    // 重复默认占位必须在声明阶段失败。
    .expect_err("重复默认 Slot 不得通过");
    // 诊断必须说明默认 Slot 重复。
    assert!(duplicate_default.message.contains("重复声明默认 Slot"));
    // 解析动态 name 属性。
    let dynamic_name = parse_document(
        // 插槽登记必须在编译期闭合。
        r#"<Widget name="Bad"><Slot name={target} /></Widget><Bad />"#,
    )
    // 动态插槽名必须失败。
    .expect_err("动态 Slot name 不得通过");
    // 诊断必须说明字符串字面量约束。
    assert!(dynamic_name.message.contains("非空字符串字面量"));
}

// 验证调用方归位名称存在且 slot 属性不能动态求值。
#[test]
fn rejects_unknown_or_dynamic_slot_targets() {
    // 解析指向不存在具名插槽的调用方子节点。
    let unknown = parse_document(
        // Panel 只声明默认插槽。
        r#"<Widget name="Panel"><Container><Slot /></Container></Widget><Panel><Text slot="footer">操作</Text></Panel>"#,
    )
    // 声明语法本身合法。
    .expect("未知 slot 目标应在组件展开阶段诊断");
    // 读取不存在目标诊断。
    let unknown_error = generate_document_view(&unknown)
        // 不存在具名插槽不得生成。
        .expect_err("未知 slot 目标必须失败");
    // 诊断必须包含具体名称。
    assert!(unknown_error.message.contains("footer"));
    // 解析动态归位目标。
    let dynamic = parse_document(
        // Panel 声明具名 footer，但调用方使用表达式。
        r#"<Widget name="Panel"><Container><Slot name="footer" /></Container></Widget><Panel><Text slot={target}>操作</Text></Panel>"#,
    )
    // 声明语法本身合法。
    .expect("动态 slot 目标应在组件展开阶段诊断");
    // 读取字面量约束诊断。
    let dynamic_error = generate_document_view(&dynamic)
        // 动态 slot 属性不得生成。
        .expect_err("动态 slot 目标必须失败");
    // 诊断必须说明字符串字面量约束。
    assert!(dynamic_error.message.contains("字符串字面量"));
}

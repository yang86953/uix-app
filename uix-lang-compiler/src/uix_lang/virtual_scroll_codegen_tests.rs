// 引入文档解析、公开 View 生成与诊断类型。
use super::{Diagnostic, generate_view, parse_document};

// 解析单根文档并返回稳定生成令牌。
fn generate(source: &str) -> Result<String, Diagnostic> {
    // 解析并验证 UIX 文档结构。
    let document = parse_document(source)?;
    // 生成公开 View 令牌。
    let tokens = generate_view(&document.root)?;
    // 使用 proc_macro2 的稳定空白形式生成断言文本。
    Ok(tokens.to_string())
}

// 验证 VirtualScroll 只求值一次数据并生成惰性 renderer。
#[test]
fn generates_virtual_scroll_snapshot_and_identity_contract() {
    // 覆盖数据、行高、冗余 item 一致性、索引、key 与公共高度。
    let source = r#"<VirtualScroll data={logs} rowHeight="32px" item={log} height="160px"><For {log} {index} in {logs} key={log.id}><Container direction="row"><Text>{index}</Text><Text>{log.line}</Text></Container></For></VirtualScroll>"#;
    // 生成确定性虚拟列表令牌。
    let tokens = generate(source).expect("VirtualScroll 完整契约应生成 Rust View");
    // 必须复用公开 VirtualScroll 构造器。
    assert!(tokens.contains("prelude :: VirtualScroll :: new"));
    // 项目总数必须来自一次性数据快照的 len。
    assert!(tokens.contains("item_count") && tokens.contains("len"));
    // 固定行高必须进入公开运行时构建器。
    assert!(tokens.contains("item_height (32"));
    // 带业务 key 的模板必须在行构建前走 keyed renderer。
    assert!(tokens.contains("render_keyed"));
    // 用户行变量必须从数据快照的当前索引克隆。
    assert!(tokens.contains("let log") && tokens.contains("clone"));
    // 用户索引绑定必须接收运行时绝对索引。
    assert!(tokens.contains("let index = __uix_virtual_index"));
    // 行 renderer 不得再次设置第二份 ViewNode key。
    assert!(!tokens.contains(". key ("));
    // key 表达式必须保留业务 id 字段访问。
    assert!(tokens.contains("id"));
    // 公共高度必须继续走统一 View 样式契约。
    assert!(tokens.contains("height (160"));
    // data 与 For in 的同一表达式不得在生成物中重复求值。
    assert_eq!(tokens.matches("logs").count(), 1);
}

// 验证文档示例在不声明冗余 item 与 key 时仍生成后备索引身份。
#[test]
fn generates_documented_virtual_scroll_shape_without_item_attribute() {
    // 使用文档中的最小 data、rowHeight 与 For 结构。
    let source = r#"<VirtualScroll data={logs} rowHeight="32px"><For {log} in {logs}><Text>{log.line}</Text></For></VirtualScroll>"#;
    // 生成最小虚拟列表。
    let tokens = generate(source).expect("文档 VirtualScroll 示例应可生成");
    // 无业务 key 的模板继续使用绝对索引 renderer。
    assert!(tokens.contains("render (move | __uix_virtual_index |"));
    // 无业务 key 时不得生成 keyed renderer。
    assert!(!tokens.contains("render_keyed"));
    // 未声明 key 时不应伪造业务 key 调用。
    assert!(!tokens.contains("format !") && !tokens.contains(". key"));
    // renderer 仍必须从数据快照按绝对索引克隆行值。
    assert!(tokens.contains("__uix_virtual_data") && tokens.contains("__uix_virtual_index"));
}

// 验证带嵌套 AST 的同一数据表达式忽略源码位置且仍只求值一次。
#[test]
fn normalizes_virtual_scroll_data_expression_before_consistency_check() {
    // 两处函数调用具有不同源码跨度但相同求值语义。
    let source = r#"<VirtualScroll data={load_logs()} rowHeight="32px"><For {log} in {load_logs()}><Text>{log.line}</Text></For></VirtualScroll>"#;
    // 规范化比较必须接受同一调用表达式。
    let tokens = generate(source).expect("同一函数调用数据源应通过结构一致性检查");
    // 运行时代码中只能保留一次实际函数调用。
    assert_eq!(tokens.matches("load_logs").count(), 1);
}

// 验证 VirtualScroll 的必需属性与数据表达式形状。
#[test]
fn validates_virtual_scroll_required_attributes() {
    // 缺少 data 时无法确定项目总数。
    let missing_data = generate(
        // 保留其他结构以隔离 data 诊断。
        r#"<VirtualScroll rowHeight="32px"><For {item} in {items}><Text>{item}</Text></For></VirtualScroll>"#,
    )
    // 提取预期缺失属性诊断。
    .expect_err("缺少 data 必须失败");
    // 诊断必须点名 data。
    assert!(missing_data.message.contains("data"));
    // 缺少 rowHeight 时无法建立定高窗口。
    let missing_height = generate(
        // 保留其他结构以隔离 rowHeight 诊断。
        r#"<VirtualScroll data={items}><For {item} in {items}><Text>{item}</Text></For></VirtualScroll>"#,
    )
    // 提取预期缺失行高诊断。
    .expect_err("缺少 rowHeight 必须失败");
    // 诊断必须点名 rowHeight。
    assert!(missing_height.message.contains("rowHeight"));
    // 字符串 data 不能充当数组表达式。
    let literal_data = generate(
        // 使用非法字符串数据源。
        r#"<VirtualScroll data="items" rowHeight="32px"><For {item} in {items}><Text>{item}</Text></For></VirtualScroll>"#,
    )
    // 提取预期表达式形状诊断。
    .expect_err("字符串 data 必须失败");
    // 修复建议必须给出花括号写法。
    assert!(literal_data.message.contains("花括号") && literal_data.suggestion.contains("data={"));
}

// 验证 VirtualScroll 只有一个直接 For 模板和一个稳定行根。
#[test]
fn validates_virtual_scroll_template_shape() {
    // 普通元素不能替代惰性 For 模板。
    let direct = generate(
        // 构造直接 Text 子节点。
        r#"<VirtualScroll data={items} rowHeight="32px"><Text>all</Text></VirtualScroll>"#,
    )
    // 提取预期直接模板诊断。
    .expect_err("非 For 直接子节点必须失败");
    // 诊断必须要求直接 For。
    assert!(direct.message.contains("直接子节点") && direct.suggestion.contains("For"));
    // 多个直接 For 会产生相互竞争的 renderer。
    let multiple_templates = generate(
        // 构造两个直接模板。
        r#"<VirtualScroll data={items} rowHeight="32px"><For {item} in {items}><Text>{item}</Text></For><For {item} in {items}><Text>{item}</Text></For></VirtualScroll>"#,
    )
    // 提取预期唯一模板诊断。
    .expect_err("多个 For 模板必须失败");
    // 诊断必须说明恰好一个。
    assert!(multiple_templates.message.contains("恰好包含一个"));
    // 一个 For 内的多个行根不能由 renderer 同时返回。
    let multiple_rows = generate(
        // 构造两个直接行根。
        r#"<VirtualScroll data={items} rowHeight="32px"><For {item} in {items}><Text>A</Text><Text>B</Text></For></VirtualScroll>"#,
    )
    // 提取预期唯一行根诊断。
    .expect_err("多个行根必须失败");
    // 修复建议必须要求显式布局容器。
    assert!(
        multiple_rows.message.contains("一个直接行根")
            && multiple_rows.suggestion.contains("Container")
    );
}

// 验证 VirtualScroll 不接受相互矛盾的数据源与行变量声明。
#[test]
fn validates_virtual_scroll_binding_consistency() {
    // data 与 For in 不一致会让项目总数和行值来自不同数组。
    let data_mismatch = generate(
        // 构造两个不同的数据源。
        r#"<VirtualScroll data={items} rowHeight="32px"><For {item} in {others}><Text>{item}</Text></For></VirtualScroll>"#,
    )
    // 提取预期数据源冲突诊断。
    .expect_err("不一致数据源必须失败");
    // 诊断必须说明 data 与 For in 冲突。
    assert!(data_mismatch.message.contains("data") && data_mismatch.message.contains("in"));
    // 冗余 item 与 For 行变量不一致必须失败。
    let item_mismatch = generate(
        // 构造冲突行变量名。
        r#"<VirtualScroll data={items} rowHeight="32px" item={row}><For {item} in {items}><Text>{item}</Text></For></VirtualScroll>"#,
    )
    // 提取预期变量冲突诊断。
    .expect_err("不一致 item 必须失败");
    // 诊断必须保留两个名称并给出删除冗余属性的方案。
    assert!(
        // 检查实际 item 名称。
        item_mismatch.message.contains("row")
            // 检查 For 行变量名称。
            && item_mismatch.message.contains("item")
            // 检查最小修复动作。
            && item_mismatch.suggestion.contains("删除 item")
    );
}

// 验证 VirtualScroll 业务 key 不会捕获 renderer 外部状态或执行调用。
#[test]
fn rejects_virtual_scroll_key_external_capture_and_calls() {
    // 外部前缀会被 key 与行两个 move 闭包竞争，并且无法形成树级响应式绑定。
    let external = generate(
        // 构造同时读取外部标识符与当前项成员的 key。
        r#"<VirtualScroll data={items} rowHeight="32px"><For {item} in {items} key={prefix + item.id}><Text>{item.name}</Text></For></VirtualScroll>"#,
    )
    // 提取预期的局部依赖诊断。
    .expect_err("VirtualScroll key 不得捕获外部标识符");
    // 诊断必须指明只允许当前 item 或 index。
    assert!(external.message.contains("item") && external.message.contains("index"));
    // 调用表达式可能产生副作用，不能作为捕获前稳定身份工厂。
    let call = generate(
        // 构造调用当前项成员方法的 key。
        r#"<VirtualScroll data={items} rowHeight="32px"><For {item} in {items} key={item.id.to_string()}><Text>{item.name}</Text></For></VirtualScroll>"#,
    )
    // 提取预期的纯身份诊断。
    .expect_err("VirtualScroll key 不得包含调用");
    // 诊断必须明确拒绝调用结构。
    assert!(call.message.contains("调用"));
    // 字符串字面量会被降低为组件准备区的转换器调用，key 工厂闭包内不可见。
    let string_literal = generate(
        // 构造包含字符串字面量的 key。
        r#"<VirtualScroll data={items} rowHeight="32px"><For {item} in {items} key={'row-' + item.id}><Text>{item.name}</Text></For></VirtualScroll>"#,
    )
    // 提取预期的字面量诊断。
    .expect_err("VirtualScroll key 不得包含字符串字面量");
    // 诊断必须指向 item 字段方案。
    assert!(
        string_literal.message.contains("字符串字面量")
            && string_literal.suggestion.contains("item")
    );
}

// 验证 VirtualScroll 行 For 的实例作用域在 renderer 闭包内自包含建立。
#[test]
fn generates_row_scope_inside_renderer_closure() {
    // 行模板包含嵌套 For 与字符串比较时，依赖闭包内声明的路径与准备语句；
    // 嵌套 For 的实例身份需要完整组件展开入口（与公开 uix! 一致）。
    let source = r#"<VirtualScroll data={logs} rowHeight="32px"><For {log} in {logs} key={log.id}><Container direction="row"><For {tag} in {log.tags}><Text>{tag}</Text></For><If {log.line != ''}><Text>{log.line}</Text></If></Container></For></VirtualScroll>"#;
    // 经完整展开生成包含嵌套结构与比较的行模板令牌。
    let tokens = super::generate_test_document_view(source)
        .expect("行内嵌套 For 与字符串比较应生成行作用域");
    // keyed renderer 保持不变。
    assert!(tokens.contains("render_keyed"));
    // renderer 闭包内必须声明行 For 实例路径（嵌套 For 的父级身份来源）。
    assert!(tokens.contains("virtual-scroll-row"));
    // 行模板准备语句（字符串字面量转换器）必须出现在同一闭包内。
    assert!(tokens.contains("owned_string"));
}

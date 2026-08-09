// 引入文档解析与核心 View 生成入口。
use super::{generate_view, parse_document};

// 解析单根文档并生成稳定令牌文本。
fn generate(source: &str) -> Result<String, super::Diagnostic> {
    // 解析已验证文档。
    let document = parse_document(source)?;
    // 生成根 View 令牌。
    let tokens = generate_view(&document.root)?;
    // 使用 proc_macro2 的稳定空白规范形成快照。
    Ok(tokens.to_string())
}

// 验证元素、属性、文本插值和事件生成使用公开 API 且保持顺序。
#[test]
fn generates_core_view_snapshot_in_source_order() {
    // 构造覆盖核心映射的单行文档以排除排版空白。
    let source = r#"<Container direction="row" gap="8px" align="center"><Text fontSize="heading2">Count: {count}</Text><Button type="primary" disabled={busy} @click="onConfirm()">Save</Button><Icon name="star" size="16px" /></Container>"#;
    // 生成确定性令牌快照。
    let snapshot = generate(source).expect("核心元素应生成 Rust View");
    // 锁定完整 TokenStream 快照。
    assert_eq!(
        // 比较实际令牌。
        snapshot,
        // 保存公开 API、属性链与子节点顺序的稳定快照。
        r#":: uix :: prelude :: View :: build (((:: uix :: prelude :: row ({ let mut __uix_children = :: std :: vec :: Vec :: < :: uix :: prelude :: ViewNode > :: new () ; __uix_children . push (:: uix :: prelude :: View :: build ((:: uix :: prelude :: label ({ let mut __uix_text = :: std :: string :: String :: new () ; __uix_text . push_str ("Count: ") ; __uix_text . push_str (& :: std :: string :: ToString :: to_string (& (count))) ; __uix_text })) . font_size (:: uix :: prelude :: TypographyToken :: Heading2))) ; __uix_children . push (:: uix :: prelude :: View :: build ((((:: uix :: prelude :: button ("Save")) . primary ()) . disabled (busy)) . on_click_fn (move || { let _ = { (onConfirm) () } ; }))) ; __uix_children . push (:: uix :: prelude :: View :: build (:: uix :: prelude :: ViewNode :: leaf ((:: uix :: prelude :: Icon :: new ("star")) . size (16.0)))) ; __uix_children })) . gap (8.0)) . align (:: uix :: prelude :: AlignItems :: Center))"#
    );
}

// 验证 If 与 For 生成真实 Rust 控制流、索引和稳定 key。
#[test]
fn generates_if_for_and_key_snapshot() {
    // 构造条件与带索引、key 的循环文档。
    let source = r#"<Column><If {visible}><Text>{title}</Text></If><For {item} {index} in {items} key={item.id}><Row><Text>{index}</Text><Text>{item.name}</Text></Row></For></Column>"#;
    // 生成确定性令牌快照。
    let snapshot = generate(source).expect("If 与 For 应生成 Rust 控制流");
    // 锁定完整 TokenStream 快照。
    assert_eq!(
        // 比较实际令牌。
        snapshot,
        // 保存 If、For、enumerate 与 key 的稳定快照。
        r#":: uix :: prelude :: View :: build (:: uix :: prelude :: column ({ let mut __uix_children = :: std :: vec :: Vec :: < :: uix :: prelude :: ViewNode > :: new () ; if visible { __uix_children . push (:: uix :: prelude :: View :: build (:: uix :: prelude :: label ({ let mut __uix_text = :: std :: string :: String :: new () ; __uix_text . push_str (& :: std :: string :: ToString :: to_string (& (title))) ; __uix_text }))) ; } for (index , item) in (:: std :: iter :: IntoIterator :: into_iter ((items) . clone ())) . enumerate () { let __uix_for_view = :: uix :: prelude :: View :: build (:: uix :: prelude :: row ({ let mut __uix_children = :: std :: vec :: Vec :: < :: uix :: prelude :: ViewNode > :: new () ; __uix_children . push (:: uix :: prelude :: View :: build (:: uix :: prelude :: label ({ let mut __uix_text = :: std :: string :: String :: new () ; __uix_text . push_str (& :: std :: string :: ToString :: to_string (& (index))) ; __uix_text }))) ; __uix_children . push (:: uix :: prelude :: View :: build (:: uix :: prelude :: label ({ let mut __uix_text = :: std :: string :: String :: new () ; __uix_text . push_str (& :: std :: string :: ToString :: to_string (& ((item) . name))) ; __uix_text }))) ; __uix_children })) ; __uix_children . push (__uix_for_view . key (:: std :: format ! ("{}" , (item) . id))) ; } __uix_children }))"#
    );
}

// 验证事件保留参数映射到公开点击载荷与坐标字段。
#[test]
fn maps_event_payload_and_coordinates() {
    // 构造读取点击坐标的处理器。
    let source = r#"<Button @click="onClick($event.x, $event.y)">Open</Button>"#;
    // 生成事件令牌。
    let snapshot = generate(source).expect("$event 应映射到点击载荷");
    // 必须使用带语义事件的公开入口。
    assert!(snapshot.contains("on_click_event"));
    // 必须从语义事件提取点击载荷。
    assert!(snapshot.contains("click_payload"));
    // x 必须映射到 ClickEvent.pos.x。
    assert!(snapshot.contains("pos . x"));
    // y 必须映射到 ClickEvent.pos.y。
    assert!(snapshot.contains("pos . y"));
}

// 验证三元表达式与不可变数组操作转换为确定的 Rust 结构。
#[test]
fn maps_ternary_and_immutable_array_operations() {
    // 构造同时覆盖 push、removeAt、length 与三元表达式的插值。
    let source =
        r#"<Text>{flag ? values.push(item).length : values.removeAt(index).length}</Text>"#;
    // 生成表达式令牌。
    let snapshot = generate(source).expect("数组操作与三元表达式应生成 Rust 代码");
    // 三元表达式必须翻译为 Rust if。
    assert!(snapshot.contains("if flag"));
    // 两个不可变操作都必须克隆原数组。
    assert_eq!(snapshot.matches("clone").count(), 2);
    // push 必须追加到克隆数组。
    assert!(snapshot.contains("push (item)"));
    // removeAt 必须映射为 Vec::remove。
    assert!(snapshot.contains("remove (index)"));
    // length 必须映射为 len 调用。
    assert_eq!(snapshot.matches("len ()").count(), 2);
}

// 验证事件保留参数不能逃逸到普通文本表达式。
#[test]
fn rejects_event_parameter_outside_handler() {
    // 解析普通文本中的事件保留参数。
    let document = parse_document(r#"<Text>{$event.x}</Text>"#)
        // 表达式语法本身保持上下文无关。
        .expect("表达式语法本身应合法");
    // View 生成必须执行作用域检查。
    let error = generate_view(&document.root).expect_err("$event 不得逃逸事件处理器");
    // 诊断必须说明事件作用域。
    assert!(error.message.contains("只能在事件处理器中使用"));
}

// 验证未登记元素不会被静默猜测为 Rust API。
#[test]
fn rejects_unregistered_element_mapping() {
    // 解析尚未进入核心矩阵的 Input。
    let document = parse_document(r#"<Input value={name} />"#).expect("语法本身应合法");
    // 代码生成必须返回登记诊断。
    let error = generate_view(&document.root).expect_err("未登记元素必须失败");
    // 诊断必须明确缺少 Rust API 映射。
    assert!(error.message.contains("尚无已登记的 Rust API 映射"));
}

// 验证未知属性不会从生成代码中静默消失。
#[test]
fn rejects_unregistered_attribute_mapping() {
    // 解析带未知属性的 Text。
    let document = parse_document(r#"<Text mystery="value">Hello</Text>"#)
        // 语法层应接受可扩展属性名。
        .expect("语法本身应合法");
    // 代码生成必须返回属性映射诊断。
    let error = generate_view(&document.root).expect_err("未登记属性必须失败");
    // 诊断必须包含具体属性名。
    assert!(error.message.contains("mystery"));
}

// 验证样式映射明确留给对应后续 Gate。
#[test]
fn rejects_style_until_style_mapping_gate() {
    // 解析带内联样式的 Text。
    let document = parse_document(r#"<Text style="color: #fff;">Hello</Text>"#)
        // 样式语法应已由前置 Gate 验证。
        .expect("样式语法本身应合法");
    // 核心 View Gate 不得伪装支持样式。
    let error = generate_view(&document.root).expect_err("样式应明确延后");
    // 诊断必须指向样式映射阶段。
    assert!(error.message.contains("样式映射"));
}

// 验证 setState 明确需要后续 Component 上下文。
#[test]
fn rejects_set_state_without_component_context() {
    // 解析包含 setState 的按钮事件。
    let document = parse_document(
        // 使用当前表达式 Gate 已接受的命名参数形式。
        r#"<Button @click="setState(count: count + 1)">Add</Button>"#,
    )
    // 语法层应接受组件内置操作。
    .expect("setState 语法本身应合法");
    // 当前核心 View 生成必须拒绝缺失状态上下文。
    let error = generate_view(&document.root).expect_err("setState 需要 Component Gate");
    // 诊断必须明确 Component state 上下文。
    assert!(error.message.contains("Component state"));
}

// 验证带 key 的循环要求唯一直接行根。
#[test]
fn rejects_keyed_for_with_multiple_direct_roots() {
    // 构造会生成两个直接行节点的循环。
    let source = r#"<Column><For {item} in {items} key={item.id}><Text>A</Text><Text>B</Text></For></Column>"#;
    // 解析控制结构。
    let document = parse_document(source).expect("For 语法本身应合法");
    // 代码生成必须拒绝不稳定的多根 key。
    let error = generate_view(&document.root).expect_err("key 需要唯一行根");
    // 诊断必须说明唯一直接子节点约束。
    assert!(error.message.contains("恰好生成一个直接子节点"));
}

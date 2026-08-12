// 引入组件感知生成入口与文档解析器。
use super::{generate_document_view, parse_document};

// 验证私有状态、回调 prop 与静态组件组合生成确定性令牌。
#[test]
fn generates_state_callback_and_composition_tokens() {
    // 解析两层组件组合与私有计数状态。
    let document = parse_document(
        // 使用静态字符串保存完整消费场景。
        r#"
        <Component name="Counter" props="label: String, onDone: () -> bool" state="count: 0">
          <Column>
            <Text>{label}: {count}</Text>
            <Button @click="setState(count: count + 1)">+</Button>
            <Button @click="onDone()">Done</Button>
          </Column>
        </Component>
        <Component name="Panel" props="title: String, onClose: () -> bool">
          <Counter label={title} onDone={onClose} />
        </Component>
        <Panel title="Count" onClose={do_close} />
        "#,
    )
    // 合法组件文档必须解析成功。
    .expect("组合组件文档应解析成功");
    // 生成完整组件感知 View 令牌。
    let tokens = generate_document_view(&document)
        // 合法组件文档必须生成成功。
        .expect("组合组件应生成成功")
        // 转成稳定文本便于检查关键语义。
        .to_string();
    // 私有 number 状态应固定为 f64 State。
    assert!(tokens.contains("State < f64 >"));
    // 私有 state 必须经由窗口作用域复用接口取得，而不是在每次 View 构建时直接新建。
    assert!(tokens.contains("uix_component_state"));
    // 私有 state 令牌不得再直接调用 State::new，避免 reconcile 时重置组件状态。
    assert!(!tokens.contains("State :: new"));
    // 拥有私有 state 的组件调用必须声明运行时作用域以便关联 View 生命周期。
    assert!(tokens.contains("uix_component_scope"));
    // setState 应降低为公开 State::set。
    assert!(tokens.contains(". set"));
    // 回调应生成显式 Fn 类型适配器。
    assert!(tokens.contains("Arc < dyn Fn () -> bool >"));
    // Rust 外层回调名称应保留到最终令牌。
    assert!(tokens.contains("do_close"));
    // 自定义标签名不应泄漏给核心元素生成器。
    assert!(!tokens.contains("Panel"));
    // 嵌套自定义标签也应完成展开。
    assert!(!tokens.contains("Counter"));
}

// 验证同一静态组件的多个调用各自生成不同的作用域局部变量与生命周期标记。
#[test]
fn generates_distinct_scopes_for_multiple_static_component_calls() {
    // 解析两个相同组件的静态调用。
    let document = parse_document(
        // 使用同一私有 state 组件的相邻调用覆盖声明身份分配。
        r#"
        <Component name="Counter" state="count: 0"><Button @click="setState(count: count + 1)">{count}</Button></Component>
        <Column><Counter /><Counter /></Column>
        "#,
    )
    // 两个静态调用的组件文档必须解析成功。
    .expect("多实例组件文档应解析成功");
    // 生成完整展开令牌。
    let tokens = generate_document_view(&document)
        // 多实例组件调用必须生成成功。
        .expect("多实例组件应生成成功")
        // 转换成稳定文本以检查内部运行时接口。
        .to_string();
    // 两次静态调用必须各自产生一次运行时作用域获取。
    assert_eq!(tokens.matches("uix_component_scope").count(), 4);
    // 两次静态调用必须使用不同的卫生作用域局部变量，避免状态句柄串用。
    assert!(tokens.contains("__uix_component_scope_0_Counter"));
    // 第二次调用必须继续分配新的卫生作用域局部变量。
    assert!(tokens.contains("__uix_component_scope_3_Counter"));
    // 每个实际根都必须携带对应的 ViewNode 生命周期作用域标记。
    assert_eq!(tokens.matches(". uix_component_scope").count(), 2);
}

// 验证嵌套私有 state 组件在同一实际根上保留外层与内层两个生命周期标记。
#[test]
fn generates_all_nested_component_scope_markers_on_one_root() {
    // 解析外层组件直接展开为内层私有 state 组件的单根组合。
    let document = parse_document(
        // 外层和内层均拥有私有 state，以覆盖同根多标记契约。
        r#"
        <Component name="Counter" state="count: 0"><Button @click="setState(count: count + 1)">{count}</Button></Component>
        <Component name="Panel" state="visible: true"><Counter /></Component>
        <Panel />
        "#,
    )
    // 嵌套组件文档必须解析成功。
    .expect("嵌套组件文档应解析成功");
    // 生成完整展开令牌。
    let tokens = generate_document_view(&document)
        // 嵌套组件必须生成成功。
        .expect("嵌套组件应生成成功")
        // 转换成稳定文本以检查链式元数据。
        .to_string();
    // 外层与内层私有 state 调用都必须取得各自作用域。
    assert_eq!(tokens.matches("uix_component_scope").count(), 4);
    // 同一 Button 根必须被连续标记两次，而非由后层覆盖前层标记。
    assert!(tokens.contains(". uix_component_scope"));
    // 两层私有 state 都必须经由运行时状态复用接口取得句柄。
    assert_eq!(tokens.matches("uix_component_state").count(), 2);
}

// 验证 For 内可递归展开完全无 prop 和无私有 state 的静态组件组合。
#[test]
fn for_nested_allows_pure_static_components() {
    // 解析两层纯静态组件与外层 For 调用。
    let document = parse_document(
        // 内外组件均不声明 prop 或私有 state，因此无需逐实例存储。
        r#"
        <Component name="Leaf"><Text>固定行</Text></Component>
        <Component name="Row"><Leaf /></Component>
        <Column><For {item} in {items}><Row /></For></Column>
        "#,
    )
    // 该文档的动态数据绑定在生成阶段保持合法。
    .expect("纯静态嵌套组件应能位于 For 内");
    // 生成完整令牌以检查 For 与叶子节点均被保留。
    let tokens = generate_document_view(&document)
        // 无状态静态组件不应触发逐实例存储诊断。
        .expect("纯静态嵌套组件应生成成功")
        // 转为稳定文本以断言控制流和叶子内容。
        .to_string();
    // For 控制流必须保留在生成结果中。
    assert!(tokens.contains("for item in"));
    // 最内层静态组件必须已在 For 体内展开。
    assert!(tokens.contains("固定行"));
}

// 验证 For 内的纯静态外层组件不能掩盖内层私有 state 的逐实例存储需求。
#[test]
fn for_nested_rejects_private_state_component() {
    // 解析外层无状态组件包裹内层私有 state 组件的组合。
    let document = parse_document(
        // Leaf 的私有 state 必须在每个 For 项中独立拥有，但当前编译期契约尚未提供该存储。
        r#"
        <Component name="Leaf" state="selected: false"><Text>{selected}</Text></Component>
        <Component name="Row"><Leaf /></Component>
        <Column><For {item} in {items}><Row /></For></Column>
        "#,
    )
    // 语法与组件声明本身均应合法。
    .expect("内层 state 应在生成阶段诊断");
    // 读取跨越嵌套组件边界的 For 诊断。
    let error = generate_document_view(&document)
        // 不能为动态实例错误复用同一个私有状态槽。
        .expect_err("For 内嵌套私有 state 组件必须失败");
    // 诊断必须指向实际需要逐实例存储的内层组件。
    assert!(error.message.contains("<Leaf>"));
    // 诊断必须说明该限制来自 For 动态实例边界。
    assert!(error.message.contains("For 内"));
}

// 验证 For 内的纯静态外层组件不能掩盖内层 prop 的逐实例存储需求。
#[test]
fn for_nested_rejects_prop_component() {
    // 解析外层无状态组件向内层 prop 组件传入固定属性的组合。
    let document = parse_document(
        // 即使传入字面量，Leaf 仍具有 prop 契约且需要逐实例绑定边界。
        r#"
        <Component name="Leaf" props="label: String"><Text>{label}</Text></Component>
        <Component name="Row"><Leaf label="固定" /></Component>
        <Column><For {item} in {items}><Row /></For></Column>
        "#,
    )
    // 语法与 prop 对应关系本身均应合法。
    .expect("内层 prop 应在生成阶段诊断");
    // 读取跨越嵌套组件边界的 For 诊断。
    let error = generate_document_view(&document)
        // 不能让动态实例共享一个内层 prop 绑定上下文。
        .expect_err("For 内嵌套 prop 组件必须失败");
    // 诊断必须指向实际带 prop 的内层组件。
    assert!(error.message.contains("<Leaf>"));
    // 诊断必须说明该限制来自 For 动态实例边界。
    assert!(error.message.contains("For 内"));
}

// 验证 State<T> prop 读取与写入同一共享句柄。
#[test]
fn generates_shared_state_prop_tokens() {
    // 解析共享状态组件。
    let document = parse_document(
        // 使用公开 State<number> 契约。
        r#"
        <Component name="SharedCounter" props="count: State<number>">
          <Column>
            <Text>{count}</Text>
            <Button @click="setState(count: count + 1)">+</Button>
          </Column>
        </Component>
        <SharedCounter count={shared_count} />
        "#,
    )
    // 合法共享状态文档必须解析成功。
    .expect("共享状态文档应解析成功");
    // 生成完整令牌。
    let tokens = generate_document_view(&document)
        // 合法共享状态应生成成功。
        .expect("共享状态组件应生成成功")
        // 转换为文本。
        .to_string();
    // 共享句柄应保留调用方名称。
    assert!(tokens.contains("shared_count"));
    // State prop 应固定内部 number 类型。
    assert!(tokens.contains("State < f64 >"));
    // 读取应调用同一句柄的 get。
    assert!(tokens.contains(". get"));
    // 更新应调用句柄的 set。
    assert!(tokens.contains(". set"));
}

// 验证缺失与未知 props 在生成阶段给出结构化诊断。
#[test]
fn rejects_missing_and_unknown_component_props() {
    // 解析缺少必需 prop 的组件调用。
    let missing = parse_document(
        // 声明一个必需 String prop。
        r#"<Component name="Greeting" props="name: String"><Text>{name}</Text></Component><Greeting />"#,
    )
    // 声明本身合法。
    .expect("缺失 prop 应在生成阶段诊断");
    // 读取缺失 prop 诊断。
    let missing_error = generate_document_view(&missing)
        // 调用必须被拒绝。
        .expect_err("缺失必需 prop 必须失败");
    // 诊断应指出缺失字段。
    assert!(missing_error.message.contains("缺少必需 prop name"));

    // 解析包含未知 prop 的组件调用。
    let unknown = parse_document(
        // 额外传入组件未声明字段。
        r#"<Component name="Greeting" props="name: String"><Text>{name}</Text></Component><Greeting name="UIX" extra="x" />"#,
    )
    // 声明本身合法。
    .expect("未知 prop 应在生成阶段诊断");
    // 读取未知 prop 诊断。
    let unknown_error = generate_document_view(&unknown)
        // 调用必须被拒绝。
        .expect_err("未知 prop 必须失败");
    // 诊断应指出未知字段。
    assert!(unknown_error.message.contains("未声明 prop extra"));
}

// 验证 setState 只能写入私有或共享响应式状态。
#[test]
fn rejects_read_only_and_unknown_set_state_targets() {
    // 解析尝试写入普通 prop 的组件。
    let read_only = parse_document(
        // setState 目标为 String 普通 prop。
        r#"<Component name="Editor" props="label: String"><Button @click="setState(label: 'x')">Edit</Button></Component><Editor label="A" />"#,
    )
    // 声明本身合法。
    .expect("只读写入应在生成阶段诊断");
    // 读取只读 prop 诊断。
    let read_only_error = generate_document_view(&read_only)
        // 只读 prop 更新必须失败。
        .expect_err("setState 不能写入普通 prop");
    // 诊断应明确只读性质。
    assert!(read_only_error.message.contains("不能写入只读 prop label"));

    // 解析尝试写入不存在状态的组件。
    let unknown = parse_document(
        // setState 目标未在 state 或 props 声明。
        r#"<Component name="Editor"><Button @click="setState(missing: 1)">Edit</Button></Component><Editor />"#,
    )
    // 声明本身合法。
    .expect("越界目标应在生成阶段诊断");
    // 读取越界目标诊断。
    let unknown_error = generate_document_view(&unknown)
        // 越界更新必须失败。
        .expect_err("setState 越界目标必须失败");
    // 诊断应指出目标来源范围。
    assert!(unknown_error.message.contains("不是当前组件的 state"));
}

// 验证 setState 不能越过事件处理器作用域。
#[test]
fn rejects_set_state_outside_component_event() {
    // 解析在文本插值中执行状态更新的组件。
    let document = parse_document(
        // setState 位于普通插值而不是事件属性。
        r#"<Component name="Counter" state="count: 0"><Text>{setState(count: count + 1)}</Text></Component><Counter />"#,
    )
    // 语法形状合法但语义作用域应由生成器判断。
    .expect("越界 setState 应在组件生成阶段诊断");
    // 读取作用域诊断。
    let error = generate_document_view(&document)
        // 普通插值中的状态更新必须失败。
        .expect_err("setState 不能用于普通插值");
    // 诊断应明确事件处理器边界。
    assert!(error
        .message
        .contains("只能在 Component 的事件处理器中使用"));
}

// 验证组件递归调用在代码生成前被拒绝。
#[test]
fn rejects_recursive_component_composition() {
    // 解析直接自调用组件。
    let document = parse_document(
        // 组件体再次调用自身。
        r#"<Component name="Loop"><Loop /></Component><Loop />"#,
    )
    // 名称与结构解析合法。
    .expect("递归应在组件展开阶段诊断");
    // 读取递归诊断。
    let error = generate_document_view(&document)
        // 递归展开必须失败。
        .expect_err("递归组件必须失败");
    // 诊断应包含确定性闭环路径。
    assert!(error.message.contains("Loop -> Loop"));
}

// 验证样式类继承与内联覆盖在完整文档入口展开。
#[test]
fn expands_style_class_inheritance_before_view_codegen() {
    // 解析父子样式类和更高优先级的内联样式。
    let document = parse_document(
        // 子类覆盖颜色，内联样式覆盖父类内边距。
        r#"baseText { color: #ff0000; padding: 4px; } derivedText { extends: baseText; color: #0000ff; } <Text class="derivedText" style="padding: 8px;">Styled</Text>"#,
    )
    // 完整样式类文档必须解析成功。
    .expect("样式类文档应解析成功");
    // 展开完整文档并取得稳定令牌。
    let tokens = generate_document_view(&document)
        // 样式继承与覆盖必须生成成功。
        .expect("样式类应在编译期展开")
        // 规范化令牌文本。
        .to_string();
    // 最终生成物必须使用统一受控样式入口。
    assert!(tokens.contains("map_style"));
    // 颜色与内边距都必须进入 Style 更新。
    assert!(tokens.contains("color") && tokens.contains("padding"));
    // 生成物不得包含运行时 class 或 extends 解析。
    assert!(!tokens.contains("derivedText") && !tokens.contains("extends"));
}

// 验证未知样式类在使用位置产生明确诊断。
#[test]
fn rejects_unknown_style_class_reference() {
    // 解析引用缺失样式类的元素。
    let document = parse_document(r#"<Text class="missingStyle">Styled</Text>"#)
        // class 名称本身语法合法。
        .expect("未知 class 应在文档生成阶段诊断");
    // 展开必须拒绝缺失声明。
    let error = generate_document_view(&document).expect_err("未知 class 必须失败");
    // 诊断必须包含具体缺失类名。
    assert!(error.message.contains("missingStyle") && error.message.contains("未声明"));
}

// 验证样式类循环继承在任何 View 生成前被拒绝。
#[test]
fn rejects_style_class_inheritance_cycle() {
    // 解析两个互相继承的样式类。
    let document = parse_document(
        // 根元素引用其中一个类。
        r#"first { extends: second; color: red; } second { extends: first; color: blue; } <Text class="first">Styled</Text>"#,
    )
    // 声明语法本身合法。
    .expect("继承环应在样式解析器构造阶段诊断");
    // 构造解析器必须拒绝继承环。
    let error = generate_document_view(&document).expect_err("样式继承环必须失败");
    // 诊断必须给出闭合路径。
    assert!(
        error.message.contains("first -> second -> first")
            || error.message.contains("second -> first -> second")
    );
}

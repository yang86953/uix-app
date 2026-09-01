// 引入组件感知生成入口与文档解析器。
use super::{generate_document_view, parse_document};
// 引入内置组件状态句柄位登记查询。
use super::widget_codegen::is_state_handle_attribute;

// 验证 computed 依次读取 state、external、闭包捕获与先前派生值。
#[test]
fn generates_ordered_computed_bindings_without_cache() {
    // 解析包含数组筛选与后续长度派生的组件。
    let document = parse_document(
        // kept 捕获 threshold，remaining 再读取先声明的 kept。
        r#"
        <Widget name="Summary" state="threshold: usize = 1" computed="kept: items.filter(|item| item.id >= threshold), remaining: kept.length" external="items">
          <Text>{remaining}</Text>
        </Widget>
        <Summary />
        "#,
    )
    // 合法 computed 文档必须解析成功。
    .expect("有序 computed 文档应解析成功");
    // 生成完整组件感知 View 令牌。
    let tokens = generate_document_view(&document)
        // 派生值必须完成静态展开。
        .expect("有序 computed 应生成成功")
        // 转成稳定文本检查生成顺序与数组语义。
        .to_string();
    // 首个派生值必须生成卫生局部变量。
    let kept = tokens
        // 定位 kept 派生绑定。
        .find("_kept")
        // 缺少派生绑定时失败。
        .expect("应生成 kept 派生绑定");
    // 后续派生值必须生成独立卫生局部变量。
    let remaining = tokens
        // 定位 remaining 派生绑定。
        .find("_remaining")
        // 缺少派生绑定时失败。
        .expect("应生成 remaining 派生绑定");
    // Rust let 顺序必须与 computed 声明顺序一致。
    assert!(kept < remaining);
    // filter 必须保持不可变 clone 加 into_iter 的生成形状。
    assert!(tokens.contains("clone () . into_iter () . filter"));
    // 组件体必须读取后项派生的卫生名称。
    assert!(tokens.contains("__uix_computed_") && tokens.contains("_remaining"));
}

// 验证 computed 自引用与前向引用在展开阶段返回定向诊断。
#[test]
fn rejects_computed_forward_reference() {
    // 解析语法合法但依赖顺序非法的派生声明。
    let document = parse_document(
        // first 在 second 建立绑定前引用它。
        r#"<Widget name="Bad" computed="first: second + 1, second: 2"><Text>{first}</Text></Widget><Bad />"#,
    )
    // 声明语法本身必须解析成功。
    .expect("前向引用应在组件展开阶段诊断");
    // 读取有序依赖诊断。
    let error = generate_document_view(&document)
        // computed 前向引用不得生成。
        .expect_err("computed 前向引用必须失败");
    // 诊断必须包含尚未声明的具体名称。
    assert!(error.message.contains("尚未声明的派生值 second"));
    // 修复建议必须说明声明顺序约束。
    assert!(error.suggestion.contains("移到当前 computed 之前"));
}

// 验证 external 显式开放 Rust 符号且保留 For 局部变量与语言内建。
#[test]
fn accepts_declared_external_and_closed_widget_names() {
    // 解析外部格式化函数、循环绑定、状态与主题内建组合。
    let document = parse_document(
        // 只有 items 与 format 来自 Rust 调用点。
        r#"
        <Widget name="Rows" state="active: false" external="items, format">
          <Column>
            <For {item} {index} in {items} key={item.id}>
              <Text>{format(item.name)}: {index}</Text>
            </For>
            <Button @click="setState(active: !active)">{active}</Button>
            <Button @click="setTheme('dark')">主题</Button>
          </Column>
        </Widget>
        <Rows />
        "#,
    )
    // 合法封闭名称集合必须解析成功。
    .expect("external 与局部名称文档应解析成功");
    // 生成阶段必须接受全部已声明或内建名称。
    let tokens = generate_document_view(&document)
        // external 不承担 Rust 可见性判断。
        .expect("已声明 external 应成功生成")
        // 转成稳定文本检查外部名称保留。
        .to_string();
    // Rust 外部函数名称必须保留给调用点解析。
    assert!(tokens.contains("format"));
    // Rust 外部数据源名称必须保留给调用点解析。
    assert!(tokens.contains("items"));
}

// 验证 VirtualScroll item 一致性属性使用直接 For 的局部绑定而非 external。
#[test]
fn accepts_virtual_scroll_item_binding_inside_widget() {
    // 解析只把真实宿主数据源声明为 external 的虚拟滚动组件。
    let document = parse_document(
        // item 属性必须复用直接 For 绑定且不额外开放宿主名称。
        r#"
        <Widget name="Rows" external="items">
          <VirtualScroll data={items} rowHeight="32px" item={item}>
            <For {item} {index} in {items} key={item.id}>
              <Text>{index}: {item.name}</Text>
            </For>
          </VirtualScroll>
        </Widget>
        <Rows />
        "#,
    )
    // 合法局部绑定文档必须解析成功。
    .expect("VirtualScroll 组件文档应解析成功");
    // 生成阶段必须接受 item 与直接 For 绑定的一致性声明。
    let tokens = generate_document_view(&document)
        // item 不是需要声明的宿主 external。
        .expect("VirtualScroll item 局部绑定应生成成功")
        // 转成稳定文本确认数据源仍被生成。
        .to_string();
    // 唯一真实外部数据源必须保留到最终 Rust 令牌。
    assert!(tokens.contains("items"));
}

// 验证组件体未声明外部名称得到表达式位置诊断。
#[test]
fn rejects_undeclared_widget_external_at_expression_span() {
    // 解析缺少 external 的多行组件。
    let document = parse_document(
        // 把违规标识符放在稳定的第四行插值中。
        "<Widget name=\"Search\" props=\"count: number\">\n  <Column>\n    <Text>Count</Text>\n    <Text>{format(count)}</Text>\n  </Column>\n</Widget>\n<Search count={1} />",
    )
    // 声明语法本身合法。
    .expect("未声明 external 应在组件展开阶段诊断");
    // 读取组件封装诊断。
    let error = generate_document_view(&document)
        // 未声明 Rust 外部名称必须失败。
        .expect_err("未声明 external 不得生成");
    // 诊断必须包含具体名称。
    assert!(
        error.message.contains("未声明的外部符号 format"),
        "实际诊断：{error:?}"
    );
    // 诊断必须回到违规表达式所在 UIX 行。
    assert_eq!(error.span.line, 4);
    // 诊断必须提供可直接采用的 external 修复。
    assert!(error.suggestion.contains("external=\"format\""));
}

// 验证私有状态、回调 prop 与静态组件组合生成确定性令牌。
#[test]
fn generates_state_callback_and_composition_tokens() {
    // 解析两层组件组合与私有计数状态。
    let document = parse_document(
        // 使用静态字符串保存完整消费场景。
        r#"
        <Widget name="Counter" props="label: String, onDone: () -> bool" state="count: 0">
          <Column>
            <Text>{label}: {count}</Text>
            <Button @click="setState(count: count + 1)">+</Button>
            <Button @click="onDone()">Done</Button>
          </Column>
        </Widget>
        <Widget name="Panel" props="title: String, onClose: () -> bool">
          <Counter label={title} onDone={onClose} />
        </Widget>
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
    assert!(tokens.contains("uix_widget_state"));
    // 私有 state 令牌不得再直接调用 State::new，避免 reconcile 时重置组件状态。
    assert!(!tokens.contains("State :: new"));
    // 拥有私有 state 的组件调用必须声明运行时作用域以便关联 View 生命周期。
    assert!(tokens.contains("uix_widget_scope"));
    // setState 应降低为公开 State::set。
    assert!(tokens.contains(". set"));
    // 回调应生成显式 Fn 类型适配器。
    assert!(tokens.contains("Arc < dyn Fn () -> bool >"));
    // 回调来源必须在 move 适配器外先克隆，允许组件位于可重复 App 根工厂。
    assert!(tokens.contains("Clone :: clone") && tokens.contains("callback_source"));
    // Rust 外层回调名称应保留到最终令牌。
    assert!(tokens.contains("do_close"));
    // 无私有 state 的自定义标签不应泄漏到核心元素生成器。
    assert!(!tokens.contains("Panel"));
    // 私有 state 组件的自定义标签只允许出现在卫生作用域标识符中。
    assert!(tokens.contains("__uix_widget_scope_"));
}

// 验证 setStyle 生成组件私有闭合枚举、完整样式分支与非吞噬指针事件。
#[test]
fn generates_typed_dynamic_style_and_pointer_event_tokens() {
    // 解析原始类、目标类、内联覆盖及进入离开恢复组合。
    let document = parse_document(
        r##"
        baseCard { padding: 4px; color: red; }
        hoverCard { padding: 12px; color: blue; }
        <Widget name="HoverCard">
          <Text class="baseCard" style="color: #00ff00;" @click="setStyle('hoverCard')" @mouseEnter="setStyle('hoverCard')" @mouseLeave="setStyle('')">Hover</Text>
        </Widget>
        <HoverCard />
        "##,
    )
    // 合法动态样式文档必须解析成功。
    .expect("组件动态样式文档应解析成功");
    // 生成完整类型化令牌。
    let tokens = generate_document_view(&document)
        // 已声明目标类必须生成成功。
        .expect("组件动态样式应生成成功")
        // 转成稳定文本检查契约边界。
        .to_string();
    // 无显式 state 的动态样式组件仍必须取得组件作用域。
    assert!(tokens.contains("uix_widget_scope"));
    // 实际节点必须派生带 key/位置身份的子作用域。
    assert!(tokens.contains("uix_widget_child_scope"));
    // 运行时状态必须是闭合枚举而不是字符串注册表。
    assert!(tokens.contains("enum __uix_dynamic_style_") && tokens.contains("Class0"));
    // 进入离开必须复用指针事件入口。
    assert_eq!(tokens.matches(". on_pointer").count(), 2);
    // 相同目标出现在不同事件时必须生成独占 setter，避免闭包重复移动。
    assert_eq!(tokens.matches("set_style_").count() >= 3, true);
    // 两个处理器都必须继续交付组件自身指针事件。
    assert_eq!(tokens.matches("EventResult :: NotHandled").count(), 2);
    // 原始与目标类的 padding 都必须形成完整分支。
    assert!(tokens.contains("padding") && tokens.contains("4.0") && tokens.contains("12.0"));
}

// 验证状态伪类自动读取既有事实并按 hover、checked、disabled 优先级叠加。
#[test]
fn generates_pseudo_style_overlay_from_existing_state_facts() {
    // 声明基础类、三个状态差异与拥有现有布尔状态的复选框组件。
    let document = parse_document(
        r##"
        stateful { padding: 8px; color: #colorText; }
        stateful:hover { backgroundColor: #colorFillTertiary; }
        stateful:checked { borderColor: #colorPrimary; borderWidth: 2px; }
        stateful:disabled { opacity: 0.5; }
        <Widget name="Stateful" state="checked: bool = false, disabled: bool = false">
          <Checkbox class="stateful" text="状态" checked={checked} disabled={disabled} />
        </Widget>
        <Stateful />
        "##,
    )
    .expect("状态伪类组件文档应解析成功");
    // 生成完整编译期展开令牌。
    let tokens = generate_document_view(&document)
        .expect("状态伪类应生成成功")
        .to_string();
    // hover 必须复用组件私有状态存储与实际节点子作用域。
    assert!(
        tokens.contains("uix_widget_child_scope") && tokens.contains("__uix_pseudo_hover_state")
    );
    // 自动指针监听器必须同时处理进入与离开且不吞噬组件事件。
    assert!(
        tokens.contains("SystemEvent :: PointerEnter")
            && tokens.contains("SystemEvent :: PointerLeave")
            && tokens.contains("EventResult :: NotHandled")
    );
    // checked 必须读取现有 State<bool>，disabled 必须读取组件布尔值绑定。
    assert!(tokens.contains(". get ()") && tokens.contains("disabled"));
    // 基础与三个差异字段必须都进入样式更新，但变体只通过 map_style 叠加。
    assert!(
        tokens.contains("padding")
            && tokens.contains("background")
            && tokens.contains("border_color")
            && tokens.contains("opacity")
            && tokens.matches("map_style").count() >= 4
    );
    // disabled 包装位于最终输出，形成高于 checked 与 hover 的选择层。
    let hover = tokens
        .find("__uix_pseudo_hover_current")
        .expect("应生成 hover 事实");
    let opacity = tokens.rfind("opacity").expect("应生成 disabled 差异");
    assert!(hover < opacity);
}

// 验证状态伪类拒绝缺失事实并允许文档根复用既有状态存储。
#[test]
fn rejects_pseudo_style_without_required_runtime_facts() {
    // disabled 变体要求使用元素声明同名属性。
    let missing = parse_document(
        "stateful { color: red; } stateful:disabled { opacity: 0.5; } <Widget name=\"Stateful\"><Button class=\"stateful\">状态</Button></Widget><Stateful />",
    )
    .expect("缺失事实不影响结构解析");
    // 生成期必须给出同名事实诊断。
    let error = generate_document_view(&missing).expect_err("缺失 disabled 事实必须失败");
    // 诊断必须保留属性名。
    assert!(error.message.contains("disabled 属性"));
    // 文档根 hover 自动取得隐式生命周期作用域。
    let outside = parse_document(
        "stateful { color: red; } stateful:hover { color: blue; } <Button class=\"stateful\">状态</Button>",
    )
    .expect("组件外 hover 不影响结构解析");
    // 生成期必须复用既有组件状态存储而不要求显式 Widget。
    let tokens = generate_document_view(&outside)
        .expect("文档根 hover 应生成隐式生命周期作用域")
        .to_string();
    // 根作用域、子作用域与生命周期标记必须闭合。
    assert!(
        tokens.contains("__uix_document_scope_")
            && tokens.contains("uix_widget_child_scope")
            && tokens.contains("uix_widget_scope")
    );
}

// 验证 setStyle 目标与组件所有权在生成期关闭。
#[test]
fn rejects_unknown_or_widget_external_dynamic_style() {
    // 未声明目标类必须在样式类解析阶段失败。
    let unknown = parse_document(
        r#"<Widget name="Card"><Text @click="setStyle('missing')">Card</Text></Widget><Card />"#,
    )
    // 语法本身保持合法。
    .expect("未知类应在生成阶段诊断");
    // 读取目标类诊断。
    let unknown_error = generate_document_view(&unknown).expect_err("未知动态类必须失败");
    // 诊断必须包含缺失类名。
    assert!(unknown_error.message.contains("missing") && unknown_error.message.contains("未声明"));
    // 组件外 setStyle 不得回退到调用方同名函数。
    let external =
        parse_document(r#"active { color: red; } <Text @click="setStyle('active')">Card</Text>"#)
            // 语法本身保持合法。
            .expect("组件外调用应在生成阶段诊断");
    // 读取组件所有权诊断。
    let external_error = generate_document_view(&external).expect_err("组件外 setStyle 必须失败");
    // 诊断必须明确要求 UIX Widget。
    assert!(external_error.message.contains("Widget 内"));
}

// 验证 For 内动态样式使用显式 key 或稳定位置形成逐节点身份路径。
#[test]
fn generates_per_node_dynamic_style_identity_inside_for() {
    // 解析组件内部带 key 的动态行样式。
    let document = parse_document(
        r#"
        selectedRow { padding: 8px; }
        <Widget name="Rows" external="items">
          <Column><For {item} in {items} key={item.id}><Text @click="setStyle('selectedRow')">{item.name}</Text></For></Column>
        </Widget>
        <Rows />
        "#,
    )
    // 合法逐行动态样式文档必须解析成功。
    .expect("For 动态样式文档应解析成功");
    // 生成逐实例身份令牌。
    let tokens = generate_document_view(&document)
        // 当前组件自身位于静态位置，内部 For 应可独立持有节点状态。
        .expect("For 内动态样式应生成成功")
        // 转换为稳定文本。
        .to_string();
    // For 必须枚举内部位置并声明实际实例路径。
    assert!(tokens.contains("__uix_for_path_") && tokens.contains("enumerate"));
    // 行 key 必须只求值一次后同时用于 View key 与动态身份。
    assert_eq!(tokens.matches(". id").count(), 1);
    // 动态节点子作用域必须消费循环实例路径。
    assert!(tokens.contains("uix_widget_child_scope"));
}

// 验证 For 内事件为每个迭代实例重新克隆字段捕获和 setState 更新器。
#[test]
fn clones_owned_event_captures_for_each_for_iteration() {
    // 解析循环行事件同时读取私有状态并执行 setState 的组件。
    let document = parse_document(
        // 使用外部集合避免让测试依赖数组状态的具体类型推断。
        r#"
        <Widget name="Rows" state="count: 0" external="items">
          <Column><For {item} in {items}><Button @click="setState(count: count + 1)">{item}</Button></For></Column>
        </Widget>
        <Rows />
        "#,
    )
    // 合法循环事件文档必须解析成功。
    .expect("For 事件文档应解析成功");
    // 生成包含组件准备区与真实循环体的完整令牌。
    let tokens = generate_document_view(&document)
        // 循环事件必须可以完成组件展开。
        .expect("For 事件应生成成功")
        // 转换为稳定文本以检查所有权边界。
        .to_string();
    // 定位真实循环体，排除组件全局准备区中的首次捕获声明。
    let loop_tokens = tokens
        // For 代码生成固定使用内部位置与项绑定元组。
        .split_once("for (__uix_for_ordinal")
        // 生成器契约必须保留可定位的真实 Rust for。
        .expect("输出应包含 For 循环")
        // 只检查循环开始后的令牌。
        .1;
    // 循环体必须重新声明事件字段副本，避免第一行闭包移走全局副本。
    assert!(loop_tokens.contains("let __uix_event_value_") && loop_tokens.contains(". clone ()"));
    // 循环体必须重新声明 setState 更新器，保证每行闭包拥有独立可调用值。
    assert!(loop_tokens.contains("let __uix_set_state_") && loop_tokens.contains("set_state"));
}

// 验证同一静态组件的多个调用各自生成不同的作用域局部变量与生命周期标记。
#[test]
fn generates_distinct_scopes_for_multiple_static_widget_calls() {
    // 解析两个相同组件的静态调用。
    let document = parse_document(
        // 使用同一私有 state 组件的相邻调用覆盖声明身份分配。
        r#"
        <Widget name="Counter" state="count: 0"><Button @click="setState(count: count + 1)">{count}</Button></Widget>
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
    assert_eq!(
        tokens
            .matches(":: uix :: ui :: __private :: uix_widget_scope")
            .count(),
        2
    );
    // 两次静态调用必须使用不同的卫生作用域局部变量，避免状态句柄串用。
    let scope_numbers = tokens
        // 定位全部卫生作用域标识符。
        .match_indices("__uix_widget_scope_")
        // 提取标识符编号片段。
        .map(|(index, _)| {
            // 读取编号起始后的数字前缀。
            tokens[index + "__uix_widget_scope_".len()..]
                // 逐字符读取。
                .chars()
                // 只保留数字。
                .take_while(|character| character.is_ascii_digit())
                // 收集编号文本。
                .collect::<String>()
        })
        // 去重收集编号集合。
        .collect::<std::collections::BTreeSet<_>>();
    // 两次调用必须分配两个不同的作用域编号。
    assert_eq!(scope_numbers.len(), 2);
    // 每个实际根都必须携带对应的 ViewNode 生命周期作用域标记。
    assert_eq!(tokens.matches(". uix_widget_scope").count(), 2);
}

// 验证嵌套私有 state 组件在同一实际根上保留外层与内层两个生命周期标记。
#[test]
fn generates_all_nested_widget_scope_markers_on_one_root() {
    // 解析外层组件直接展开为内层私有 state 组件的单根组合。
    let document = parse_document(
        // 外层和内层均拥有私有 state，以覆盖同根多标记契约。
        r#"
        <Widget name="Counter" state="count: 0"><Button @click="setState(count: count + 1)">{count}</Button></Widget>
        <Widget name="Panel" state="visible: true"><Counter /></Widget>
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
    assert_eq!(
        tokens
            .matches(":: uix :: ui :: __private :: uix_widget_scope")
            .count(),
        2
    );
    // 同一 Button 根必须被连续标记两次，而非由后层覆盖前层标记。
    assert_eq!(tokens.matches(". uix_widget_scope").count(), 2);
    // 两层私有 state 都必须经由运行时状态复用接口取得句柄。
    assert_eq!(tokens.matches("uix_widget_state").count(), 2);
}

// 验证 For 内可递归展开完全无 prop 和无私有 state 的静态组件组合。
#[test]
fn for_nested_allows_pure_static_widgets() {
    // 解析两层纯静态组件与外层 For 调用。
    let document = parse_document(
        // 内外组件均不声明 prop 或私有 state，因此无需逐实例存储。
        r#"
        <Widget name="Leaf"><Text>固定行</Text></Widget>
        <Widget name="Row"><Leaf /></Widget>
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
    assert!(tokens.contains("for (__uix_for_ordinal , item) in") && tokens.contains("enumerate"));
    // 最内层静态组件必须已在 For 体内展开。
    assert!(tokens.contains("固定行"));
}

// 验证 For 内的纯静态外层组件允许内层私有 state 按业务 key 隔离实例。
#[test]
fn for_nested_supports_private_state_widget_instances() {
    // 解析外层无状态组件包裹内层私有 state 组件的 keyed 组合。
    let document = parse_document(
        // Leaf 的私有 state 必须按每个实际行 key 派生独立作用域。
        r#"
        <Widget name="Leaf" state="selected: false"><Text>{selected}</Text></Widget>
        <Widget name="Row"><Leaf /></Widget>
        <Column><For {item} in {items} key={item.id}><Row /></For></Column>
        "#,
    )
    // 语法与组件声明必须解析成功。
    .expect("For 内嵌套私有 state 组件应可解析");
    // 生成完整逐实例作用域令牌。
    let tokens = generate_document_view(&document)
        // keyed For 内的私有 state 组件必须完成生成。
        .expect("For 内嵌套私有 state 组件应生成成功")
        // 规范化令牌便于检查实例边界。
        .to_string();
    // 私有状态组件必须从行路径派生运行时子作用域。
    assert!(tokens.contains("uix_widget_child_scope"));
    // 每个实际迭代必须取得或复用自己的私有状态槽。
    assert!(tokens.contains("uix_widget_state"));
    // 私有状态初始化必须位于真实循环之后而不是根准备区。
    assert!(
        tokens.find("for (__uix_for_ordinal").expect("应生成 For")
            < tokens.find("uix_widget_state").expect("应生成私有状态")
    );
}

// 验证 For 内的纯静态外层组件允许内层 prop 在每次迭代中求值。
#[test]
fn for_nested_supports_prop_widget_instances() {
    // 解析外层无状态组件向内层 prop 组件转发当前行字段的组合。
    let document = parse_document(
        // 每个 Leaf 必须在所属迭代中读取当前 item 的标签。
        r#"
        <Widget name="Leaf" props="label: String"><Text>{label}</Text></Widget>
        <Widget name="Row" props="label: String"><Leaf label={label} /></Widget>
        <Column><For {item} in {items}><Row label={item.label} /></For></Column>
        "#,
    )
    // 语法与 prop 对应关系必须解析成功。
    .expect("For 内嵌套 prop 组件应可解析");
    // 生成每次迭代的 prop 准备语句。
    let tokens = generate_document_view(&document)
        // 当前行 prop 必须完成生成。
        .expect("For 内嵌套 prop 组件应生成成功")
        // 规范化令牌便于检查声明位置。
        .to_string();
    // 外层和内层组件必须各自建立 String prop 绑定。
    assert_eq!(tokens.matches("let __uix_prop_").count(), 2);
    // 两层 prop 初始化都必须位于真实循环之后才能读取 item。
    assert!(
        tokens.find("for (__uix_for_ordinal").expect("应生成 For")
            < tokens.find("let __uix_prop_").expect("应生成 prop")
    );
}

// 验证 State<T> prop 读取与写入同一共享句柄。
#[test]
fn generates_shared_state_prop_tokens() {
    // 解析共享状态组件。
    let document = parse_document(
        // 使用公开 State<number> 契约。
        r#"
        <Widget name="SharedCounter" props="count: State<number>">
          <Column>
            <Text>{count}</Text>
            <Button @click="setState(count: count + 1)">+</Button>
          </Column>
        </Widget>
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
fn rejects_missing_and_unknown_widget_props() {
    // 解析缺少必需 prop 的组件调用。
    let missing = parse_document(
        // 声明一个必需 String prop。
        r#"<Widget name="Greeting" props="name: String"><Text>{name}</Text></Widget><Greeting />"#,
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
        r#"<Widget name="Greeting" props="name: String"><Text>{name}</Text></Widget><Greeting name="UIX" extra="x" />"#,
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
        r#"<Widget name="Editor" props="label: String"><Button @click="setState(label: 'x')">Edit</Button></Widget><Editor label="A" />"#,
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
        r#"<Widget name="Editor"><Button @click="setState(missing: 1)">Edit</Button></Widget><Editor />"#,
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
fn rejects_set_state_outside_widget_event() {
    // 解析在文本插值中执行状态更新的组件。
    let document = parse_document(
        // setState 位于普通插值而不是事件属性。
        r#"<Widget name="Counter" state="count: 0"><Text>{setState(count: count + 1)}</Text></Widget><Counter />"#,
    )
    // 语法形状合法但语义作用域应由生成器判断。
    .expect("越界 setState 应在组件生成阶段诊断");
    // 读取作用域诊断。
    let error = generate_document_view(&document)
        // 普通插值中的状态更新必须失败。
        .expect_err("setState 不能用于普通插值");
    // 诊断应明确事件处理器边界。
    assert!(error.message.contains("只能在 Widget 的事件处理器中使用"));
}

// 验证组件递归调用在代码生成前被拒绝。
#[test]
fn rejects_recursive_widget_composition() {
    // 解析直接自调用组件。
    let document = parse_document(
        // 组件体再次调用自身。
        r#"<Widget name="Loop"><Loop /></Widget><Loop />"#,
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

// 验证句柄位属性在组件体内把 state 字段改写为 State 句柄。
#[test]
fn rewrites_state_fields_to_handles_in_handle_position_attributes() {
    // 解析由私有 state 控制的受控组件与双向绑定组件。
    let document = parse_document(
        // 浮层、输入、日历、列表与上传受控属性都读取句柄本身。
        r#"
        <Widget name="Controlled" state="open: false, checked: true, start: 20, end: 80, calendar_date: Date = Date(2026, 8, 15), active: Option<String> = None, collapse_keys: Vec<String> = [], upload_files: Vec<UploadFile> = []">
          <Column>
            <Modal open={open} title="受控">
              <Text>{open}</Text>
              <Button @click="setState(open: true)">打开</Button>
            </Modal>
            <Switch checked={checked} />
            <RangeSlider value={{ start: start, end: end }} min="0" max="100" />
            <Calendar value={calendar_date} />
            <SelectableList items={[SelectableItem('alpha', 'Alpha')]} active={active} />
            <Collapse panels={[CollapsePanel('面板', '内容').key('panel')]} activeKeys={collapse_keys} />
            <Upload files={upload_files} />
          </Column>
        </Widget>
        <Controlled />
        "#,
    )
    // 受控组件文档必须解析成功。
    .expect("受控组件文档应解析成功");
    // 生成完整组件感知 View 令牌。
    let tokens = generate_document_view(&document)
        // 受控组件必须生成成功。
        .expect("受控组件应生成成功")
        // 转成稳定文本便于检查句柄与读值分流。
        .to_string();
    // Modal open 必须直接绑定私有 state 句柄而非读值。
    assert!(tokens.contains(". open (& (__uix_state_"));
    // 插值必须生成动态标签，并在延迟闭包内读取当前值。
    assert!(tokens.contains("dynamic_label"));
    // 组件入口不得再生成会订阅根节点的预读值标识符。
    assert!(!tokens.contains("__uix_state_value_"));
    // Switch checked 必须绑定私有 state 句柄。
    assert!(tokens.contains(". checked (& (__uix_state_"));
    // RangeSlider 对象字段必须改写为两个独立 state 句柄。
    assert!(tokens.contains(". start (& (__uix_state_"));
    // 区间终点同样读取句柄。
    assert!(tokens.contains(". end (& (__uix_state_"));
    // Calendar value 必须登记为私有日期状态句柄位。
    assert!(is_state_handle_attribute("Calendar", "value"));
    // SelectableList active 必须绑定可空稳定 id 状态句柄。
    assert!(tokens.contains(". active_state (& (__uix_state_"));
    // Collapse activeKeys 必须绑定稳定 key 集合状态句柄。
    assert!(tokens.contains(". active_keys (& (__uix_state_"));
    // Upload files 必须绑定完整上传文件队列状态句柄。
    assert!(tokens.contains("Upload :: new () . files (& (__uix_state_"));
}

// 验证句柄位属性引用只读 prop 时返回明确诊断。
#[test]
fn rejects_read_only_props_in_handle_position_attributes() {
    // 解析把只读 prop 用作受控打开状态的组件。
    let document = parse_document(
        // 普通 String prop 不能提供 Modal 需要的 State<bool> 句柄。
        r#"<Widget name="Bad" props="label: String"><Modal open={label} /></Widget><Bad label="x" />"#,
    )
    // 名称与结构解析合法。
    .expect("只读 prop 形状应在组件展开阶段诊断");
    // 读取句柄位类型诊断。
    let error = generate_document_view(&document)
        // 句柄位引用只读 prop 必须失败。
        .expect_err("句柄位属性不能引用只读 prop");
    // 诊断必须点出句柄位契约。
    assert!(error.message.contains("句柄位属性"));
    // 诊断必须包含违规字段名。
    assert!(error.message.contains("label"));
}

// 验证类型化私有 state 注解生成显式 State 内部类型。
#[test]
fn generates_typed_private_state_tokens() {
    // 解析带 u32 与 usize 类型注解的组件。
    let document = parse_document(
        // 评分与步骤状态分别注解为 u32 与 usize。
        r#"<Widget name="Typed" state="rating: u32 = 7, current: usize = 1"><Text>{rating}</Text><Button @click="setState(rating: rating + 1)">+</Button></Widget><Typed />"#,
    )
    // 类型化状态文档必须解析成功。
    .expect("类型化状态文档应解析成功");
    // 生成完整组件感知 View 令牌。
    let tokens = generate_document_view(&document)
        // 类型化状态必须生成成功。
        .expect("类型化状态应生成成功")
        // 转成稳定文本便于检查显式类型。
        .to_string();
    // u32 状态必须固定显式 State<u32>。
    assert!(tokens.contains("State < u32 >"));
    // usize 状态必须固定显式 State<usize>。
    assert!(tokens.contains("State < usize >"));
    // 整数字面量必须保留作者形状，不追加 f64 小数点。
    assert!(tokens.contains("rating"));
    // setState 更新值中的整数字面量同样恢复作者形状。
    assert!(!tokens.contains("rating + 1.0"));
}

// 验证 State<Vec<String>> prop 作为共享列表句柄进入组件并被 For 消费。
#[test]
fn generates_shared_state_vec_string_prop_tokens() {
    // 解析共享列表组件。
    let document = parse_document(
        // 使用公开 State<Vec<String>> 契约。
        r#"
        <Widget name="QueuePanel" props="queue: State<Vec<String>>">
          <Column>
            <For {item} {i} in {queue}>
              <Text>{i}: {item}</Text>
            </For>
          </Column>
        </Widget>
        <QueuePanel queue={shared_queue} />
        "#,
    )
    // 合法共享列表文档必须解析成功。
    .expect("共享列表文档应解析成功");
    // 生成完整令牌。
    let tokens = generate_document_view(&document)
        // 合法共享列表应生成成功。
        .expect("共享列表组件应生成成功")
        // 转换为文本。
        .to_string();
    // State prop 应固定内部 Vec<String> 类型。
    assert!(tokens.contains("State < :: std :: vec :: Vec < :: std :: string :: String > >"));
    // 共享句柄应保留调用方名称。
    assert!(tokens.contains("shared_queue"));
    // For 数据源应从句柄读值并克隆迭代。
    assert!(tokens.contains(". get ()"));
}

// 验证 State<usize> prop 作为精确索引句柄被组件读取。
#[test]
fn generates_shared_state_usize_prop_tokens() {
    // 解析共享索引组件。
    let document = parse_document(
        // 使用公开 State<usize> 契约。
        r#"
        <Widget name="PositionBadge" props="index: State<usize>">
          <Text>当前 {index}</Text>
        </Widget>
        <PositionBadge index={shared_index} />
        "#,
    )
    // 合法共享索引文档必须解析成功。
    .expect("共享索引文档应解析成功");
    // 生成完整令牌。
    let tokens = generate_document_view(&document)
        // 合法共享索引应生成成功。
        .expect("共享索引组件应生成成功")
        // 转换为文本。
        .to_string();
    // State prop 应固定内部 usize 类型。
    assert!(tokens.contains("State < usize >"));
    // 共享句柄应保留调用方名称。
    assert!(tokens.contains("shared_index"));
}

// 验证集合类型仍不能冒充基础值 prop 或回调参数。
#[test]
fn rejects_collection_value_props_and_callback_arguments() {
    // 解析集合值 prop 组件。
    let value_prop = parse_document(
        // 直接把 Vec<String> 用作值 prop。
        r#"<Widget name="BadList" props="items: Vec<String>"><Text>items</Text></Widget><BadList />"#,
    )
    // 解析阶段即应拒绝未登记值 prop。
    .expect_err("集合值 prop 必须解析失败");
    // 诊断应指向类型白名单。
    assert!(value_prop.message.contains("不支持 prop 类型"));
    // 解析集合回调参数组件。
    let callback = parse_document(
        // 把 Vec<String> 用作回调参数类型。
        r#"<Widget name="BadCb" props="onItems: (Vec<String>)"><Text>x</Text></Widget><BadCb onItems={noop} />"#,
    )
    // 解析阶段即应拒绝集合回调参数。
    .expect_err("集合回调参数必须解析失败");
    // 诊断应指向 props 类型。
    assert!(callback.message.contains("不支持 prop 类型"));
}

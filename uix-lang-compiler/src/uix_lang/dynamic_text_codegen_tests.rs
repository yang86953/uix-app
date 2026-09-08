// 引入核心与组件感知文档生成入口。
use super::{generate_document_view, generate_view, parse_document};

// 生成核心根 View 的稳定令牌文本。
fn generate_core(source: &str) -> String {
    // 解析单根核心文档。
    let document = parse_document(source).expect("动态文本测试文档应解析");
    // 生成公开 View 并规范化令牌空白。
    generate_view(&document.root)
        // 核心 Text 应成功生成。
        .expect("动态 Text 应生成")
        // 转为稳定文本供结构断言。
        .to_string()
}

// 验证静态 Text 保持 Label，插值 Text 使用拥有型 DynamicLabel。
#[test]
fn lowers_only_interpolated_text_to_dynamic_label() {
    // 静态文本无需运行时闭包。
    let static_text = generate_core(r#"<Text fontSize="heading2">静态标题</Text>"#);
    // 静态路径保持既有公开 label 构造器。
    assert!(static_text.contains("Label :: new (\"静态标题\") . word_wrap (true)"));
    // 静态路径不得制造 DynamicLabel。
    assert!(!static_text.contains("dynamic_label"));
    // 动态文本读取调用方 State 并带字号样式。
    let dynamic = generate_core(r#"<Text fontSize="heading2">tick: {tick.get()}s</Text>"#);
    // 动态路径必须使用公开 dynamic_label 构造器。
    assert!(dynamic.contains("dynamic_label (move ||"));
    // 调用方值必须先克隆，避免移走兄弟 View 仍需使用的句柄。
    assert!(dynamic.contains("Clone :: clone") && dynamic.contains("dynamic_text_capture"));
    // State 当前值读取必须位于动态闭包生成物中。
    assert!(dynamic.contains(". get ()"));
    // Text 字号仍由公共样式链应用。
    assert!(dynamic.contains("font_size"));
}

// 验证组件状态只在结构、事件或 DynamicLabel 实际使用位置读取。
#[test]
fn avoids_unconditional_widget_state_reads() {
    // 构造同时覆盖动态 tick、结构 page 与事件 count 的组件。
    let document = parse_document(
        r#"
        <Widget name="Demo" props="tick: State<number>, page: State<number>, count: State<number>">
          <Column>
            <Text>tick: {tick}</Text>
            <If {page == 0}><Text>首页</Text></If>
            <Button @click="setState(count: count + 1)">增加</Button>
          </Column>
        </Widget>
        <Demo tick={tick} page={page} count={count} />
        "#,
    )
    // 组件声明与调用必须解析成功。
    .expect("组件动态文本文档应解析");
    // 生成完整组件感知 Rust View。
    let tokens = generate_document_view(&document)
        // 结构、事件与动态文本应同时生成。
        .expect("组件状态分流应生成")
        // 转为稳定令牌文本。
        .to_string();
    // 组件入口不得再创建无条件 state_value 快照。
    assert!(!tokens.contains("__uix_state_value"));
    // tick 插值必须物化为延迟 DynamicLabel。
    assert!(tokens.contains("dynamic_label (move ||"));
    // 结构 If 仍保留构建期 State::get 以触发 reconcile。
    assert!(tokens.contains("if") && tokens.contains(". get ()"));
    // 事件注册只克隆 State 句柄，不在注册时读取值。
    assert!(tokens.contains("__uix_event_value") && tokens.contains(". clone ()"));
}

// 验证 State 字段显式 .get() 成员调用与裸字段读值同义归一，嵌套条件内不叠加双重解引用。
#[test]
fn normalizes_explicit_state_get_inside_nested_conditionals() {
    // 构造单层条件与嵌套条件两处同构的显式 get 动态文本，实参与调用方形状对齐。
    let document = parse_document(
        // 单层 If 与 If 内嵌 Column 再嵌 If 使用完全相同的字段表达式。
        r#"
        <Widget name="Demo" props="label: State<String>">
          <If {true}>
            <Text>单层: {label.get()} 字</Text>
          </If>
          <If {true}>
            <Column>
              <If {true}>
                <Text fontSize="small">嵌套: {label.get()} 字</Text>
              </If>
            </Column>
          </If>
        </Widget>
        <Demo label={label.clone()} />
        "#,
    )
    // 组件声明与调用必须解析成功。
    .expect("显式 get 组件文档应解析");
    // 生成完整组件感知 Rust View 令牌。
    let tokens = generate_document_view(&document)
        // 两处动态文本都应生成成功。
        .expect("显式 get 动态文本应生成")
        // 转成稳定文本供结构断言。
        .to_string();
    // State 读值不得叠加双重 .get()：显式成员调用必须与裸字段读值同义归一。
    // 双重形状含跨闭括号与直连两种，均视为叠加。
    assert!(
        !tokens.contains("get () . get ()") && !tokens.contains("get ()) . get ()"),
        "State 字段显式 get 不得叠加句柄 get：{tokens}"
    );
    // 两处动态文本仍必须延迟求值。
    assert!(tokens.contains("dynamic_label (move ||"));
}

#[test]
fn text_ellipsis_preserves_dynamic_captures_and_validates_boolean() {
    for source in [
        r#"<Text ellipsis>{name.get()}</Text>"#,
        r#"<Text ellipsis={false}>长标签</Text>"#,
    ] {
        let generated = generate_core(source);
        assert!(generated.contains("elided_label"));
        assert!(generated.contains("dynamic_label"));
    }
    let doc = parse_document(r#"<Text ellipsis="yes">invalid</Text>"#).unwrap();
    assert!(generate_view(&doc.root).is_err());
}

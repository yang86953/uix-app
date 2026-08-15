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
    assert!(static_text.contains("label (\"静态标题\")"));
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
fn avoids_unconditional_component_state_reads() {
    // 构造同时覆盖动态 tick、结构 page 与事件 count 的组件。
    let document = parse_document(
        r#"
        <Component name="Demo" props="tick: State<number>, page: State<number>, count: State<number>">
          <Column>
            <Text>tick: {tick}</Text>
            <If {page == 0}><Text>首页</Text></If>
            <Button @click="setState(count: count + 1)">增加</Button>
          </Column>
        </Component>
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

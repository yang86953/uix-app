// 引入文档解析、核心 View 与完整文档测试生成入口。
use super::{generate_test_document_view, generate_view, parse_document};

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
        r#":: uix :: prelude :: View :: build (((:: uix :: prelude :: row ({ let mut __uix_children = :: std :: vec :: Vec :: < :: uix :: prelude :: ViewNode > :: new () ; __uix_children . push (:: uix :: prelude :: View :: build (({ let __uix_dynamic_text_capture_0 = :: std :: clone :: Clone :: clone (& (count)) ; :: uix :: prelude :: dynamic_label (move || { let mut __uix_text = :: std :: string :: String :: new () ; __uix_text . push_str ("Count: ") ; __uix_text . push_str (& :: std :: string :: ToString :: to_string (& ((__uix_dynamic_text_capture_0) . clone ()))) ; __uix_text }) }) . font_size (:: uix :: prelude :: TypographyToken :: Heading2))) ; __uix_children . push (:: uix :: prelude :: View :: build ((((:: uix :: prelude :: button ("Save")) . primary ()) . disabled (busy)) . on_click_fn (move || { let _ = { (onConfirm) () } ; }))) ; __uix_children . push (:: uix :: prelude :: View :: build (:: uix :: prelude :: ViewNode :: leaf ((:: uix :: prelude :: Icon :: new ("star")) . size (16.0)))) ; __uix_children })) . gap (8.0)) . align (:: uix :: prelude :: AlignItems :: Center))"#
    );
}

// 验证 Divider 生成只调用现有公开组件与 View API。
#[test]
fn generates_divider_with_documented_properties() {
    // 构造覆盖文字、动态虚线、垂直方向和公共样式的 Divider。
    let source =
        r#"<Divider text={title} dashed={show_dashes} direction="vertical" margin="8px" />"#;
    // 生成稳定令牌文本。
    let snapshot = generate(source).expect("文档化 Divider 属性应生成 Rust View");
    // 必须从公开 Divider 默认构造器开始。
    assert!(snapshot.contains("Divider :: new"));
    // 公开 Divider 必须通过 View 契约进入 UIX 声明壳。
    assert!(snapshot.contains("View :: build"));
    // 文字必须通过现有拥有所有权的构建器进入组件。
    assert!(snapshot.contains("with_text") && snapshot.contains("title"));
    // 动态虚线必须保持 true/false 两条同类型路径。
    assert!(snapshot.contains("if show_dashes") && snapshot.contains("dashed"));
    // 垂直方向必须调用现有构建器。
    assert!(snapshot.contains("vertical"));
    // 公共样式仍由统一 View 映射处理。
    assert!(snapshot.contains("margin"));
    // 生成物不得包含运行时标签解析或第二个 Divider 实现。
    assert!(!snapshot.contains("parse_divider"));
    // 字面属性应遵守同一公开构建路径。
    let literal = generate(
        // 同时覆盖文字字面量、false 虚线与默认水平方向。
        r#"<Divider text="Section" dashed="false" direction="horizontal" />"#,
    )
    // 字面属性必须生成成功。
    .expect("Divider 字面属性应生成 Rust View");
    // 文字字面量必须进入现有构建器。
    assert!(literal.contains("with_text") && literal.contains("Section"));
    // false 必须保留显式分支而不强制启用虚线。
    assert!(literal.contains("if false") && !literal.contains("vertical"));
}

// 验证 Divider 的默认值与非法边界保持确定诊断。
#[test]
fn validates_divider_shape_and_direction() {
    // 默认自闭合 Divider 应直接复用组件默认契约。
    let default = generate(r#"<Divider />"#).expect("默认 Divider 应可生成");
    // 默认路径不应伪造文字、虚线或垂直状态。
    assert!(
        // 同时检查三个可选构建器均未出现。
        !default.contains("with_text")
            // 默认不启用虚线。
            && !default.contains("dashed")
            // 默认保持水平方向。
            && !default.contains("vertical")
    );
    // 非法方向必须在代码生成期失败。
    let direction_error = generate(r#"<Divider direction="diagonal" />"#)
        // 提取预期诊断。
        .expect_err("未知 Divider 方向必须失败");
    // 诊断必须包含具体方向和合法集合提示。
    assert!(
        // 消息指出不支持的方向值。
        direction_error.message.contains("diagonal")
            // 修复建议给出两个合法值。
            && direction_error.suggestion.contains("horizontal")
            // 修复建议同时包含垂直值。
            && direction_error.suggestion.contains("vertical")
    );
    // 可见子内容不能被叶子组件静默丢弃。
    let child_error = generate(r#"<Divider>label</Divider>"#)
        // 提取预期结构诊断。
        .expect_err("Divider 可见子节点必须失败");
    // 诊断必须说明叶子形状并指向 text 属性。
    assert!(
        // 消息说明不接受子节点。
        child_error.message.contains("不接受子节点")
            // 修复建议指向文档化文字属性。
            && child_error.suggestion.contains("text")
    );
    // 动态 direction 无法在编译期选择构建器，必须明确拒绝。
    let dynamic_direction = generate(r#"<Divider direction={axis} />"#)
        // 提取预期字面量诊断。
        .expect_err("动态 Divider direction 必须失败");
    // 诊断必须指出方向需要字符串字面量。
    assert!(dynamic_direction.message.contains("必须使用字符串字面量"));
    // Divider 未登记属性必须继续走统一拒绝路径。
    let unknown_attribute = generate(r#"<Divider mystery="value" />"#)
        // 提取预期属性映射诊断。
        .expect_err("Divider 未登记属性必须失败");
    // 诊断必须包含具体未知属性名。
    assert!(unknown_attribute.message.contains("mystery"));
}

// 验证 Space 生成只调用现有公开组件与 View API。
#[test]
fn generates_space_with_documented_properties_and_children() {
    // 构造覆盖动态间距、动态换行、垂直方向、公共样式与有序子节点的 Space。
    let source = r#"<Space direction="vertical" gap={space_gap} wrap={should_wrap} margin="4px"><Text>First</Text><Button>Second</Button></Space>"#;
    // 生成稳定令牌文本。
    let snapshot = generate(source).expect("文档化 Space 属性与子节点应生成 Rust View");
    // 必须从公开 Space 默认构造器开始。
    assert!(snapshot.contains("Space :: new"));
    // 动态 gap 必须进入现有 SpaceSize 自定义值。
    assert!(snapshot.contains("SpaceSize :: Custom (space_gap)"));
    // 动态 wrap 必须直接传入现有构建器。
    assert!(snapshot.contains("wrap (should_wrap)"));
    // 垂直方向必须调用现有构建器。
    assert!(snapshot.contains("vertical"));
    // Space 与子树必须通过公开 ViewNode 组合。
    assert!(snapshot.contains("ViewNode :: new"));
    // 两个子节点必须保持源码顺序生成。
    let first = snapshot.find("First").expect("首个 Space 子节点应存在");
    // 定位第二个子节点文字。
    let second = snapshot.find("Second").expect("第二个 Space 子节点应存在");
    // 首个子节点必须先于第二个子节点。
    assert!(first < second);
    // 公共样式仍由统一 View 映射处理。
    assert!(snapshot.contains("margin"));
    // 生成物不得包含运行时标签解析或第二个 Space 实现。
    assert!(!snapshot.contains("parse_space"));
    // 字面属性应遵守同一公开构建路径。
    let literal = generate(
        // 同时覆盖数值字面量、false 换行与默认水平方向。
        r#"<Space direction="horizontal" gap="12px" wrap="false"><Text>Only</Text></Space>"#,
    )
    // 字面属性必须生成成功。
    .expect("Space 字面属性应生成 Rust View");
    // 十二像素必须进入公开自定义间距枚举。
    assert!(literal.contains("SpaceSize :: Custom (12.0)"));
    // false 必须原样传给换行构建器且不切换方向。
    assert!(literal.contains("wrap (false)") && !literal.contains("vertical"));
}

// 验证 Space 的默认值与非法边界保持确定诊断。
#[test]
fn validates_space_defaults_and_attribute_contract() {
    // 默认 Space 应保留水平、八像素与不换行的组件默认契约。
    let default = generate(r#"<Space><Text>Default</Text></Space>"#)
        // 默认容器与子节点必须生成成功。
        .expect("默认 Space 应可生成");
    // 默认路径不应伪造方向、间距或换行构建器。
    assert!(
        // 默认保持水平方向。
        !default.contains("vertical")
            // 默认保留 SpaceSize::Small 八像素间距。
            && !default.contains("SpaceSize :: Custom")
            // 默认保持不换行。
            && !default.contains(". wrap (")
    );
    // 非法方向必须在代码生成期失败。
    let direction_error = generate(r#"<Space direction="diagonal" />"#)
        // 提取预期诊断。
        .expect_err("未知 Space 方向必须失败");
    // 诊断必须包含具体方向和合法集合提示。
    assert!(
        // 消息指出不支持的方向值。
        direction_error.message.contains("diagonal")
            // 修复建议给出水平值。
            && direction_error.suggestion.contains("horizontal")
            // 修复建议同时包含垂直值。
            && direction_error.suggestion.contains("vertical")
    );
    // 动态 direction 无法在编译期选择构建器，必须明确拒绝。
    let dynamic_direction = generate(r#"<Space direction={axis} />"#)
        // 提取预期字面量诊断。
        .expect_err("动态 Space direction 必须失败");
    // 诊断必须指出方向需要字符串字面量。
    assert!(dynamic_direction.message.contains("必须使用字符串字面量"));
    // 非数值 gap 必须由共享数值映射拒绝。
    let gap_error = generate(r#"<Space gap="wide" />"#)
        // 提取预期数值诊断。
        .expect_err("非数值 Space gap 必须失败");
    // 诊断必须指出无法映射为数值。
    assert!(gap_error.message.contains("无法映射为 f32"));
    // 非布尔 wrap 必须由共享布尔映射拒绝。
    let wrap_error = generate(r#"<Space wrap="yes" />"#)
        // 提取预期布尔诊断。
        .expect_err("非布尔 Space wrap 必须失败");
    // 诊断必须指出 wrap 需要布尔值。
    assert!(wrap_error.message.contains("wrap") && wrap_error.message.contains("布尔"));
    // Space 未登记属性必须继续走统一拒绝路径。
    let unknown_attribute = generate(r#"<Space mystery="value" />"#)
        // 提取预期属性映射诊断。
        .expect_err("Space 未登记属性必须失败");
    // 诊断必须包含具体未知属性名。
    assert!(unknown_attribute.message.contains("mystery"));
}

// 验证 Typography 生成只调用公开组件、主题值与 View API。
#[test]
fn generates_typography_with_documented_properties_and_text() {
    // 构造覆盖标题、危险色、动态标记、下划线、复制与插值的排版标签。
    let source = r#"<Typography level={1} type="danger" mark={marked} underline="true" copyable={copyable} margin="4px">Hello {name}</Typography>"#;
    // 生成稳定令牌文本。
    let snapshot = generate(source).expect("文档化 Typography 属性与文本应生成 Rust View");
    // 必须从公开 Typography 文本构造器开始。
    assert!(snapshot.contains("Typography :: text"));
    // 标题表达式必须进入现有 level 构建器。
    assert!(snapshot.contains("level (1)"));
    // danger 必须使用主题感知的错误色板角色。
    assert!(
        // 检查语义颜色构建器。
        snapshot.contains("semantic_color")
            // 检查错误色板角色。
            && snapshot.contains("PaletteColor :: Error")
    );
    // 动态 mark 必须形成保持同类型的条件构建。
    assert!(snapshot.contains("if marked") && snapshot.contains("mark ()"));
    // true 下划线必须调用现有启用式构建器。
    assert!(snapshot.contains("if true") && snapshot.contains("underline ()"));
    // 动态复制状态必须直接传给公开构建器。
    assert!(snapshot.contains("copyable (copyable)"));
    // 动态文本必须保留源码顺序并转换插值。
    assert!(snapshot.contains("Hello") && snapshot.contains("name"));
    // Typography 必须作为公开叶视图组合。
    assert!(snapshot.contains("ViewNode :: leaf"));
    // 公共样式仍由统一 View 映射处理。
    assert!(snapshot.contains("margin"));
    // 生成物不得包含运行时标签解析器。
    assert!(!snapshot.contains("parse_typography"));
}

// 验证 Typography 默认值与非法边界保持确定诊断。
#[test]
fn validates_typography_defaults_and_attribute_contract() {
    // 默认 Typography 应保留正文、主题正文色与关闭装饰的组件默认契约。
    let default = generate(r#"<Typography>Body</Typography>"#)
        // 默认排版文本必须生成成功。
        .expect("默认 Typography 应可生成");
    // 默认路径不应伪造标题、语义色或装饰构建器。
    assert!(
        // 默认不设置标题层级。
        !default.contains("level")
            // 默认沿用主题正文色。
            && !default.contains("semantic_color")
            // 默认不启用标记。
            && !default.contains("mark ()")
            // 默认不启用下划线。
            && !default.contains("underline ()")
            // 默认不配置复制按钮。
            && !default.contains("copyable")
    );
    // 越界标题层级必须在代码生成期失败。
    let range_error = generate(r#"<Typography level="0">Zero</Typography>"#)
        // 提取预期范围诊断。
        .expect_err("越界 Typography level 必须失败");
    // 诊断必须包含具体范围。
    assert!(range_error.message.contains("1~5"));
    // 非整数层级必须在代码生成期失败。
    let integer_error = generate(r#"<Typography level="title">Bad</Typography>"#)
        // 提取预期整数诊断。
        .expect_err("非整数 Typography level 必须失败");
    // 诊断必须指出整数要求。
    assert!(integer_error.message.contains("整数"));
    // 动态 type 无法在编译期选择语义枚举，必须明确拒绝。
    let dynamic_type = generate(r#"<Typography type={kind}>Dynamic</Typography>"#)
        // 提取预期字面量诊断。
        .expect_err("动态 Typography type 必须失败");
    // 诊断必须指出 type 需要字符串字面量。
    assert!(dynamic_type.message.contains("必须使用字符串字面量"));
    // 未登记语义类型必须返回枚举诊断。
    let type_error = generate(r#"<Typography type="info">Info</Typography>"#)
        // 提取预期枚举诊断。
        .expect_err("未登记 Typography type 必须失败");
    // 修复建议必须列出合法语义色。
    assert!(
        type_error.suggestion.contains("secondary") && type_error.suggestion.contains("danger")
    );
    // 元素子节点不能被文本组件静默丢弃。
    let child_error = generate(r#"<Typography><Icon name="star" /></Typography>"#)
        // 提取预期内容形状诊断。
        .expect_err("Typography 元素子节点必须失败");
    // 诊断必须指出 Typography 的文本边界。
    assert!(child_error.message.contains("Typography"));
    // Typography 未登记属性必须继续走统一拒绝路径。
    let unknown_attribute = generate(r#"<Typography mystery="value">Text</Typography>"#)
        // 提取预期属性映射诊断。
        .expect_err("Typography 未登记属性必须失败");
    // 诊断必须包含具体未知属性名。
    assert!(unknown_attribute.message.contains("mystery"));
}

// 验证 ThemeToggle 生成只调用现有公开组件与 View API。
#[test]
fn generates_theme_toggle_from_public_widget() {
    // 构造带公共样式的文档化 ThemeToggle。
    let snapshot = generate(r#"<ThemeToggle margin="4px" />"#)
        // ThemeToggle 与公共样式必须生成成功。
        .expect("文档化 ThemeToggle 应生成 Rust View");
    // 必须从现有公开 ThemeToggle 默认构造器开始。
    assert!(snapshot.contains("ThemeToggle :: new"));
    // 声明式快捷控件必须自动进入与 setTheme 相同的 App 级主题通道。
    assert!(snapshot.contains("on_change_fn") && snapshot.contains("uix_set_theme"));
    // 组件必须通过公开 View 契约进入 UIX 声明壳。
    assert!(snapshot.contains("View :: build"));
    // 公共样式仍由统一 View 映射处理。
    assert!(snapshot.contains("margin"));
    // 生成物不得包含运行时标签解析器或第二个组件实现。
    assert!(!snapshot.contains("parse_theme_toggle"));
}

// 验证 ThemeToggle 的叶子形状与属性边界保持确定诊断。
#[test]
fn validates_theme_toggle_shape_and_attribute_contract() {
    // 默认自闭合 ThemeToggle 应直接复用组件默认契约。
    let default = generate(r#"<ThemeToggle />"#).expect("默认 ThemeToggle 应可生成");
    // 默认路径不预置暗色状态，但必须进入唯一的 App 级主题请求通道。
    assert!(!default.contains("dark") && default.contains("uix_set_theme"));
    // 可见文本不能被叶子组件静默丢弃。
    let text_error = generate(r#"<ThemeToggle>dark</ThemeToggle>"#)
        // 提取预期结构诊断。
        .expect_err("ThemeToggle 文本子节点必须失败");
    // 诊断必须说明叶子形状。
    assert!(text_error.message.contains("不接受子节点"));
    // 元素子节点同样不能被静默丢弃。
    let element_error = generate(r#"<ThemeToggle><Icon name="moon" /></ThemeToggle>"#)
        // 提取预期结构诊断。
        .expect_err("ThemeToggle 元素子节点必须失败");
    // 修复建议必须给出文档化自闭合写法。
    assert!(element_error.suggestion.contains("<ThemeToggle />"));
    // ThemeToggle 未登记属性必须继续走统一拒绝路径。
    let unknown_attribute = generate(r#"<ThemeToggle dark="true" />"#)
        // 提取预期属性映射诊断。
        .expect_err("ThemeToggle 未登记属性必须失败");
    // 诊断必须包含具体未知属性名。
    assert!(unknown_attribute.message.contains("dark"));
}

// 验证 ButtonGroup 保留子按钮能力并生成稳定连体位置。
#[test]
fn generates_button_group_with_positioned_button_contracts() {
    // 构造覆盖类型、动态禁用、事件、公共样式与外层样式的三按钮组。
    let snapshot = generate(
        r#"<ButtonGroup margin="4px"><Button type="primary" disabled={busy} @click="save()">Left</Button><Button style="padding: 8px;">Middle</Button><Button type="ghost">Right</Button></ButtonGroup>"#,
    )
    // 文档化 ButtonGroup 与完整 Button 能力必须生成成功。
    .expect("文档化 ButtonGroup 应生成 Rust View");
    // 外层必须使用零间距行容器。
    assert!(snapshot.contains("row") && snapshot.contains("gap (0.0)"));
    // 三个位置必须按源码顺序出现。
    let left = snapshot
        .find("ButtonGroupPosition :: Left")
        .expect("首按钮应标记 Left");
    // 定位中间按钮位置。
    let middle = snapshot
        .find("ButtonGroupPosition :: Middle")
        // 中间位置必须存在。
        .expect("中间按钮应标记 Middle");
    // 定位末按钮位置。
    let right = snapshot
        .find("ButtonGroupPosition :: Right")
        // 右侧位置必须存在。
        .expect("末按钮应标记 Right");
    // 连体位置必须保持源码顺序。
    assert!(left < middle && middle < right);
    // Button 专有类型与动态禁用必须保留。
    assert!(snapshot.contains("primary") && snapshot.contains("disabled (busy)"));
    // 子按钮点击事件与内联样式必须保留。
    assert!(snapshot.contains("on_click_fn") && snapshot.contains("padding"));
    // 外层公共样式必须继续应用。
    assert!(snapshot.contains("margin"));
}

// 验证 ButtonGroup 单项、空组与非法子树边界。
#[test]
fn validates_button_group_shape_and_attribute_contract() {
    // 单按钮组必须使用完整圆角位置。
    let single = generate(r#"<ButtonGroup><Button>Only</Button></ButtonGroup>"#)
        // 单按钮组必须生成成功。
        .expect("单按钮 ButtonGroup 应可生成");
    // 单项只能出现 Single 位置。
    assert!(single.contains("ButtonGroupPosition :: Single"));
    // 空组沿用现有 ButtonGroup 的合法空行契约。
    let empty = generate(r#"<ButtonGroup />"#).expect("空 ButtonGroup 应可生成");
    // 空组不能伪造任何按钮位置。
    assert!(!empty.contains("ButtonGroupPosition"));
    // 裸文本不能被静默转换为按钮。
    let text_error = generate(r#"<ButtonGroup>Loose</ButtonGroup>"#)
        // 提取预期文本形状诊断。
        .expect_err("ButtonGroup 裸文本必须失败");
    // 修复建议必须指向直接 Button 子项。
    assert!(text_error.suggestion.contains("<Button>"));
    // 非 Button 元素不能进入连体位置计算。
    let element_error = generate(r#"<ButtonGroup><Icon name="x" /></ButtonGroup>"#)
        // 提取预期元素形状诊断。
        .expect_err("ButtonGroup 非 Button 子项必须失败");
    // 诊断必须包含非法元素名。
    assert!(element_error.message.contains("Icon"));
    // 动态控制子树无法在编译期确定位置，必须明确拒绝。
    let control_error =
        generate(r#"<ButtonGroup><If {visible}><Button>Conditional</Button></If></ButtonGroup>"#)
            // 提取预期动态形状诊断。
            .expect_err("ButtonGroup 动态 If 子树必须失败");
    // 修复建议必须指向动态 Row 组合。
    assert!(control_error.suggestion.contains("Row"));
    // 未登记外层属性必须走统一拒绝路径。
    let attribute_error = generate(r#"<ButtonGroup compact="true" />"#)
        // 提取预期属性诊断。
        .expect_err("ButtonGroup 未登记属性必须失败");
    // 诊断必须包含具体未知属性名。
    assert!(attribute_error.message.contains("compact"));
}

// 验证 WindowControl 生成只调用运行时公开组合函数。
#[test]
fn generates_window_controls_with_documented_flags() {
    // 构造覆盖字面量、受限表达式与公共样式的窗口控制标签。
    let snapshot = generate(
        r#"<WindowControl showMinimize="false" showMaximize={show_maximize} showClose="true" margin="4px" />"#,
    )
    // 文档化 WindowControl 属性必须生成成功。
    .expect("文档化 WindowControl 应生成 Rust View");
    // 必须调用 window_chrome 公开组合函数。
    assert!(snapshot.contains("window_controls"));
    // 三项显示值必须按最小化、最大化/还原、关闭顺序传递。
    assert!(
        // 最小化字面量保持 false。
        snapshot.contains("window_controls (false , show_maximize , true)")
    );
    // 公共样式仍由统一 View 映射处理。
    assert!(snapshot.contains("margin"));
    // 过程宏不得重复持有平台窗口动作或外观策略。
    assert!(!snapshot.contains("WindowControl :: Minimize") && !snapshot.contains("Icon :: new"));
}

// 验证 WindowControl 默认值、叶子形状与属性边界。
#[test]
fn validates_window_controls_defaults_shape_and_attributes() {
    // 默认自闭合标签必须显示全部三个标准动作。
    let default = generate(r#"<WindowControl />"#).expect("默认 WindowControl 应可生成");
    // 默认值必须按文档传入三个 true。
    assert!(default.contains("window_controls (true , true , true)"));
    // 可见子节点不能被组合叶标签静默丢弃。
    let child_error = generate(r#"<WindowControl><Icon name="x" /></WindowControl>"#)
        // 提取预期结构诊断。
        .expect_err("WindowControl 可见子节点必须失败");
    // 诊断必须说明叶子形状。
    assert!(child_error.message.contains("不接受子节点"));
    // 非布尔显示属性必须由共享布尔映射拒绝。
    let boolean_error = generate(r#"<WindowControl showClose="yes" />"#)
        // 提取预期布尔诊断。
        .expect_err("WindowControl 非布尔显示属性必须失败");
    // 诊断必须指出具体属性与布尔要求。
    assert!(boolean_error.message.contains("showClose") && boolean_error.message.contains("布尔"));
    // 未登记属性必须继续走统一拒绝路径。
    let unknown_attribute = generate(r#"<WindowControl action="close" />"#)
        // 提取预期属性映射诊断。
        .expect_err("WindowControl 未登记属性必须失败");
    // 诊断必须包含具体未知属性名。
    assert!(unknown_attribute.message.contains("action"));
}

// 验证 FloatButton 完整属性映射到现有公开运行时组件。
#[test]
fn generates_float_button_with_documented_contract() {
    // 构造覆盖字符串表达式、四角、badge、事件与公共样式的标签。
    let snapshot = generate(
        r#"<FloatButton icon="message" description={label} tooltip="反馈" position="leftTop" badge={{ count: unread, dot: true }} @click="openFeedback" margin="4px" />"#,
    )
    // 完整文档契约必须生成成功。
    .expect("FloatButton 应生成公开 Rust View");
    // 必须调用现有 FloatButton 构建器。
    assert!(snapshot.contains("FloatButton :: new"));
    // 四角关键字必须映射公开 Placement。
    assert!(snapshot.contains("Placement :: TopLeft"));
    // 说明、提示和两个 badge 字段必须保留。
    assert!(
        snapshot.contains("description")
            && snapshot.contains("tooltip")
            && snapshot.contains("badge (unread)")
            && snapshot.contains("badge_dot (true)")
    );
    // 点击与公共样式继续走统一 View 映射。
    assert!(snapshot.contains("on_click_fn") && snapshot.contains("margin"));
}

// 验证 FloatButton 缺省值、叶子形状和专有属性诊断。
#[test]
fn validates_float_button_defaults_shape_and_attributes() {
    // 缺省位置必须显式映射右下角。
    let default = generate(r#"<FloatButton icon="message" />"#).expect("最小 FloatButton 应可生成");
    // 检查文档默认 Placement。
    assert!(default.contains("Placement :: BottomRight"));
    // 缺失 icon 必须返回必填属性诊断。
    let missing = generate(r#"<FloatButton />"#).expect_err("缺失 icon 必须失败");
    // 诊断必须点名 icon。
    assert!(missing.message.contains("icon"));
    // 可见子节点必须被拒绝。
    let child = generate(r#"<FloatButton icon="x">Text</FloatButton>"#)
        // 提取叶子形状诊断。
        .expect_err("FloatButton 子节点必须失败");
    // 诊断必须说明叶子边界。
    assert!(child.message.contains("不接受子节点"));
    // 未知 position 必须列出合法集合。
    let position = generate(r#"<FloatButton icon="x" position="center" />"#)
        // 提取位置诊断。
        .expect_err("未知 position 必须失败");
    // 诊断必须包含未知值和合法值。
    assert!(position.message.contains("center") && position.suggestion.contains("rightBottom"));
    // badge 必须使用对象字面量。
    let badge = generate(r#"<FloatButton icon="x" badge={count} />"#)
        // 提取 badge 结构诊断。
        .expect_err("非对象 badge 必须失败");
    // 诊断必须指向对象字面量。
    assert!(badge.message.contains("对象字面量"));
    // badge 未知字段必须失败。
    let unknown_field = generate(r#"<FloatButton icon="x" badge={{ total: 1 }} />"#)
        // 提取未知字段诊断。
        .expect_err("未知 badge 字段必须失败");
    // 诊断必须点名字段。
    assert!(unknown_field.message.contains("total"));
    // 负数 count 必须失败。
    let negative = generate(r#"<FloatButton icon="x" badge={{ count: -1 }} />"#)
        // 提取 count 边界诊断。
        .expect_err("负数 count 必须失败");
    // 诊断必须说明非负约束。
    assert!(negative.message.contains("不能为负数"));
    // 非布尔 dot 必须失败。
    let dot = generate(r#"<FloatButton icon="x" badge={{ dot: 1 }} />"#)
        // 提取 dot 类型诊断。
        .expect_err("非布尔 dot 必须失败");
    // 诊断必须说明布尔类型。
    assert!(dot.message.contains("布尔"));
    // 未登记属性继续走统一拒绝路径。
    let unknown = generate(r#"<FloatButton icon="x" mystery="value" />"#)
        // 提取未知属性诊断。
        .expect_err("未知 FloatButton 属性必须失败");
    // 诊断必须点名属性。
    assert!(unknown.message.contains("mystery"));
}

// 验证 If 与 For 生成真实 Rust 控制流、实例路径、索引和稳定 key。
#[test]
fn generates_if_for_and_key_snapshot() {
    // 构造条件与带索引、key 的循环文档。
    let source = r#"<Column><If {visible}><Text>{title}</Text></If><For {item} {index} in {items} key={item.id}><Container direction="row"><Text>{index}</Text><Text>{item.name}</Text></Container></For></Column>"#;
    // 生成确定性令牌快照。
    let snapshot = generate_test_document_view(source).expect("If 与 For 应生成 Rust 控制流");
    // If 必须保留为真实条件分支。
    assert!(snapshot.contains("if visible"));
    // 带索引 For 必须枚举输入集合。
    assert!(snapshot.contains("enumerate"));
    // 作者索引绑定必须复用内部枚举序号。
    assert!(snapshot.contains("let index = __uix_for_ordinal"));
    // 循环体必须声明当前行的实际实例路径。
    assert!(snapshot.contains("let __uix_for_path_"));
    // 显式 key 必须继续附加到唯一行根。
    assert!(snapshot.contains(". key") && snapshot.contains("(item) . id"));
    // 显式 key 表达式必须只求值一次。
    assert_eq!(snapshot.matches("(item) . id").count(), 1);
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
    assert!(snapshot.contains("if (__uix_dynamic_text_capture_0) . clone ()"));
    // 动态闭包捕获与两个不可变操作都必须克隆各自所有权。
    assert!(snapshot.matches("clone").count() >= 2);
    // push 必须追加到克隆数组。
    assert!(snapshot.contains("push ((__uix_dynamic_text_capture_2) . clone ())"));
    // removeAt 必须映射为 Vec::remove。
    assert!(snapshot.contains("remove ((__uix_dynamic_text_capture_3) . clone ())"));
    // length 必须映射为 len 调用。
    assert_eq!(snapshot.matches("len ()").count(), 2);
}

// 验证扩展数组操作生成拥有型迭代器与不可变更新结构。
#[test]
fn maps_extended_array_operations_and_restricted_closures() {
    // 用多个文本插值覆盖全部新增操作且避免生成结果参与组件属性类型推断。
    let source = r#"<Column>
        <Text>{values.insertAt(index, value).length}</Text>
        <Text>{values.updateAt(index, value).length}</Text>
        <Text>{values.removeBy(|it| it.id == target).length}</Text>
        <Text>{values.filter(|it| it.active).length}</Text>
        <Text>{values.map(|it| it.name).length}</Text>
        <Text>{values.sortBy(|it| it.order).length}</Text>
        <Text>{values.find(|it| it.id == target)}</Text>
    </Column>"#;
    // 生成全部数组操作 Rust 令牌。
    let snapshot = generate(source).expect("扩展数组操作应生成 Rust 代码");
    // insertAt 必须映射为 Vec::insert。
    assert!(
        snapshot.contains("insert ((__uix_dynamic_text_capture_1) . clone () , (__uix_dynamic_text_capture_2) . clone ())")
    );
    // updateAt 必须映射为索引赋值。
    assert!(snapshot.contains(
        "[(__uix_dynamic_text_capture_1) . clone ()] = (__uix_dynamic_text_capture_2) . clone ()"
    ));
    // removeBy 必须查找首个位置后调用一次 remove。
    assert!(snapshot.contains("iter () . position") && snapshot.contains("remove"));
    // filter 与 map 必须基于克隆数组的拥有型迭代器。
    assert!(snapshot.matches("into_iter").count() >= 3);
    // filter 与 map 必须收集新的 Vec。
    assert!(
        snapshot
            .matches("collect :: < :: std :: vec :: Vec")
            .count()
            >= 2
    );
    // sortBy 必须使用标准库稳定键排序。
    assert!(snapshot.contains("sort_by_key"));
    // find 必须保留 Option 返回语义。
    assert!(snapshot.contains("find"));
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

// 验证 App 已登记入口不能误入 View 目标。
#[test]
fn rejects_app_from_view_target_with_directed_entry_diagnostic() {
    // 完整 View 目标必须返回定向入口诊断。
    let error = generate_test_document_view(r#"<App />"#).expect_err("App 不能由 View 目标生成");
    // 诊断必须明确正确公开入口。
    assert!(error.message.contains("不能生成 <App> 应用入口"));
    assert!(error.suggestion.contains("uix_app!"));
}

// 验证 Rust-only DataTable 不被伪装成未来 UIX 标签。
#[test]
fn rejects_rust_only_data_table_as_unknown_element() {
    // 解析 PascalCase DataTable 标签语法。
    let document = parse_document(r#"<DataTable />"#)
        // 标签词法合法，但没有 UIX 契约登记。
        .expect("DataTable 标签词法本身应合法");
    // 代码生成必须走普通未知标签诊断。
    let error = generate_view(&document.root).expect_err("Rust-only DataTable 必须拒绝 UIX 生成");
    // 诊断不得暗示 DataTable 是规划中的 UIX 标签。
    assert!(!error.message.contains("规划中"));
    // 诊断必须明确缺少 Rust API 映射登记。
    assert!(error.message.contains("尚无已登记的 Rust API 映射"));
}

// 验证文档外未知元素仍返回普通映射诊断。
#[test]
fn rejects_unknown_element_mapping() {
    // 解析未在任何矩阵登记的元素。
    let document = parse_document(r#"<UnknownWidget />"#).expect("PascalCase 标签语法应合法");
    // 代码生成必须拒绝未知元素。
    let error = generate_view(&document.root).expect_err("未知元素必须失败");
    // 诊断必须明确缺少登记映射。
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

// 验证已映射内联样式生成精确 Style 字段更新。
#[test]
fn generates_mapped_inline_style_fields() {
    // 解析覆盖布局、盒模型、颜色、Grid 与显示状态的代表样式。
    let document = parse_document(
        // 使用样式参考中标记为已映射的属性。
        r#"<Text style="display: grid; gridTemplateColumns: 1fr 100px; margin: 1px 2px 3px 4px; paddingLeft: 5px; color: #fff; backgroundColor:hover: rgba(0,0,0,0.5); boxShadow: 0 2px 4px #000; visible: true;">Hello</Text>"#,
    )
    // 样式语法与映射都应成功。
    .expect("已映射样式语法应合法");
    // 生成可消费 View 令牌。
    let tokens = generate_view(&document.root)
        // 已映射属性不得再返回阶段边界诊断。
        .expect("已映射样式应生成 Rust 令牌")
        // 规范化令牌便于断言字段事实。
        .to_string();
    // 令牌必须通过受控样式更新入口。
    assert!(tokens.contains("map_style"));
    // 令牌必须更新 Grid 显示模式与轨道。
    assert!(tokens.contains("display") && tokens.contains("grid_template_columns"));
    // 令牌必须保留四边简写和单边覆盖。
    assert!(tokens.contains("EdgeInsets :: new") && tokens.contains("padding . left"));
    // 令牌必须更新颜色状态与阴影。
    assert!(tokens.contains("background_hover") && tokens.contains("box_shadow"));
    // 生成物不得包含运行时样式解析器。
    assert!(!tokens.contains("parse_style"));
}

// 验证 z-index 复用 View 运行时的绘制与命中层级。
#[test]
fn generates_z_index_runtime_mapping() {
    // 解析同时携带结构层级与普通视觉字段的样式。
    let document = parse_document(r#"<Text style="z-index: -3; color: #fff;">Hello</Text>"#)
        // 已映射的层级语法必须合法。
        .expect("z-index 样式语法应合法");
    // 生成可消费的 View 令牌。
    let tokens = generate_view(&document.root)
        // 运行时已有等价入口时不得继续返回规划中诊断。
        .expect("z-index 应映射到 View 运行时")
        // 规范化令牌便于锁定结构事实。
        .to_string();
    // 普通视觉字段仍通过 Style 受控更新。
    assert!(tokens.contains("map_style") && tokens.contains("color"));
    // 层级必须通过 ViewNode 的公开入口应用，并保留负整数。
    assert!(tokens.contains("z_index") && tokens.contains("- 3i32"));
}

// 验证 z-index 拒绝无法由运行时 i32 精确表达的值。
#[test]
fn rejects_invalid_z_index_values() {
    // 覆盖小数与超过 i32 上界的输入。
    for value in ["1.5", "2147483648"] {
        // 构造单一非法层级属性。
        let source = format!(r#"<Text style="z-index: {value};">Hello</Text>"#);
        // 语法层保留原始值供映射层诊断。
        let document = parse_document(&source).expect("z-index 原始值应完成语法解析");
        // 代码生成必须拒绝非精确 i32。
        let error = generate_view(&document.root).expect_err("非法 z-index 必须失败");
        // 诊断必须同时说明属性与整数契约。
        assert!(error.message.contains("z-index") && error.message.contains("整数"));
    }
}

// 验证 transform 四类函数按源码顺序映射到公开二维矩阵。
#[test]
fn generates_transform_runtime_mapping() {
    // 解析覆盖平移、旋转、非等比缩放与双轴倾斜的函数列表。
    let document = parse_document(
        r#"<Text style="transform: translate(10px, -4px) rotate(0.25turn) scale(2, 0.5) skew(10deg, 0);">Hello</Text>"#,
    )
    // 已登记函数与单位必须完成语法解析。
    .expect("transform 函数列表语法应合法");
    // 生成可消费的 View 令牌。
    let tokens = generate_view(&document.root)
        // 四类函数都已有公开运行时等价入口。
        .expect("transform 应映射到二维仿射运行时")
        // 规范化令牌便于断言组合事实。
        .to_string();
    // 最终节点必须通过唯一公开视觉变换入口更新。
    assert!(tokens.contains("affine_transform"));
    // 四类构造器必须全部保留并使用 concat 组合。
    assert!(
        tokens.contains("Transform :: translate")
            && tokens.contains("Transform :: rotate")
            && tokens.contains("Transform :: scale")
            && tokens.contains("Transform :: skew")
            && tokens.matches("concat").count() == 4
    );
}

// 验证 transform 拒绝未知函数、非法单位与不可逆矩阵。
#[test]
fn rejects_invalid_transform_values() {
    // 覆盖函数白名单、角度单位、相对长度和零缩放。
    for (value, expected) in [
        // 未登记 matrix 函数必须明确拒绝。
        ("matrix(1,0,0,1,0,0)", "未知"),
        // 非零旋转必须提供角度单位。
        ("rotate(45)", "单位"),
        // 百分比平移尚无帧尺寸上下文。
        ("translate(50%)", "百分比"),
        // 零缩放会破坏命中逆变换。
        ("scale(0)", "不可逆"),
    ] {
        // 构造单一待拒绝变换。
        let source = format!(r#"<Text style="transform: {value};">Hello</Text>"#);
        // 样式语法层保留函数文本供映射层诊断。
        let document = parse_document(&source).expect("transform 原始值应完成语法解析");
        // 代码生成必须返回确定诊断。
        let error = generate_view(&document.root).expect_err("非法 transform 必须失败");
        // 诊断必须命中对应失败原因。
        assert!(error.message.contains(expected), "{}", error.message);
    }
}

// 验证已经落地的定位样式继续生成公开五模式契约。
#[test]
fn maps_position_inline_style_at_compile_time() {
    // 解析文档已登记的绝对定位属性。
    let document = parse_document(r#"<Text style="position: absolute;">Hello</Text>"#)
        // 语法层必须接受已实现属性。
        .expect("position 样式语法应合法");
    // 代码生成必须返回公开 View 链。
    let tokens = generate_view(&document.root)
        // 已实现定位不得回退成规划中诊断。
        .expect("position 样式必须生成")
        // 转换成稳定文本供公开枚举断言。
        .to_string();
    // 绝对定位必须映射到公开 PositionMode 枚举。
    assert!(tokens.contains("position") && tokens.contains("PositionMode :: Absolute"));
}

// 验证已映射字段的未实现子取值也不会被静默近似。
#[test]
fn rejects_percentage_dimension_without_rust_equivalent() {
    // 解析百分比宽度。
    let document = parse_document(r#"<Text style="width: 50%;">Hello</Text>"#)
        // 语法层保留目标设计值。
        .expect("百分比尺寸语法应合法");
    // 映射层必须拒绝当前 Style 无法表达的百分比。
    let error = generate_view(&document.root).expect_err("百分比不得伪装为像素值");
    // 诊断必须说明等价表示缺失。
    assert!(error.message.contains("百分比") && error.message.contains("等价"));
}

// 验证 setState 明确需要后续 Widget 上下文。
#[test]
fn rejects_set_state_without_widget_context() {
    // 解析包含 setState 的按钮事件。
    let document = parse_document(
        // 使用当前表达式 Gate 已接受的命名参数形式。
        r#"<Button @click="setState(count: count + 1)">Add</Button>"#,
    )
    // 语法层应接受组件内置操作。
    .expect("setState 语法本身应合法");
    // 当前核心 View 生成必须拒绝缺失状态上下文。
    let error = generate_view(&document.root).expect_err("setState 需要 Widget Gate");
    // 诊断必须明确 Widget state 上下文。
    assert!(error.message.contains("Widget state"));
}

// 验证带 key 的循环要求唯一直接行根。
#[test]
fn rejects_keyed_for_with_multiple_direct_roots() {
    // 构造会生成两个直接行节点的循环。
    let source = r#"<Column><For {item} in {items} key={item.id}><Text>A</Text><Text>B</Text></For></Column>"#;
    // 完整文档生成必须拒绝不稳定的多根 key。
    let error = generate_test_document_view(source).expect_err("key 需要唯一行根");
    // 诊断必须说明唯一直接子节点约束。
    assert!(error.message.contains("恰好生成一个直接子节点"));
}

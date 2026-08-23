// 引入文档解析与公开 View 生成入口。
use super::{Diagnostic, generate_view, parse_document};

// 解析单根布局文档并返回稳定令牌文本。
fn generate(source: &str) -> Result<String, Diagnostic> {
    // 解析经过结构验证的文档。
    let document = parse_document(source)?;
    // 生成公开 View 令牌。
    let tokens = generate_view(&document.root)?;
    // 使用 proc_macro2 的稳定空白规范形成断言文本。
    Ok(tokens.to_string())
}

// UIX 纵向 Flex 默认不吞占剩余高度，只有显式 flexGrow 才参与增长。
#[test]
fn keeps_vertical_flex_at_default_zero_grow() {
    let column =
        generate(r#"<Column><Text>A</Text></Column>"#).expect("默认 Column 应生成固有高度容器");
    let container = generate(r#"<Container><Text>A</Text></Container>"#)
        .expect("默认纵向 Container 应生成固有高度容器");
    let column_cell = generate(r#"<Row><Col><Text>A</Text></Col></Row>"#)
        .expect("默认 Col 内容应生成固有高度容器");
    assert!(column.contains("prelude :: column_fit"));
    assert!(container.contains("prelude :: column_fit"));
    assert!(column_cell.contains("prelude :: column_fit"));
    assert!(!column.contains("flex_grow"));
    assert!(!container.contains("flex_grow"));

    let growing = generate(r#"<Column flexGrow={1}><Text>A</Text></Column>"#)
        .expect("显式增长 Column 应保留公共属性覆盖");
    assert!(
        growing.contains("prelude :: column_fit") && growing.contains("flex_grow (1"),
        "{growing}",
    );
}

// 验证 Row/Col 生成现有 24 单元响应式 Grid 契约。
#[test]
fn generates_responsive_row_col_contract() {
    // 覆盖 Row 公共布局属性与全部响应式 Col 结构属性。
    let source = r#"<Row gap="16px" align="center" justify="space-between"><Col span={8} offset={1} order={-1} sm={12} md={10} lg={8} xl={6} xxl={4}><Text>Left</Text></Col><Col span={16}><Text>Right</Text></Col></Row>"#;
    // 生成确定性布局令牌。
    let tokens = generate(source).expect("Row/Col 文档契约应生成 Rust View");
    // Row 必须复用公开 GridBuilder，而不是旧 Flex row 别名。
    assert!(tokens.contains("prelude :: grid") && tokens.contains("responsive"));
    // 列配置必须与源码顺序子 View 一一绑定。
    assert!(tokens.contains("cols") && tokens.matches("Col :: new").count() == 2);
    // 基础跨度、偏移与视觉顺序必须进入现有 Col API。
    assert!(
        // 检查基础跨度。
        tokens.contains("span (8")
            // 检查基础偏移。
            && tokens.contains("offset (1")
            // 检查负视觉顺序。
            && tokens.contains("order (- 1")
    );
    // 五个响应式断点必须完整生成。
    for method in ["sm", "md", "lg", "xl", "xxl"] {
        // 每个断点方法必须出现在令牌中。
        assert!(
            tokens.contains(&format!("{method} (")),
            "缺少 {method} 映射"
        );
    }
    // Row 公共 gap、align 与 justify 必须保留。
    assert!(
        // 检查统一间距。
        tokens.contains("gap (16")
            // 检查交叉轴对齐。
            && tokens.contains("AlignItems :: Center")
            // 检查主轴分布。
            && tokens.contains("JustifyContent :: SpaceBetween")
    );
    // 两列内容必须保持源码顺序。
    let left = tokens.find("Left").expect("首列内容应存在");
    // 定位第二列内容。
    let right = tokens.find("Right").expect("次列内容应存在");
    // 首列内容必须先于次列。
    assert!(left < right);
}

// 验证 Container 第一批视觉与布局简写映射到既有公开 View 契约。
#[test]
fn generates_container_visual_and_layout_shorthands() {
    // 构造覆盖确定关键字、静态数值与动态表达式的 Container。
    let tokens = generate(
        // 保留外部变量名以直接检查动态布尔和高度令牌。
        r##"<Container alignSelf="center" bg="#112233" radius="8px" w="120px" h={panel_height} opacity="0.5" visible={shown} overflow="hidden" border="1px #d9d9d9" shadow="-2px 4px 8px rgba(0, 0, 0, 0.15)"><Text>内容</Text></Container>"##,
    )
    // 全部登记简写必须生成成功。
    .expect("Container 第一批简写应生成");
    // alignSelf 必须复用统一 AlignItems。
    assert!(tokens.contains("align_self") && tokens.contains("AlignItems :: Center"));
    // 背景色必须进入既有 bg 入口。
    assert!(tokens.contains("bg") && tokens.contains("#112233"));
    // 静态圆角必须进入 radius。
    assert!(tokens.contains("radius (8"));
    // 宽度简写必须进入固定宽度入口。
    assert!(tokens.contains("width (120"));
    // 动态高度表达式必须保持原变量。
    assert!(tokens.contains("height (panel_height)"));
    // 透明度必须进入整体透明度入口。
    assert!(tokens.contains("opacity (0.5"));
    // 动态可见性必须保持布尔表达式。
    assert!(tokens.contains("visible (shown)"));
    // hidden 必须生成显式子树裁剪。
    assert!(tokens.contains("clip_content (true)"));
    // border 必须生成非负宽度与具体颜色值。
    assert!(tokens.contains("border (1") && tokens.contains("217u8 , 217u8 , 217u8 , 255u8"));
    // shadow 必须保留负横向与正纵向偏移。
    assert!(
        tokens.contains("box_shadow")
            && tokens.contains("BoxShadowDef :: new")
            && tokens.contains("8.0 , - 2.0 , 4.0"),
        "{tokens}"
    );
    // none 必须显式清除 Container 阴影。
    let none = generate(r#"<Container shadow="none" />"#)
        // 明确清除语法必须生成成功。
        .expect("Container shadow none 应生成");
    // 清除阴影必须调用公开完整阴影入口并传入 None。
    assert!(none.contains("box_shadow") && none.contains("Option :: None"));
}

// 验证 Container 阴影简写同时支持旧语法与有符号 spread。
#[test]
fn generates_container_box_shadow_spread_contract() {
    // 生成旧四段阴影简写。
    let legacy = generate(r##"<Container shadow="-2px 4px 8px #000000" />"##)
        // 旧语法必须继续生成成功。
        .expect("Container shadow 旧语法应保持兼容");
    // 旧语法必须显式保持零 spread。
    assert!(legacy.contains("with_spread (0.0)"), "{legacy}");
    // 生成带负 spread 的五段阴影简写。
    let spread = generate(r##"<Container shadow="-2px 4px 8px -3px #000000" />"##)
        // 五段阴影语法必须生成成功。
        .expect("Container shadow 应支持有符号 spread");
    // 生成物必须把 spread 保存在公开阴影定义中。
    assert!(spread.contains("with_spread (- 3.0)"), "{spread}");
}

// 验证 Container 阴影拒绝非有限 spread。
#[test]
fn rejects_non_finite_container_box_shadow_spread() {
    // 非有限 spread 不得进入运行时几何。
    let spread = generate(r##"<Container shadow="0 2px 4px NaN #000000" />"##)
        // 提取有限值诊断。
        .expect_err("非有限 Container shadow spread 必须失败");
    // 诊断必须明确要求有限数值。
    assert!(spread.message.contains("必须有限"));
}

// 验证 Container overflow 与内联样式共享显式裁剪契约。
#[test]
fn maps_container_overflow_without_reusing_scroll_layout_flag() {
    // 直接简写 visible 必须显式清除旧裁剪。
    let visible = generate(r#"<Container overflow="visible" />"#)
        // 文档默认关键字必须生成成功。
        .expect("Container overflow visible 应生成");
    // visible 必须调用裁剪入口且传入 false。
    assert!(visible.contains("clip_content (false)"));
    // 规范内联样式 hidden 必须更新新三态字段。
    let style = generate(r#"<Container style="overflow: hidden;" />"#)
        // 已登记样式属性必须继续生成。
        .expect("内联 overflow hidden 应生成");
    // 样式生成不得再误用自然尺寸溢出字段。
    assert!(style.contains("clip_content") && !style.contains("overflow_content ="));
    // hidden 必须保存显式 Some(true)。
    assert!(style.contains("Option :: Some (true)"));
}

// 验证 Container 简写静态边界和滚动所有权诊断。
#[test]
fn rejects_invalid_container_shorthand_values() {
    // 负圆角不得传入运行时。
    let radius = generate(r#"<Container radius="-1px" />"#)
        // 提取预期范围诊断。
        .expect_err("负 Container radius 必须失败");
    // 诊断必须点名 radius 与越界值。
    assert!(radius.message.contains("radius") && radius.message.contains("-1"));
    // 大于一的透明度不得静默钳制。
    let opacity = generate(r#"<Container opacity="1.5" />"#)
        // 提取预期范围诊断。
        .expect_err("越界 Container opacity 必须失败");
    // 建议必须给出零到一范围。
    assert!(opacity.suggestion.contains("0 到 1"));
    // Container 不拥有滚动状态。
    let scroll = generate(r#"<Container overflow="scroll" />"#)
        // 提取滚动所有权诊断。
        .expect_err("Container overflow scroll 必须失败");
    // 诊断必须引导到专用滚动组件。
    assert!(scroll.suggestion.contains("ScrollView"));
    // 未登记 alignSelf 关键字必须复用统一对齐诊断。
    let align = generate(r#"<Container alignSelf="baseline" />"#)
        // 提取预期关键字诊断。
        .expect_err("未登记 alignSelf 必须失败");
    // 诊断必须保留实际关键字。
    assert!(align.message.contains("baseline"));
    // 负边框宽度不得进入公开 View API。
    let border = generate(r##"<Container border="-1px #d9d9d9" />"##)
        // 提取非负长度诊断。
        .expect_err("负 Container border 宽度必须失败");
    // 诊断必须说明数值不能为负。
    assert!(border.message.contains("不能为负"));
    // 负模糊半径不得生成盒阴影定义。
    let shadow = generate(r##"<Container shadow="0 2px -4px #000000" />"##)
        // 提取非负模糊半径诊断。
        .expect_err("负 Container shadow 模糊半径必须失败");
    // 诊断必须说明数值不能为负。
    assert!(shadow.message.contains("不能为负"));
    // 动态复合阴影不得在编译器中猜测分词规则。
    let dynamic = generate(r#"<Container shadow={card_shadow} />"#)
        // 提取复合字面量形状诊断。
        .expect_err("动态 Container shadow 必须失败");
    // 建议必须指向规范样式或 Rust API。
    assert!(dynamic.message.contains("编译期确定") && dynamic.suggestion.contains("Rust API"));
    // 颜色名称不得绕过具体颜色语法边界。
    let named = generate(r#"<Container border="1px red" />"#)
        // 提取具体颜色语法诊断。
        .expect_err("Container border 颜色名称必须失败");
    // 建议必须列出具体颜色形态。
    assert!(named.suggestion.contains("rgb") && named.suggestion.contains("十六进制"));
}

// 验证 Grid/Col 生成显式轨道与跨轨道样式。
#[test]
fn generates_explicit_grid_col_contract() {
    // 覆盖三类轨道、独立间距、内边距与两种列跨度写法。
    let source = r#"<Grid columns="1fr 200px auto" rows="auto 2fr" gap="16px" colGap="8px" rowGap="4px" padding="12px"><Col span={2}><Text>A</Text></Col><Col gridColumnSpan={1} gridRowSpan={2}><Text>B</Text></Col></Grid>"#;
    // 生成确定性布局令牌。
    let tokens = generate(source).expect("Grid/Col 文档契约应生成 Rust View");
    // 列轨道必须包含 Fr、Px 与 Auto。
    assert!(
        // 检查弹性轨道。
        tokens.contains("GridTrack :: Fr (1")
            // 检查固定轨道。
            && tokens.contains("GridTrack :: Px (200")
            // 检查自动轨道。
            && tokens.contains("GridTrack :: Auto")
    );
    // 行轨道必须进入公开 rows 构建器。
    assert!(tokens.contains("rows") && tokens.contains("GridTrack :: Fr (2"));
    // 独立间距与公共间距、内边距必须全部保留。
    assert!(
        // 检查独立列间距。
        tokens.contains("col_gap (8")
            // 检查独立行间距。
            && tokens.contains("row_gap (4")
            // 检查统一间距。
            && tokens.contains("gap (16")
            // 检查统一内边距。
            && tokens.contains("padding (12")
    );
    // 独立轴间距必须在统一 gap 之后应用并覆盖对应轴。
    let gap = tokens.find("gap (16").expect("统一 Grid gap 应存在");
    // 定位独立列间距。
    let col_gap = tokens.find("col_gap (8").expect("Grid colGap 应存在");
    // 定位独立行间距。
    let row_gap = tokens.find("row_gap (4").expect("Grid rowGap 应存在");
    // 两个独立值必须晚于统一值。
    assert!(gap < col_gap && gap < row_gap);
    // 两个 Col 必须分别形成 2x1 与 1x2 跨度。
    assert!(
        // 检查首列跨度。
        tokens.contains("grid_span (2u32 , 1u32)")
            // 检查次列跨度。
            && tokens.contains("grid_span (1u32 , 2u32)")
    );
}

// 验证 Flex 横向入口与 Row 栅格语义清晰分离。
#[test]
fn keeps_flex_row_on_container_and_rejects_row_direction() {
    // Flex 横向布局继续使用文档化 Container direction。
    let flex = generate(r#"<Container direction="row"><Text>A</Text></Container>"#)
        // Container 横向入口必须仍可生成。
        .expect("Container direction=row 应保留 Flex 横向能力");
    // Flex Container 必须调用公开 row 构造器而不是 Grid。
    assert!(flex.contains("prelude :: row") && !flex.contains("prelude :: grid"));
    // Row 不再接受旧 direction 属性。
    let error = generate(r#"<Row direction="row"><Col><Text>A</Text></Col></Row>"#)
        // 提取预期迁移诊断。
        .expect_err("Row direction 必须明确迁移到 Container");
    // 修复建议必须给出 Flex 横向入口。
    assert!(error.suggestion.contains("Container") && error.suggestion.contains("direction"));
}

// 验证 Row/Col 的父子形状与整数范围在编译期拒绝。
#[test]
fn validates_responsive_row_col_shape_and_ranges() {
    // Row 的可见非 Col 直接子节点不得被静默包装。
    let direct_child = generate(r#"<Row><Text>A</Text></Row>"#)
        // 提取预期父子形状诊断。
        .expect_err("Row 直接 Text 必须失败");
    // 诊断必须指出直接 Col 约束。
    assert!(direct_child.message.contains("直接 <Col>"));
    // 零跨度违反 1..=24 契约。
    let zero = generate(r#"<Row><Col span={0}><Text>A</Text></Col></Row>"#)
        // 提取预期范围诊断。
        .expect_err("Col 零跨度必须失败");
    // 诊断必须包含文档范围。
    assert!(zero.message.contains("1..=24"));
    // 超过 24 的响应式跨度必须失败。
    let overflow = generate(r#"<Row><Col lg={25}><Text>A</Text></Col></Row>"#)
        // 提取预期范围诊断。
        .expect_err("Col 响应式跨度越界必须失败");
    // 诊断必须包含具体属性和值。
    assert!(overflow.message.contains("Col lg=25"));
    // 动态结构值无法静态验证时必须明确拒绝。
    let dynamic = generate(r#"<Row><Col span={column_span}><Text>A</Text></Col></Row>"#)
        // 提取预期结构常量诊断。
        .expect_err("动态 Col span 必须失败");
    // 诊断必须说明编译期整数要求。
    assert!(dynamic.message.contains("编译期无符号整数"));
}

// 验证 Grid 的轨道、Col 上下文与冲突跨度拒绝路径。
#[test]
fn validates_explicit_grid_contract_errors() {
    // Grid 缺少 columns 时不得生成无轨道布局。
    let missing = generate(r#"<Grid><Col><Text>A</Text></Col></Grid>"#)
        // 提取缺失结构属性诊断。
        .expect_err("Grid 缺少 columns 必须失败");
    // 修复建议必须给出轨道格式。
    assert!(missing.suggestion.contains("1fr"));
    // 百分比轨道尚未登记，必须明确拒绝。
    let unit = generate(r#"<Grid columns="50% 1fr"><Col><Text>A</Text></Col></Grid>"#)
        // 提取非法单位诊断。
        .expect_err("Grid 百分比轨道必须失败");
    // 诊断必须包含具体非法轨道与合法单位。
    assert!(
        // 消息保留具体非法轨道。
        unit.message.contains("50%")
            // 修复建议给出固定轨道格式。
            && unit.suggestion.contains("100px")
            // 修复建议同时给出弹性轨道格式。
            && unit.suggestion.contains("1fr")
    );
    // Grid 直接非 Col 子项不得被静默包装。
    let direct_child = generate(r#"<Grid columns="1fr"><Text>A</Text></Grid>"#)
        // 提取父子形状诊断。
        .expect_err("Grid 直接 Text 必须失败");
    // 诊断必须指出直接 Col 约束。
    assert!(direct_child.message.contains("直接 <Col>"));
    // 两种同义列跨度不能同时出现。
    let conflict = generate(
        // 构造冲突 Col。
        r#"<Grid columns="1fr 1fr"><Col span={1} gridColumnSpan={2}><Text>A</Text></Col></Grid>"#,
    )
    // 提取冲突诊断。
    .expect_err("重复列跨度来源必须失败");
    // 修复建议必须要求二选一。
    assert!(conflict.message.contains("不能同时") && conflict.suggestion.contains("其中一个"));
    // Col 脱离父布局时必须给出上下文诊断。
    let orphan = generate(r#"<Col span={8}><Text>A</Text></Col>"#)
        // 提取孤立 Col 诊断。
        .expect_err("孤立 Col 必须失败");
    // 诊断必须列出两个合法父级。
    assert!(orphan.message.contains("Row") && orphan.message.contains("Grid"));
}

// 验证 ScrollView 生成公开滚动构建器、方向与双向偏移绑定。
#[test]
fn generates_scroll_view_contract() {
    // 构造双轴滚动、状态绑定与公共尺寸属性。
    let source = r#"<ScrollView direction="both" offset={scroll_offset} height="320px"><Column><Text>A</Text><Text>B</Text></Column></ScrollView>"#;
    // 生成确定性滚动布局令牌。
    let tokens = generate(source).expect("ScrollView 文档契约应生成 Rust View");
    // 标签必须复用公开 scroll 构造器。
    assert!(tokens.contains("prelude :: scroll"));
    // 双轴关键字必须映射到公开 both 方法。
    assert!(tokens.contains("both"));
    // offset 必须以引用形式进入公开双向 State 绑定。
    assert!(tokens.contains("scroll_offset") && tokens.contains("& (scroll_offset)"));
    // 公共高度属性继续走统一 StyleExt 契约。
    assert!(tokens.contains("height (320"));
    // 唯一 Column 内容必须保持两个文本节点的源码顺序。
    let first = tokens.find("A").expect("首个滚动内容应存在");
    // 定位第二个滚动内容。
    let second = tokens.find("B").expect("第二个滚动内容应存在");
    // 源码顺序不得被滚动包装改变。
    assert!(first < second);
}

// 验证 ScrollView 在编译期拒绝无内容、多内容与非法专有属性。
#[test]
fn validates_scroll_view_contract_errors() {
    // 空滚动容器没有内容范围，必须失败。
    let empty = generate(r#"<ScrollView />"#)
        // 提取预期的缺失内容诊断。
        .expect_err("空 ScrollView 必须失败");
    // 诊断必须说明唯一可渲染子节点要求。
    assert!(empty.message.contains("一个可渲染直接子节点"));
    // 多个直接内容节点不得被宏静默包裹。
    let multiple = generate(r#"<ScrollView><Text>A</Text><Text>B</Text></ScrollView>"#)
        // 提取预期的父子形状诊断。
        .expect_err("多子节点 ScrollView 必须失败");
    // 修复建议必须要求显式内容布局容器。
    assert!(multiple.message.contains("只能包含一个") && multiple.suggestion.contains("Column"));
    // 字符串 offset 不能表达 State<Point> 绑定。
    let offset = generate(r#"<ScrollView offset="0,0"><Text>A</Text></ScrollView>"#)
        // 提取预期的绑定形状诊断。
        .expect_err("字面量 offset 必须失败");
    // 诊断必须指向 State<Point> 与表达式写法。
    assert!(offset.message.contains("State<Point>") && offset.suggestion.contains("offset={"));
    // 未登记方向不得回退到默认垂直方向。
    let direction = generate(r#"<ScrollView direction="diagonal"><Text>A</Text></ScrollView>"#)
        // 提取预期的关键字诊断。
        .expect_err("非法 ScrollView direction 必须失败");
    // 修复建议必须列出全部合法方向。
    assert!(
        // 检查垂直方向关键字。
        direction.suggestion.contains("vertical")
            // 检查水平方向关键字。
            && direction.suggestion.contains("horizontal")
            // 检查双轴方向关键字。
            && direction.suggestion.contains("both")
    );
}

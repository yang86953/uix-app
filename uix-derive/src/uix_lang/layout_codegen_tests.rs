// 引入文档解析与公开 View 生成入口。
use super::{generate_view, parse_document, Diagnostic};

// 解析单根布局文档并返回稳定令牌文本。
fn generate(source: &str) -> Result<String, Diagnostic> {
    // 解析经过结构验证的文档。
    let document = parse_document(source)?;
    // 生成公开 View 令牌。
    let tokens = generate_view(&document.root)?;
    // 使用 proc_macro2 的稳定空白规范形成断言文本。
    Ok(tokens.to_string())
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

// 引入核心 View 生成与文档解析入口。
use super::{generate_document_view, generate_view, parse_document};

// 验证四种背景来源生成公开运行时契约。
#[test]
fn style_background_generates_all_image_sources() {
    // 覆盖显式 none、本地路径与两种双色渐变。
    for (source, fragment) in [
        // 显式关闭背景图。
        ("none", "BackgroundImage :: None"),
        // 使用带空格的本地相对路径。
        ("url('assets/hero image.png')", "BackgroundImage :: Url"),
        // 使用普通颜色与透明色的线性渐变。
        (
            "linear-gradient(#ffffff, rgba(0,0,0,0.5))",
            "BackgroundImage :: LinearGradient",
        ),
        // 使用命名色的径向渐变。
        (
            "radial-gradient(white, black)",
            "BackgroundImage :: RadialGradient",
        ),
    ] {
        // 构造只包含目标背景来源的元素文档。
        let document = parse_document(&format!(
            // 保留函数与颜色源码供映射层解析。
            r#"<Text style="backgroundImage: {source};">背景</Text>"#
        ))
        // 样式语法必须接受规范背景值。
        .expect("backgroundImage 语法应合法");
        // 生成真实 Style 字段更新令牌。
        let tokens = generate_view(&document.root)
            // 已映射来源不得继续返回规划中诊断。
            .expect("backgroundImage 应映射到运行时来源")
            // 规范化令牌用于断言公开契约。
            .to_string();
        // 生成结果必须显式更新可选背景图字段。
        assert!(tokens.contains("background_image"));
        // 运行时枚举变体必须与文档来源一致。
        assert!(tokens.contains(fragment));
    }
}

// 验证关键词、像素与百分比背景定位映射。
#[test]
fn style_background_generates_position_axes() {
    // 覆盖单值补齐、关键词换序和数值 x y 顺序。
    for (source, fragments) in [
        // 单 center 同时居中两个轴。
        ("center", ["Center", "Center"]),
        // 垂直在前、水平在后必须重排为 x y。
        ("top right", ["End", "Start"]),
        // 两个像素值按 x y 解释。
        ("12px -8px", ["Pixels (12.0)", "Pixels (- 8.0)"]),
        // 百分比转换为零到一比例。
        ("50% 25%", ["Percent (0.5)", "Percent (0.25)"]),
    ] {
        // 构造只包含目标背景定位的元素文档。
        let document = parse_document(&format!(
            // 保留空格分隔轴值。
            r#"<Text style="backgroundPosition: {source};">背景</Text>"#
        ))
        // 样式语法必须接受原始定位文本。
        .expect("backgroundPosition 语法应合法");
        // 生成公开二维定位构造器令牌。
        let tokens = generate_view(&document.root)
            // 已映射定位不得返回规划中诊断。
            .expect("backgroundPosition 应映射到运行时定位")
            // 规范化令牌供轴顺序断言。
            .to_string();
        // 字段必须通过显式 Some 更新。
        assert!(tokens.contains("background_position"));
        // 第一个片段必须出现在生成结果中。
        assert!(
            tokens.contains(fragments[0]),
            "背景定位 {source:?} 缺少 {:?}，实际令牌：{tokens}",
            fragments[0]
        );
        // 第二个片段必须出现在生成结果中。
        assert!(
            tokens.contains(fragments[1]),
            "背景定位 {source:?} 缺少 {:?}，实际令牌：{tokens}",
            fragments[1]
        );
        // 两个不同轴片段必须按运行时 x y 顺序出现。
        if fragments[0] != fragments[1] {
            // 查找水平轴片段位置。
            let x_index = tokens.find(fragments[0]).expect("已确认水平轴片段存在");
            // 查找垂直轴片段位置。
            let y_index = tokens.find(fragments[1]).expect("已确认垂直轴片段存在");
            // 水平轴必须先交给 BackgroundPosition::new。
            assert!(
                x_index < y_index,
                "背景定位 {source:?} 的 x y 轴顺序错误：{tokens}"
            );
        }
    }
}

// 验证四种背景重复值完整映射。
#[test]
fn style_background_generates_all_repeat_modes() {
    // 覆盖完整文档关键字与运行时枚举变体。
    for (source, variant) in [
        // 双轴重复。
        ("repeat", "Repeat"),
        // 水平重复。
        ("repeat-x", "RepeatX"),
        // 垂直重复。
        ("repeat-y", "RepeatY"),
        // 不重复。
        ("no-repeat", "NoRepeat"),
    ] {
        // 构造单一重复属性文档。
        let document = parse_document(&format!(
            // 保留规范关键字。
            r#"<Text style="backgroundRepeat: {source};">背景</Text>"#
        ))
        // 样式语法必须接受关键字。
        .expect("backgroundRepeat 语法应合法");
        // 生成真实 Style 字段更新令牌。
        let tokens = generate_view(&document.root)
            // 四种值都应完成映射。
            .expect("backgroundRepeat 应映射到运行时枚举")
            // 规范化令牌供枚举断言。
            .to_string();
        // 必须更新背景重复字段。
        assert!(tokens.contains("background_repeat"));
        // 必须生成对应公开枚举变体。
        assert!(tokens.contains(&format!("BackgroundRepeat :: {variant}")));
    }
}

// 验证样式类与伪状态都保留背景字段差异更新。
#[test]
fn style_background_pseudo_state_keeps_background_deltas() {
    // 基础类使用图片，hover 改为渐变和不重复。
    let document = parse_document(
        // 通过真实组件进入动态伪类降低路径。
        r#"
        backdrop { backgroundImage: url('normal.png'); backgroundPosition: left top; }
        backdrop:hover { backgroundImage: linear-gradient(red, blue); backgroundRepeat: no-repeat; }
        <Widget name="Backdrop"><Text class="backdrop">背景</Text></Widget>
        <Backdrop />
        "#,
    )
    // 文档与伪类声明必须合法。
    .expect("背景伪类语法应合法");
    // 生成完整组件令牌。
    let tokens = generate_document_view(&document)
        // 基础与 hover 差异都必须可降低。
        .expect("背景伪类应生成 Style 差异")
        // 规范化令牌供字段计数。
        .to_string();
    // 基础本地图片必须保留。
    assert!(tokens.contains("normal.png"));
    // hover 线性渐变必须保留。
    assert!(tokens.contains("LinearGradient"));
    // hover 不重复值必须保留。
    assert!(tokens.contains("NoRepeat"));
}

// 验证网络、多层、扩展渐变与非法 URL 都被明确拒绝。
#[test]
fn style_background_rejects_unsupported_image_sources() {
    // 覆盖首批契约明确排除的代表来源。
    for source in [
        // 网络 URL。
        "url('https://example.com/image.png')",
        // 未加引号路径。
        "url(image.png)",
        // 多背景层。
        "url('a.png'), url('b.png')",
        // 非有限角度必须拒绝。
        "linear-gradient(NaNdeg, red, blue)",
        // 三个颜色端点。
        "radial-gradient(red, green, blue)",
    ] {
        // 构造单一非法背景来源文档。
        let document = parse_document(&format!(
            // 保留原值供代码生成层诊断。
            r#"<Text style="backgroundImage: {source};">背景</Text>"#
        ))
        // 语法层只负责保留原始函数文本。
        .expect("backgroundImage 原始值应完成语法解析");
        // 映射层必须拒绝超出首批契约的来源。
        let error = generate_view(&document.root).expect_err("不支持的背景来源必须失败");
        // 诊断必须明确指向 backgroundImage 能力。
        assert!(error.message.contains("backgroundImage") || error.message.contains("背景渐变") || error.message.contains("linear-gradient"));
    }
}

// 验证非法定位和重复关键字返回确定诊断。
#[test]
fn style_background_rejects_invalid_position_and_repeat() {
    // 覆盖越界百分比、同轴关键词、混合歧义与无单位长度。
    for source in ["101%", "left right", "left 10px", "12 8px"] {
        // 构造单一非法定位文档。
        let document = parse_document(&format!(
            // 保留原值供映射层诊断。
            r#"<Text style="backgroundPosition: {source};">背景</Text>"#
        ))
        // 样式语法层不负责定位支持矩阵。
        .expect("backgroundPosition 原始值应完成语法解析");
        // 映射层必须拒绝非法定位。
        let error = generate_view(&document.root).expect_err("非法背景定位必须失败");
        // 诊断必须说明定位闭合语法。
        assert!(error.message.contains("backgroundPosition 只支持"));
    }
    // 构造非法重复关键字文档。
    let document = parse_document(
        // stretch 不属于四种重复值。
        r#"<Text style="backgroundRepeat: stretch;">背景</Text>"#,
    )
    // 样式语法层保留原关键字。
    .expect("backgroundRepeat 原始值应完成语法解析");
    // 映射层必须返回闭合集合诊断。
    let error = generate_view(&document.root).expect_err("非法重复方式必须失败");
    // 诊断必须列出 repeat 支持边界。
    assert!(error.message.contains("backgroundRepeat 只支持"));
}

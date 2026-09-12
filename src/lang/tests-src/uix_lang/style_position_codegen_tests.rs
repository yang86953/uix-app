// 引入核心 View 生成与文档解析入口。
use super::{generate_document_view, generate_view, parse_document};

// 验证 position 五种文档值映射到公开布局模式。
#[test]
fn generates_all_position_modes() {
    // 覆盖每个文档值及其运行时枚举变体。
    for (source, variant) in [
        // 正常流模式。
        ("static", "Static"),
        // 保留槽位的视觉偏移模式。
        ("relative", "Relative"),
        // 最近定位祖先脱流模式。
        ("absolute", "Absolute"),
        // 根视口脱流模式。
        ("fixed", "Fixed"),
        // 最近滚动视口停靠模式。
        ("sticky", "Sticky"),
    ] {
        // 构造只包含目标模式的元素文档。
        let document = parse_document(&format!(
            // 保留关键字源码供编译期映射。
            r#"<Text style="position: {source};">定位</Text>"#
        ))
        // 语法层必须接受规范样式值。
        .expect("position 语法应合法");
        // 生成真实 View 调用令牌。
        let tokens = generate_view(&document.root)
            // 已映射值不得继续返回规划中诊断。
            .expect("position 应映射到运行时模式")
            // 规范化令牌用于断言公开契约。
            .to_string();
        // 最终节点必须通过唯一公开 position 入口更新。
        assert!(tokens.contains("position"));
        // 模式必须来自 UI prelude 公开枚举。
        assert!(tokens.contains(&format!("PositionMode :: {variant}")));
    }
}

// 验证四边像素值与 auto 生成逐边差异调用。
#[test]
fn generates_position_insets_without_resetting_other_edges() {
    // 同时包含正负像素、浮点像素和 auto。
    let document = parse_document(
        // 四边必须保持独立生成顺序。
        r#"<Text style="top: 10px; right: -2.5px; bottom: auto; left: 4px;">定位</Text>"#,
    )
    // 语法层必须保留完整四边值。
    .expect("四边定位语法应合法");
    // 生成真实 View 调用令牌。
    let tokens = generate_document_view(&document)
        // 四边都已映射。
        .expect("四边定位应映射到逐边入口")
        // 规范化令牌用于断言方法名。
        .to_string();
    // 顶边应生成显式像素调用。
    assert!(tokens.contains("top (:: uix_app :: prelude :: StyleLength :: Px (10"));
    // 右边应保留负浮点值。
    assert!(tokens.contains("right (:: uix_app :: prelude :: StyleLength :: Px (- 2.5"));
    // bottom auto 应只清除底边。
    assert!(tokens.contains("bottom_auto"));
    // 左边应生成显式像素调用。
    assert!(tokens.contains("left (:: uix_app :: prelude :: StyleLength :: Px (4"));
    // 未声明为 auto 的边不得调用清除入口。
    assert!(!tokens.contains("top_auto"));
}

// 验证伪类四边差异保留基础类未声明边。
#[test]
fn pseudo_position_inset_generates_single_edge_reset() {
    // 基础类声明左右边，hover 只把 top 恢复为 auto。
    let document = parse_document(
        // 使用组件包裹 hover 状态以进入真实伪类降低路径。
        r#"
        positioned { position: absolute; top: 8px; left: 12px; }
        positioned:hover { top: auto; }
        <Widget name="Positioned"><Text class="positioned">定位</Text></Widget>
        <Positioned />
        "#,
    )
    // 文档与状态声明必须合法。
    .expect("定位伪类语法应合法");
    // 生成完整组件令牌。
    let tokens = generate_document_view(&document)
        // 伪类定位差异必须可降低。
        .expect("定位伪类应生成逐边差异")
        // 规范化令牌供方法断言。
        .to_string();
    // 基础类保留左边像素声明。
    assert!(tokens.contains("left (:: uix_app :: prelude :: StyleLength :: Px (12"));
    // hover 分支只清除顶边。
    assert!(tokens.contains("top_auto"));
    // hover 不得清除未声明的左边。
    assert!(!tokens.contains("left_auto"));
}

// 验证未知 position 关键字产生确定诊断。
#[test]
fn rejects_unknown_position_mode() {
    // 构造文档未登记的 float 值。
    let document = parse_document(r#"<Text style="position: float;">定位</Text>"#)
        // 样式语法层保留原始关键字供映射层诊断。
        .expect("position 原始值应完成语法解析");
    // 代码生成必须返回支持边界诊断。
    let error = generate_view(&document.root).expect_err("未知 position 必须失败");
    // 诊断必须包含五模式支持边界。
    assert!(error.message.contains("position 只支持"));
}

// 验证百分比与有限 calc 保留单位进入运行时，由定位包含块解析。
#[test]
fn generates_percent_and_calc_position_insets() {
    let document = parse_document(
        r#"<Text style="left: 10%; right: calc(50% - 8px); top: calc(#padding + 2px);">定位</Text>"#,
    )
    .expect("百分比与 calc 定位语法应合法");
    let tokens = generate_document_view(&document)
        .expect("百分比与 calc 应映射到逐边入口")
        .to_string();
    assert!(
        tokens.contains("left (:: uix_app :: prelude :: StyleLength :: Percent (10"),
        "{tokens}"
    );
    assert!(
        tokens.contains("right (:: uix_app :: prelude :: StyleLength :: Calc"),
        "{tokens}"
    );
    assert!(tokens.contains("px : - 8"), "{tokens}");
    assert!(tokens.contains("percent : 50"), "{tokens}");
    assert!(
        tokens.contains("padding ()"),
        "token 项应在运行期读取：{tokens}"
    );
}

// 验证无单位、非有限、乘除与未知单位都被明确拒绝。
#[test]
fn rejects_unsupported_position_inset_values() {
    // 覆盖有限支持集合之外的全部代表值。
    for source in ["10", "NaNpx", "calc(10px * 2)", "10em", "calc(50% -8px)"] {
        // 构造单一顶边值文档。
        let document = parse_document(&format!(
            // 保留原值供映射层精确诊断。
            r#"<Text style="top: {source};">定位</Text>"#
        ))
        // 样式语法层不负责长度支持矩阵。
        .expect("top 原始值应完成语法解析");
        // 代码生成必须拒绝不支持值。
        let error = generate_view(&document.root).expect_err("不支持的 top 值必须失败");
        // 诊断必须说明有限支持集合。
        assert!(
            error
                .message
                .contains("只支持 auto、有限 px、百分比或 calc"),
            "{source}: {:?}",
            error.message
        );
    }
}

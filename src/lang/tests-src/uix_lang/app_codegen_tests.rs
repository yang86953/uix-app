// 引入 App 与 View 文档生成入口。
use super::{generate_document_app, generate_document_view, parse_document};

// 使用稳定空白规范生成断言文本。
fn normalized(source: &str) -> String {
    // 解析完整 UIX 文档。
    let document = parse_document(source).expect("测试 UIX 应解析成功");
    // 生成 App builder 并规范化令牌文本。
    generate_document_app(&document)
        // 合法测试输入必须生成成功。
        .expect("测试 App 应生成成功")
        // 转换为稳定令牌字符串。
        .to_string()
}

// 验证四项 App 配置生成现有 builder 且不调用 run。
#[test]
fn generates_existing_app_builder_without_running() {
    // 生成完整第一版配置。
    let tokens = normalized(
        r#"<App title="示例" size="1200 x 800" theme="dark" settings="settings.toml"><Text>内容</Text></App>"#,
    );
    // 入口必须从现有 App 开始。
    assert!(tokens.contains("App :: new"));
    // 标题必须交给既有 builder。
    assert!(tokens.contains("title (\"示例\")"));
    // 尺寸必须在编译期拆成两个整数。
    assert!(tokens.contains("size (1200i32 , 800i32)") || tokens.contains("size (1200 , 800)"));
    // 设置路径必须交给既有 builder。
    assert!(tokens.contains("settings (\"settings.toml\")"));
    // 初始主题必须通过 App 作用域表安装。
    assert!(tokens.contains("__uix_named_themes") && tokens.contains("\"dark\""));
    // App 根必须使用现有根工厂。
    assert!(tokens.contains("root (move ||"));
    // 宏只返回 builder，不能抢占运行生命周期。
    assert!(!tokens.contains(". run"));
}

// 验证同文档主题生成拥有所有权 token 并覆盖内建同名主题。
#[test]
fn generates_document_theme_and_semantic_style_references() {
    // 声明 light 覆盖与主题引用。
    let tokens = normalized(
        r#"@theme light { primaryColor: #336699; backgroundColor: #101820; }
        <App theme="light"><Text style="color: #primaryColor; backgroundColor: #backgroundColor;">主题</Text></App>"#,
    );
    // 自定义主题必须由公开基元与 DesignTokens 生成。
    assert!(tokens.contains("ThemePrimitives :: antd_light"));
    // 作者背景色必须精确覆盖布局背景令牌。
    assert!(tokens.contains("color_bg_layout"));
    // 主色引用必须保留为运行期主题令牌。
    assert!(tokens.contains("PaletteColor :: Primary"));
    // 背景引用必须保留为运行期中性色令牌。
    assert!(tokens.contains("NeutralRole :: BgLayout"));
}

// 验证 uix! 对 App 根给出定向入口诊断。
#[test]
fn view_entry_rejects_app_root_with_directed_diagnostic() {
    // 解析 App 根文档。
    let document = parse_document("<App><Text>内容</Text></App>").expect("文档应解析");
    // View 入口必须拒绝应用根。
    let error = generate_document_view(&document).expect_err("uix! 不应接受 App 根");
    // 诊断必须指向独立宏。
    assert!(error.suggestion.contains("uix_app!"));
}

// 验证根形状、尺寸、属性与主题名称拒绝路径。
#[test]
fn rejects_invalid_app_contracts() {
    // 非 App 根必须失败。
    let wrong_root = parse_document("<Text>内容</Text>").expect("文档应解析");
    // 入口诊断必须说明 App 根。
    assert!(
        generate_document_app(&wrong_root)
            .expect_err("非 App 根必须失败")
            .message
            .contains("<App>")
    );
    // 多直接子根必须失败。
    let multiple = parse_document("<App><Text>一</Text><Text>二</Text></App>").expect("文档应解析");
    // 诊断必须说明唯一子根。
    assert!(
        generate_document_app(&multiple)
            .expect_err("多子根必须失败")
            .message
            .contains("恰有一个")
    );
    // 非正尺寸必须失败。
    let size = parse_document("<App size=\"0x800\"><Text>内容</Text></App>").expect("文档应解析");
    // 诊断必须说明正 i32。
    assert!(
        generate_document_app(&size)
            .expect_err("零尺寸必须失败")
            .message
            .contains("正 i32")
    );
    // 未登记属性必须失败。
    let attribute = parse_document("<App custom_title_bar=\"true\"><Text>内容</Text></App>")
        .expect("文档应解析");
    // 诊断必须保留属性名。
    assert!(
        generate_document_app(&attribute)
            .expect_err("未登记属性必须失败")
            .message
            .contains("custom_title_bar")
    );
    // 未知初始主题必须失败。
    let theme = parse_document("<App theme=\"ocean\"><Text>内容</Text></App>").expect("文档应解析");
    // 诊断必须保留主题名。
    assert!(
        generate_document_app(&theme)
            .expect_err("未知主题必须失败")
            .message
            .contains("ocean")
    );
    // 未知 setTheme 目标必须失败。
    let request = parse_document("<App><Button @click=\"setTheme('ocean')\">切换</Button></App>")
        .expect("文档应解析");
    // 运行期请求必须复用同一名称表。
    assert!(
        generate_document_app(&request)
            .expect_err("未知主题请求必须失败")
            .message
            .contains("ocean")
    );
}

// 验证主题属性白名单拒绝静默丢弃。
#[test]
fn rejects_unknown_theme_property() {
    // 声明未登记主题字段。
    let document = parse_document(
        "@theme ocean { accentColor: #336699; } <App theme=\"ocean\"><Text>内容</Text></App>",
    )
    .expect("文档应解析");
    // 生成必须返回主题属性诊断。
    let error = generate_document_app(&document).expect_err("未知主题属性必须失败");
    // 诊断保留字段名。
    assert!(error.message.contains("accentColor"));
}

// 验证主题生成覆盖运行时设计 token 的全部字段类型与样式引用类型。
#[test]
fn generates_full_design_token_types_and_references() {
    // 声明颜色、字符串、数值、阴影、动效和布尔代表值。
    let tokens = normalized(
        r#"@theme ocean {
            colorText: #223344;
            colorBgRaised: rgba(10,20,30,0.8);
            fontFamily: 'Segoe UI';
            fontSize: 16px;
            padding: 18px;
            borderRadius: 9px;
            motionDurationFast: 0.12;
            motionEasingDefault: 'linear';
            screenMD: 800px;
            boxShadow: 0 2px 8px rgba(0,0,0,0.1), 0 4px 16px rgba(0,0,0,0.08), 0 8px 24px rgba(0,0,0,0.06);
            isDark: true;
        }
        <App theme="ocean"><Text style="color: #colorText; fontSize: #fontSize; padding: #padding;">主题</Text></App>"#,
    );
    // 颜色 token 必须写入精确运行时字段。
    assert!(tokens.contains("color_text") && tokens.contains("color_bg_raised"));
    // 静态字符串与数值 token 必须写入对应字段。
    assert!(tokens.contains("font_family") && tokens.contains("font_size"));
    // 间距、圆角、动效和断点必须进入完整白名单。
    assert!(
        tokens.contains("padding")
            && tokens.contains("border_radius")
            && tokens.contains("motion_duration_fast")
            && tokens.contains("screen_md")
    );
    // 三层阴影与明暗事实必须构造既有运行时类型。
    assert!(tokens.contains("ShadowToken") && tokens.contains("is_dark = true"));
    // 样式颜色引用优先保留既有中性色语义角色。
    assert!(tokens.contains("NeutralRole :: Text"));
    // 样式数值引用必须经构建期有效主题读取，而非固化默认值。
    assert!(
        tokens.contains("uix_effective_build_tokens") && tokens.contains("__uix_tokens . font_size")
            && tokens.contains("__uix_tokens . padding")
    );
}

// 验证未知或类型不兼容的样式 token 在宏展开期失败。
#[test]
fn rejects_invalid_style_token_references() {
    // 未知颜色 token 必须失败。
    let unknown = parse_document("<App><Text style=\"color: #accentColor;\">主题</Text></App>")
        .expect("未知 token 名不影响结构解析");
    // 诊断必须保留未知名称。
    assert!(
        generate_document_app(&unknown)
            .expect_err("未知颜色 token 必须失败")
            .message
            .contains("accentColor")
    );
    // 数值 token 不能用于颜色属性。
    let incompatible = parse_document("<App><Text style=\"color: #padding;\">主题</Text></App>")
        .expect("类型不兼容 token 名不影响结构解析");
    // 诊断必须说明颜色类型边界。
    assert!(
        generate_document_app(&incompatible)
            .expect_err("数值 token 不能用于颜色")
            .message
            .contains("颜色")
    );
}

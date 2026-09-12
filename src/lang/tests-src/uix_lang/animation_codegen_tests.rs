// 引入与公开宏一致的完整文档测试生成入口。
use super::generate_test_document_view as generate;

// 验证完整 animation 简写与六种首批字段生成持久化运行时绑定。
#[test]
fn animation_generates_persistent_bindings_for_supported_properties() {
    // 声明所有支持字段、完整七段简写与文档根消费者。
    let source = r#"
        @keyframes reveal {
            from {
                width: 20px;
                height: 30px;
                borderRadius: 2;
                opacity: 0;
                color: #112233;
                backgroundColor: rgba(10, 20, 30, 0.5);
            }
            to {
                width: 80px;
                height: 90px;
                borderRadius: 12;
                opacity: 1;
                color: #ffffff;
                backgroundColor: #000000;
            }
        }
        animated {
            width: 20px;
            height: 30px;
            borderRadius: 2;
            opacity: 0;
            color: #112233;
            backgroundColor: #0a141e;
            animation: reveal 1s ease-in-out 250ms 2 alternate both;
        }
        <Text class="animated">动画</Text>
    "#;
    // 完整文档生成必须成功。
    let tokens = generate(source).expect("支持矩阵与完整简写应生成运行时绑定");
    // 每个字段都必须复用组件私有状态。
    assert_eq!(tokens.matches("uix_widget_state").count(), 6);
    // 关键帧序列必须交给完整播放配置入口。
    assert_eq!(tokens.matches("animate_keyframes_with").count(), 6);
    // 完整简写必须生成交替方向。
    assert!(tokens.contains("KeyframeDirection :: Alternate"));
    // 完整简写必须生成双向填充。
    assert!(tokens.contains("KeyframeFillMode :: Both"));
    // 尺寸、圆角、透明度与两种颜色必须分别绑定公开 View 方法。
    for method in [
        // 宽度绑定入口。
        "width_animated",
        // 高度绑定入口。
        "height_animated",
        // 圆角绑定入口。
        "radius_animated",
        // 透明度绑定入口。
        "opacity_animated",
        // 前景颜色绑定入口。
        "color_animated",
        // 背景颜色绑定入口。
        "background_color_animated",
    ] {
        // 每种字段都应出现在生成令牌中。
        assert!(
            tokens.contains(method),
            "缺少动画绑定方法 {method}: {tokens}"
        );
    }
}

// 验证 animation 的默认值补齐与显式 none 不建立运行时状态。
#[test]
fn animation_supports_defaults_and_none() {
    // 只给动画名称时使用文档默认播放配置。
    let default_tokens = generate(
        // 单字段关键帧足以观察默认值。
        r#"@keyframes fade { from { opacity: 0; } to { opacity: 1; } } <Text style="animation: fade;" />"#,
    )
    // 默认简写必须成功。
    .expect("animation 名称应补齐默认值");
    // 默认方向必须为正向。
    assert!(default_tokens.contains("KeyframeDirection :: Normal"));
    // 默认填充必须为 none。
    assert!(default_tokens.contains("KeyframeFillMode :: None"));
    // 显式 none 必须作为无状态普通节点生成。
    let none_tokens = generate(r#"<Text style="animation: none;" />"#)
        // none 不需要匹配关键帧声明。
        .expect("animation none 应关闭动画");
    // 关闭动画后不得建立 Animated 状态。
    assert!(!none_tokens.contains("animate_keyframes_with"));
}

// 验证未知关键帧名称返回指向声明缺失的诊断。
#[test]
fn animation_rejects_unknown_keyframes() {
    // 引用不存在的关键帧。
    let error = generate(r#"<Text style="animation: missing 1s;" />"#)
        // 未声明名称不能静默生成。
        .expect_err("未知关键帧必须失败");
    // 诊断必须保留被引用名称。
    assert!(error.message.contains("未声明关键帧 missing"));
}

// 验证关键帧字段矩阵之外的属性被显式拒绝。
#[test]
fn animation_rejects_unsupported_keyframe_property() {
    // transform 尚未拥有 typed Animated 绑定。
    let error = generate(
        // 保留合法关键帧与简写，只让字段超出矩阵。
        r#"@keyframes move { from { transform: translateX(0); } to { transform: translateX(20); } } <Text style="animation: move 1s;" />"#,
    )
    // 不支持字段不得被忽略。
    .expect_err("未登记动画字段必须失败");
    // 诊断必须指出实际字段。
    assert!(error.message.contains("transform"));
    // 诊断必须说明缺少 Animated 绑定。
    assert!(error.message.contains("Animated 绑定"));
}

// 验证伪类中的 animation 被引导到 transition，而不建立竞争生命周期。
#[test]
fn animation_rejects_pseudo_state_lifecycle() {
    // hover 分支尝试创建独立关键帧播放实例。
    let error = generate(
        // 使用组件触发真实伪类降低路径。
        r#"
        @keyframes fade { from { opacity: 0; } to { opacity: 1; } }
        base { opacity: 1; }
        base:hover { animation: fade 1s; }
        <Widget name="AnimatedText"><Text class="base" /></Widget>
        <AnimatedText />
        "#,
    )
    // 伪类动画不得伪装成静态节点动画。
    .expect_err("伪类 animation 必须失败");
    // 诊断原因必须明确拒绝伪类启动播放实例。
    assert!(error.message.contains("状态伪类暂不支持启动 animation"));
    // 修复建议必须明确推荐状态变化契约。
    assert!(error.suggestion.contains("transition"));
}

// 验证关键帧颜色仍明确拒绝主题 token 引用，不静默回退。
#[test]
fn animation_rejects_theme_token_keyframe_colors() {
    // 关键帧与基础样式分别使用主题引用与具体颜色。
    let error = generate(
        r#"
        @keyframes pulse {
            from { color: #colorText; }
            to { color: #ffffff; }
        }
        pulsing {
            color: #112233;
            animation: pulse 1s;
        }
        <Text class="pulsing" />
        "#,
    )
    // 关键帧没有逐帧主题解析通道，必须在编译期拒绝。
    .expect_err("关键帧颜色主题 token 引用必须失败");
    // 诊断必须点明主题 token 限制。
    assert!(error.message.contains("主题 token"));
}

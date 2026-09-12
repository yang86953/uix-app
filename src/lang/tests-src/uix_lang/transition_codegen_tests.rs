// 引入与公开宏一致的完整文档测试生成入口。
use super::generate_test_document_view as generate;

// 验证 all 对 hover 的首批字段差异生成最终目标比较装饰。
#[test]
fn transition_all_generates_persistent_hover_retarget() {
    // 基础样式提供具体颜色，hover 同时改变绘制字段。
    let tokens = generate(
        // 声明完整简写与自动 hover 状态。
        r#"
        card {
          opacity: 1;
          backgroundColor: rgb(10, 20, 30);
          transition: all 250ms ease-in-out 50ms;
        }
        card:hover {
          opacity: 0.5;
          backgroundColor: rgb(30, 40, 50);
        }
        <Widget name="Card"><Text class="card">卡片</Text></Widget>
        <Card />
        "#,
    )
    // 合法状态差异必须生成。
    .expect("transition all 应包裹 hover 最终目标");
    // transition 必须复用组件私有状态。
    assert!(tokens.contains("uix_transition_state"));
    // 每次 reconcile 必须比较最终目标并原位重定向。
    assert!(tokens.contains("uix_apply_transition"));
    // all 应收集透明度字段。
    assert!(tokens.contains("UixTransitionProperty :: Opacity"));
    // all 应收集背景颜色字段。
    assert!(tokens.contains("UixTransitionProperty :: BackgroundColor"));
    // transition 必须位于 hover 目标样式之外。
    let hover = tokens
        .find("__uix_pseudo_hover_current")
        .expect("应生成 hover 事实");
    // 查找最终 transition 目标变量。
    let transition = tokens
        .find("__uix_transition_target_view")
        .expect("应生成 transition 目标");
    // 外层 transition 先声明目标局部，其右值包含完整 hover 分支。
    assert!(transition < hover);
    // 真正的目标比较调用必须位于 hover 分支令牌之后。
    let apply = tokens
        // 使用最后一次出现避开导入路径片段。
        .rfind("uix_apply_transition")
        // transition 生成必须包含运行时调用。
        .expect("应生成目标比较调用");
    // hover 目标完成后才可执行 retarget。
    assert!(hover < apply);
}

// 验证具体字段可平滑处理 setStyle 完整分支切换。
#[test]
fn transition_specific_property_wraps_set_style_targets() {
    // 原始类拥有固定宽度与过渡配置，目标类拥有新宽度。
    let tokens = generate(
        // 点击事件切换到闭合目标类。
        r#"
        compact { width: 20px; transition: width 1s linear 0s; }
        expanded { width: 80px; }
        <Widget name="Resizable">
          <Text class="compact" @click="setStyle('expanded')">调整</Text>
        </Widget>
        <Resizable />
        "#,
    )
    // 两个完整分支都提供固定宽度时必须成功。
    .expect("width transition 应包裹 setStyle 分支");
    // 动态样式枚举必须保留。
    assert!(tokens.contains("__uix_dynamic_style_current"));
    // transition 必须选择单一宽度字段。
    assert!(tokens.contains("UixTransitionProperty :: Width"));
    // 播放配置必须使用线性缓动。
    assert!(tokens.contains("Easing :: linear"));
}

// 验证 opacity 可从 Style 默认值开始过渡而无需显式基础声明。
#[test]
fn transition_uses_animatable_style_defaults() {
    // 基础类只声明 transition，hover 声明透明度目标。
    let tokens = generate(
        // 默认透明度一是可插值基础值。
        r#"fade { transition: opacity 200ms ease-out; } fade:hover { opacity: 0.25; } <Text class="fade">淡出</Text>"#,
    )
    // 文档根必须自动建立生命周期作用域。
    .expect("默认 opacity 应支持 transition");
    // 生成文档根组件状态作用域。
    assert!(tokens.contains("__uix_document_scope"));
    // 只选择透明度字段。
    assert!(tokens.contains("UixTransitionProperty :: Opacity"));
}

// 验证固定尺寸 transition 要求基础与每个动态完整分支都提供目标。
#[test]
fn transition_rejects_missing_fixed_dimension_targets() {
    // 基础样式没有固定宽度。
    let missing_base = generate(
        // hover 单独添加宽度无法为首次挂载建立 typed Animated。
        r#"grow { transition: width 200ms; } grow:hover { width: 80px; } <Text class="grow">增长</Text>"#,
    )
    // 缺失基础值必须失败。
    .expect_err("width transition 缺失基础值必须失败");
    // 诊断必须指出具体字段。
    assert!(missing_base.message.contains("width 缺少具体基础值"));
    // setStyle 目标类没有固定宽度。
    let missing_target = generate(
        // 原始类提供宽度但目标类只改变颜色。
        r#"
        compact { width: 20px; transition: width 200ms; }
        colored { color: rgb(10, 20, 30); }
        <Widget name="Resizable"><Text class="compact" @click="setStyle('colored')" /></Widget>
        <Resizable />
        "#,
    )
    // 任一完整动态分支缺值都必须失败。
    .expect_err("setStyle 目标缺失固定宽度必须失败");
    // 诊断必须保留字段名称。
    assert!(missing_target.message.contains("width 缺少具体基础值"));
}

// 验证状态分支不能改变 transition 配置。
#[test]
fn transition_rejects_pseudo_configuration_changes() {
    // hover 分支错误声明播放配置。
    let error = generate(
        // 基础类只提供颜色目标。
        r#"card { backgroundColor: rgb(10,20,30); } card:hover { backgroundColor: rgb(30,40,50); transition: backgroundColor 200ms; } <Text class="card" />"#,
    )
    // 配置必须归属基础样式。
    .expect_err("伪类 transition 配置必须失败");
    // 诊断必须说明配置所有权。
    assert!(error.message.contains("状态伪类不能改变 transition"));
}

// 验证同一节点不能让 animation 与 transition 竞争同一 Animated 值。
#[test]
fn transition_rejects_animation_competition() {
    // 同一基础样式同时声明两类播放器。
    let error = generate(
        // 两种声明都以 opacity 为目标。
        r#"
        @keyframes pulse { from { opacity: 0; } to { opacity: 1; } }
        mixed { opacity: 1; animation: pulse 1s; transition: opacity 200ms; }
        <Text class="mixed" />
        "#,
    )
    // 竞争播放器必须在编译期失败。
    .expect_err("animation 与 transition 组合必须失败");
    // 诊断必须点明两种声明。
    assert!(error.message.contains("animation 与 transition"));
}

// 验证未知字段与非法时间返回定向诊断。
#[test]
fn transition_rejects_invalid_syntax_and_values() {
    // 未登记字段不能进入首批矩阵。
    let unknown = generate(r#"<Text style="transition: transform 1s;" />"#)
        // transform 尚未拥有运行时 typed slot。
        .expect_err("未知 transition 字段必须失败");
    // 诊断必须保留字段名。
    assert!(unknown.message.contains("transform"));
    // 无单位时间必须失败。
    let time = generate(r#"<Text style="opacity: 1; transition: opacity 200;" />"#)
        // 时长需要 s 或 ms。
        .expect_err("无单位时间必须失败");
    // 诊断必须说明单位。
    assert!(time.message.contains("缺少 s 或 ms"));
    // 未登记主题名称仍必须返回定向诊断。
    let unknown_token =
        generate(r#"<Text style="color: #notAToken; transition: color 200ms;" />"#)
            // 未知 token 不能静默通过。
            .expect_err("未登记主题 token 必须失败");
    // 诊断必须保留 token 名称。
    assert!(unknown_token.message.contains("notAToken"));
}

// 验证语义主题 token 颜色可作为 transition 目标。
#[test]
fn transition_accepts_semantic_theme_token_color_targets() {
    // 基础与 hover 都使用语义主题色，配置固定时长过渡。
    let tokens = generate(
        // 语义调色板与中性角色覆盖亮暗切换场景。
        r#"
        themed {
          color: #colorText;
          backgroundColor: #colorPrimaryBg;
          transition: all 200ms ease-out;
        }
        themed:hover {
          color: #colorPrimary;
          backgroundColor: #colorPrimaryHover;
        }
        <Text class="themed">主题过渡</Text>
        "#,
    )
    // 主题 token 目标必须生成。
    .expect("语义主题颜色 transition 应生成");
    // 两个颜色字段都必须进入闭合过渡集合。
    assert!(tokens.contains("UixTransitionProperty :: Color"));
    assert!(tokens.contains("UixTransitionProperty :: BackgroundColor"));
    // 目标样式保留语义色值，由运行时按当前有效主题解析。
    assert!(tokens.contains("ColorValue :: palette"));
    assert!(tokens.contains("ColorValue :: neutral"));
}

// 验证非语义颜色 token 引用经构建期有效主题解析。
#[test]
fn transition_non_semantic_token_reads_effective_build_tokens() {
    // colorBgRaised 没有语义枚举角色，走构建期有效主题读取。
    let tokens = generate(
        // 基础样式声明非语义颜色 token 与过渡。
        r#"
        raised {
          backgroundColor: #colorBgRaised;
          transition: backgroundColor 300ms;
        }
        <Text class="raised" />
        "#,
    )
    // 非语义 token 引用必须生成。
    .expect("非语义颜色 token transition 应生成");
    // 生成代码必须经 uix_effective_build_tokens 读取窗口或局部主题。
    assert!(tokens.contains("uix_effective_build_tokens"));
}

// 导入应用、主题、视图与 RichText 公开门面。
use uix::prelude::*;

// 构造带稳定标题、边框和宽度的单个验收卡片。
fn acceptance_card(title: &'static str, width: f32, segments: Vec<RichTextSegment>) -> ViewNode {
    // 卡片标题与待验收 RichText 使用同一列布局。
    column_fit((
        // 标明当前卡片覆盖的契约边界。
        label(title),
        // 使用真实 RichText 组件渲染指定段列表。
        embed(
            // 构造富文本组件。
            RichText::new()
                // 安装当前验收内容。
                .content(segments)
                // 保留真实选择能力以覆盖零宽分隔线交互边界。
                .selectable(true)
                // 使用清晰的默认字号便于截图检查垂直居中。
                .font_size(16.0),
        )
        // RichText 填满卡片内部可用宽度。
        .width(width - 36.0),
    ))
    // 标题与内容之间保留稳定间距。
    .gap(12.0)
    // 卡片四周提供可比较的内容边界。
    .padding(18.0)
    // 固定宽度以同时核对宽窄约束。
    .width(width)
    // 使用主题容器背景验证明暗主题适配。
    .bg(ColorValue::neutral(NeutralRole::BgContainer))
    // 使用主题次级边框明确十二像素线端留白。
    .border(1.0, ColorValue::neutral(NeutralRole::BorderSecondary))
    // 采用稳定圆角避免与主题线混淆。
    .radius(8.0)
}

// 构造正文、显式换行与主题分隔线混排的段列表。
fn mixed_segments() -> Vec<RichTextSegment> {
    // 返回直接建模的段序列，避免视觉矩阵依赖解析歧义。
    vec![
        // 分隔线上方的正文使用默认 RichText 样式。
        RichTextSegment::Text {
            // 提供可见的上方正文。
            content: "分隔线前：正文保持正常行高与主题文字色".into(),
            // 沿用默认样式。
            style: RichTextStyle::default(),
        },
        // 正文后进入下一布局行。
        RichTextSegment::NewLine,
        // 插入零字符宽度的主题分隔线。
        RichTextSegment::ThematicBreak,
        // 分隔线后的源码换行不得额外制造空白行。
        RichTextSegment::NewLine,
        // 分隔线下方正文用于核对垂直节奏。
        RichTextSegment::Text {
            // 提供可见的下方正文。
            content: "分隔线后：左右各保留 12px，线条在行盒内垂直居中".into(),
            // 沿用默认样式。
            style: RichTextStyle::default(),
        },
    ]
}

// 构造完整主题分隔线真窗验收视图。
fn acceptance_view() -> ViewNode {
    // 使用两列矩阵并排展示宽度与解析边界。
    let matrix = row((
        // 左列覆盖宽卡片下的解析和混排行为。
        column_fit((
            // 三种合法标记必须都渲染为主题线。
            acceptance_card(
                // 标明输入来源和预期数量。
                "A. Markdown 标记解析：--- / *** / _ _ _（应显示三条线）",
                // 使用宽内容区核对两侧十二像素留白。
                520.0,
                // 通过公开解析器生成真实段模型。
                parse_rich_text("---\n***\n_ _ _"),
            ),
            // 显式段混排验证独立行盒与正文节奏。
            acceptance_card(
                // 标明直接段模型覆盖范围。
                "B. 正文 + ThematicBreak + 正文",
                // 与上方卡片保持相同宽度便于比较。
                520.0,
                // 使用显式段序列。
                mixed_segments(),
            ),
        ))
        // 左列内部保持稳定间距。
        .gap(18.0),
        // 右列覆盖 Setext 优先级与窄宽边界。
        column_fit((
            // Setext 标记行优先形成标题，后续孤立标记才形成主题线。
            acceptance_card(
                // 标明预期只出现一条主题线。
                "C. Setext 优先级：标题下划线不应成为分隔线（应仅一条线）",
                // 使用较窄内容区暴露错误换行或溢出。
                420.0,
                // 复用公开 Markdown 解析入口。
                parse_rich_text("Setext 二级标题\n---\n___"),
            ),
            // 窄卡片验证线宽扣除左右内边距后仍保持非负。
            acceptance_card(
                // 标明窄宽约束和目标留白。
                "D. 窄宽约束：线条仍应左右各留 12px",
                // 固定为窄内容区。
                420.0,
                // 使用混合合法标记覆盖块级识别。
                parse_rich_text("- * _ -"),
            ),
        ))
        // 右列内部保持稳定间距。
        .gap(18.0),
    ))
    // 两列之间保留稳定间距。
    .gap(20.0);
    // 页面使用标题、说明和矩阵的单一布局树。
    column((
        // 提供截图内可读的验收主题。
        label("UIX RichText Markdown 主题分隔线真窗验收").font_size(24.0),
        // 说明视觉判定标准。
        label("判定：1px 主题最淡正文色；行盒内垂直居中；组件内容区左右各留 12px；明暗主题均清晰。"),
        // 安装四项验收矩阵。
        matrix,
    ))
    // 页面各区域保持稳定间距。
    .gap(20.0)
    // 为窗口边界提供统一留白。
    .padding(28.0)
    // 使用主题布局背景突出容器色与分隔线色。
    .bg(ColorValue::neutral(NeutralRole::BgLayout))
    // 填满窗口客户区。
    .flex_grow(1.0)
}

// 启动支持明暗主题切换参数的独立验收应用。
fn main() {
    // 从稳定命令行参数读取暗色主题请求。
    let dark = std::env::args().any(|argument| argument == "--dark");
    // 根据验收模式选择窗口标题。
    let title = if dark {
        // 暗色窗口使用独立标题便于精确定位。
        "UIX RichText 主题分隔线视觉验收（暗色）"
    } else {
        // 亮色窗口使用独立标题便于精确定位。
        "UIX RichText 主题分隔线视觉验收（亮色）"
    };
    // 根据验收模式选择完整 Ant Design 主题。
    let theme = if dark {
        // 暗色验收使用权威暗色令牌。
        Theme::antd_dark()
    } else {
        // 亮色验收使用权威亮色令牌。
        Theme::antd_light()
    };
    // 使用公开 App 组合根启动唯一 UI 管线。
    App::new()
        // 设置可被自动化精确定位的窗口标题。
        .title(title)
        // 固定逻辑尺寸以容纳完整四项矩阵。
        .size(1040, 680)
        // 明确选择 Windows D3D11 参考后端。
        .graphics_backend(GraphicsBackend::Direct3D11)
        // 安装当前明暗主题。
        .theme(theme)
        // 根工厂构造真实 RichText 验收视图。
        .root(acceptance_view)
        // 进入现有原生窗口事件循环。
        .run();
}

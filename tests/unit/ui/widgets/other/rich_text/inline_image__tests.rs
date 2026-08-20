    // 引入当前图片组件和 RichText 布局入口。
    use super::*;
    // 引入不产生真实像素副作用的测试画布。
    use crate::draw::backend::cpu::noop_canvas_2d::NoopCanvas2D;
    // 引入 DisplayList 与底层绘制上下文观测入口。
    use crate::draw::painting::{DisplayList, PaintContext as DrawPaintContext, PaintOp};
    // 引入字体服务创建最小绘制依赖。
    use crate::draw::resources::font::font_service::FontService;
    // 引入 UI 主题绘制上下文。
    use crate::ui::widget_runtime::paint_context::PaintContext as UiPaintContext;
    // 引入标准浅色主题令牌。
    use crate::ui::theme::Theme;
    // 引入公开段与样式模型。
    use crate::ui::widgets::other::rich_text::{
        LayoutGlyphKind, RichTextPalette, RichTextSegment, RichTextStyle,
        layout_rich_text_with_images,
    };

    // 验证固有比例和单轴覆盖解析为稳定几何。
    #[test]
    fn resolves_intrinsic_and_single_axis_override_sizes() {
        // 构造宽高比二比一的固有尺寸状态。
        let state = InlineImageState::with_intrinsic_size(Size::new(60.0, 30.0));
        // 无覆盖时直接使用固有逻辑尺寸。
        assert_eq!(
            resolved_size(Some(&state), None, None, 18.0),
            Size::new(60.0, 30.0)
        );
        // 仅宽度覆盖时保持固有比例。
        assert_eq!(
            resolved_size(Some(&state), Some(40.0), None, 18.0),
            Size::new(40.0, 20.0)
        );
        // 仅高度覆盖时保持固有比例。
        assert_eq!(
            resolved_size(Some(&state), None, Some(15.0), 18.0),
            Size::new(30.0, 15.0)
        );
    }

    // 验证图片作为不可拆分原子折行并覆盖完整 alt 逻辑跨度。
    #[test]
    fn layout_wraps_atomic_image_and_preserves_alt_span() {
        // 构造正文后跟固定几何图片的段列表。
        let segments = vec![
            // 第一行普通文本。
            RichTextSegment::Text {
                // 使用单字符正文。
                content: "a".into(),
                // 使用默认样式。
                style: RichTextStyle::default(),
            },
            // 宽于剩余空间的图片原子。
            RichTextSegment::Image {
                // 测试不实际加载资源。
                src: "assets/test.png".into(),
                // 图片逻辑跨度为两个字符。
                alt: "图片".into(),
                // 固定四十像素宽度。
                width: Some(40.0),
                // 固定二十像素高度。
                height: Some(20.0),
                // 默认保持比例。
                fit: true,
                // 不使用圆角。
                radius: None,
            },
        ];
        // 使用三十像素行宽迫使图片整体换到第二行。
        let (lines, height, width) = layout_rich_text_with_images(
            // 传递公开段。
            &segments,
            // 设置窄行宽。
            30.0,
            // 设置十二像素默认字号。
            12.0,
            // 从稳定测试颜色派生无主题调色板。
            RichTextPalette::estimated(crate::draw::Color::black()),
            // 不提供运行时固有尺寸。
            &InlineImageStates::new(),
        );
        // 文本和图片必须形成两个视觉行。
        assert_eq!(lines.len(), 2);
        // 第二行只包含一个图片原子。
        let image = &lines[1].glyphs[0];
        // 绘制语义必须明确为内联图片。
        assert_eq!(image.kind, LayoutGlyphKind::InlineImage);
        // 图片 alt 从前一字符之后开始。
        assert_eq!(image.global_char_idx, 1);
        // 图片原子覆盖完整两个 alt 字符。
        assert_eq!(image.source_char_len, 2);
        // 第一行十八像素，第二行由二十像素图片撑开。
        assert!((height - 38.0).abs() < f32::EPSILON);
        // 原子自然宽度可以超过约束并如实报告。
        assert!((width - 40.0).abs() < f32::EPSILON);
        // 无界宽度应让正文与图片保持在同一视觉行。
        let (unbounded_lines, _, _) = layout_rich_text_with_images(
            // 复用相同公开段。
            &segments,
            // 使用与普通文本一致的无界宽度语义。
            f32::INFINITY,
            // 保持默认字号不变。
            12.0,
            // 保持派生测试调色板不变。
            RichTextPalette::estimated(crate::draw::Color::black()),
            // 不提供运行时固有尺寸。
            &InlineImageStates::new(),
        );
        // 正无穷约束不得导致图片额外换行。
        assert_eq!(unbounded_lines.len(), 1);
    }

    // 验证待加载图片的占位事实完整进入可回放 DisplayList。
    #[test]
    fn pending_image_records_placeholder_display_list() {
        // 构造固定二十像素几何的待加载图片段。
        let segments = vec![RichTextSegment::Image {
            // 测试不启动真实文件加载。
            src: "assets/pending.png".into(),
            // 保留可访问替代文本，但小尺寸占位不绘制文字。
            alt: "封面".into(),
            // 固定图片宽度。
            width: Some(20.0),
            // 固定图片高度。
            height: Some(20.0),
            // 默认保持比例。
            fit: true,
            // 使用四像素圆角验证样式进入绘制命令。
            radius: Some(4.0),
        }];
        // 生成只含一个图片原子的共享布局。
        let (lines, _, _) = layout_rich_text_with_images(
            // 传递公开段。
            &segments,
            // 提供足够宽度避免换行。
            100.0,
            // 使用十二像素默认字号。
            12.0,
            // 从稳定测试颜色派生无主题调色板。
            RichTextPalette::estimated(crate::draw::Color::black()),
            // 空状态代表资源仍待加载。
            &InlineImageStates::new(),
        );
        // 创建不会产生真实像素写入的画布。
        let mut canvas = NoopCanvas2D;
        // 创建最小字体服务依赖。
        let font_service = FontService::new();
        // 创建空图片服务依赖。
        let image_service = crate::draw::resources::image::ImageService::new();
        // 创建底层绘制上下文。
        let mut draw_context = DrawPaintContext::new_for_test(
            // 注入空画布。
            &mut canvas,
            // 使用默认字体句柄。
            crate::draw::FontHandle::new(0),
            // 注入字体服务。
            &font_service,
            // 注入图片服务。
            &image_service,
            // 使用标准九十六 DPI。
            96.0,
            // 使用一倍设备比例。
            1.0,
            // 使用 Windows 默认向下坐标。
            crate::draw::geometry::spatial::Orientation::YDown,
            // 设置正尺寸测试表面宽度。
            100,
            // 设置正尺寸测试表面高度。
            100,
        );
        // 创建接收图片绘制事实的 DisplayList。
        let mut list = DisplayList::new();
        // 创建标准浅色主题令牌。
        let tokens = Theme::antd_light().tokens_arc();
        // 在底层统一录制作用域内执行 UI 图片绘制。
        draw_context.with_recorder(&mut list, |draw_context| {
            // 包装 UI 主题绘制上下文。
            let mut ui_context = UiPaintContext::new(draw_context, tokens);
            // 绘制待加载图片占位。
            super::draw(
                // 传递 UI 绘制上下文。
                &mut ui_context,
                // 传递公开图片段。
                &segments,
                // 空状态保持待加载分支。
                &InlineImageStates::new(),
                // 传递共享布局结果。
                &lines,
                // 使用原点组件范围。
                crate::core::Rect::new(0.0, 0.0, 100.0, 100.0),
            );
        });
        // 待加载占位必须录制背景和边框两条事实。
        assert_eq!(list.len(), 2);
        // 第一条事实必须是带四像素圆角的占位背景。
        assert!(matches!(
            // 读取只读操作列表。
            &list.ops()[0],
            // 匹配二十像素矩形和四像素圆角。
            PaintOp::FillRect { rect, radius: Some(radius), .. }
                // 验证图片原子几何完整进入命令。
                if *rect == crate::core::Rect::new(0.0, 0.0, 20.0, 20.0)
                    // 验证圆角样式保持一致。
                    && radius.tl == 4.0
        ));
        // 第二条事实必须是同几何的一像素占位边框。
        assert!(matches!(
            // 读取第二条绘制操作。
            &list.ops()[1],
            // 匹配统一边框宽度与矩形。
            PaintOp::StrokeRect { rect, line_width, .. }
                // 验证边框覆盖完整图片原子。
                if *rect == crate::core::Rect::new(0.0, 0.0, 20.0, 20.0)
                    // 验证边框宽度为一个逻辑像素。
                    && *line_width == 1.0
        ));
    }

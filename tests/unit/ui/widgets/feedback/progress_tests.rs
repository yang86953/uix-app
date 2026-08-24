// 引入进度条运行时公开契约。
use super::{ProgressBar, ProgressMode, ProgressNormalizationReason, ProgressType};
// 引入动画 trait 以直接验证帧注册返回值。
use crate::ui::widget_runtime::traits::WidgetAnimation;
// 引入快照枚举以核对自动化可观察字段。
use crate::ui::SnapshotFields;
// 引入公开 View 构建入口以验证组件自己的 UIX 声明壳。
use crate::ui::view::View;

// 圆形进度必须提交单一共享弧路径，不再产生 64 条独立线段。
#[test]
fn circle_progress_records_one_arc_without_line_segments() {
    use crate::draw::backend::cpu::noop_canvas_2d::NoopCanvas2D;
    use crate::draw::painting::{DisplayList, PaintContext as DrawPaintContext, PaintOp};
    use crate::draw::resources::font::font_service::FontService;
    use crate::ui::Theme;
    use crate::ui::widget_runtime::paint_context::PaintContext as UiPaintContext;
    use crate::ui::widget_runtime::traits::WidgetRender;

    let progress = ProgressBar::new().progress(0.5).circle();
    let tree = crate::ui::WidgetTree::new();
    let mut canvas = NoopCanvas2D;
    let font_service = FontService::new();
    let image_service = crate::draw::resources::image::ImageService::new();
    let mut draw_context = DrawPaintContext::new_for_test(
        &mut canvas,
        crate::draw::FontHandle::new(0),
        &font_service,
        &image_service,
        96.0,
        1.0,
        crate::draw::geometry::spatial::Orientation::YDown,
        96,
        96,
    );
    let mut list = DisplayList::new();
    let tokens = Theme::antd_light().tokens_arc();
    draw_context.with_recorder(&mut list, |draw_context| {
        let mut ui_context = UiPaintContext::new(draw_context, tokens);
        WidgetRender::render(
            &progress,
            crate::core::Rect::new(0.0, 0.0, 96.0, 96.0),
            &mut ui_context,
            &tree,
        );
    });

    let arc_count = list
        .ops()
        .iter()
        .filter(|op| matches!(op, PaintOp::StrokePath { .. }))
        .count();
    let line_count = list
        .ops()
        .iter()
        .filter(|op| matches!(op, PaintOp::DrawLine { .. }))
        .count();
    assert_eq!(arc_count, 1);
    assert_eq!(line_count, 0);
}

// 验证 UIX 视觉声明保持原进度条单叶节点、fraction 与作者形态。
#[test]
fn uix_shell_preserves_single_progress_kernel_leaf() {
    // 通过公开 View 入口构建已配置进度条。
    let node = View::build(ProgressBar::new().progress(0.4).circle());
    // 声明壳不得引入包装子节点。
    assert!(node.children.is_empty());
    // 运行时动态类型必须仍是原 ProgressBar。
    assert!(node.widget.as_any().is::<ProgressBar>());
    // 快照必须保留配置后的 fraction 与形态。
    let SnapshotFields::ProgressBar {
        progress,
        progress_type,
        ..
    } = node.widget.snapshot_fields()
    else {
        panic!("ProgressBar 必须生成专属快照");
    };
    assert_eq!(progress, 0.4);
    assert_eq!(progress_type, ProgressType::Circle);
    let kernel = node
        .widget
        .as_any()
        .downcast_ref::<ProgressBar>()
        .expect("UIX 根必须保留 ProgressBar 内核");
    assert_eq!(kernel.visual_contract_for_test(), (200.0, 8.0, 0.75));
}

// 验证 UIX 默认尺寸与圆角只填充未显式设置的字段，并共享视觉表。
#[test]
fn uix_visual_defaults_keep_authored_dimensions_and_share_configuration() {
    let first = View::build(ProgressBar::new().size(320.0, 12.0).round(false));
    let second = View::build(ProgressBar::new());
    let first = first
        .widget
        .as_any()
        .downcast_ref::<ProgressBar>()
        .expect("第一个 UIX 根必须保留 ProgressBar 内核");
    let second = second
        .widget
        .as_any()
        .downcast_ref::<ProgressBar>()
        .expect("第二个 UIX 根必须保留 ProgressBar 内核");
    let SnapshotFields::ProgressBar {
        width: first_width,
        height: first_height,
        round: first_round,
        ..
    } = first.snapshot_fields()
    else {
        panic!("第一个 ProgressBar 必须生成专属快照");
    };
    let SnapshotFields::ProgressBar {
        width: second_width,
        height: second_height,
        round: second_round,
        ..
    } = second.snapshot_fields()
    else {
        panic!("第二个 ProgressBar 必须生成专属快照");
    };
    assert_eq!(
        (first_width, first_height, first_round),
        (320.0, 12.0, false)
    );
    assert_eq!(
        (second_width, second_height, second_round),
        (200.0, 8.0, true)
    );
    assert!(first.shares_visual_with_for_test(second));
}

// 验证四类 fraction 输入共享唯一归一值与原因。
#[test]
fn progress_normalization_is_observable() {
    // 下界越界必须归一化为零。
    let below = ProgressBar::new().progress(-0.25);
    // 公开只读状态必须报告下界原因。
    assert!(below.input_normalized());
    // 原因枚举必须稳定可比较。
    assert_eq!(
        below.normalization_reason(),
        Some(ProgressNormalizationReason::BelowRange)
    );
    // 快照必须与组件公开观察值同源。
    let SnapshotFields::ProgressBar {
        progress,
        input_normalized,
        normalization_reason,
        ..
    } = below.snapshot_fields()
    else {
        // 类型分派错误时立即失败。
        panic!("ProgressBar 必须生成专属快照");
    };
    // 绘制 fraction 必须使用安全下界。
    assert_eq!(progress, 0.0);
    // 快照必须标记发生归一化。
    assert!(input_normalized);
    // 快照必须保留下界原因。
    assert_eq!(
        normalization_reason,
        Some(ProgressNormalizationReason::BelowRange)
    );

    // 上界越界必须归一化为一。
    let above = ProgressBar::new().progress(1.25);
    // 上界原因必须可观察。
    assert_eq!(
        above.normalization_reason(),
        Some(ProgressNormalizationReason::AboveRange)
    );
    // 非有限输入必须回退零值。
    let non_finite = ProgressBar::new().progress(f32::NAN);
    // 非有限原因必须区别于范围越界。
    assert_eq!(
        non_finite.normalization_reason(),
        Some(ProgressNormalizationReason::NonFinite)
    );
    // 合法输入不得产生归一化状态。
    let valid = ProgressBar::new().progress(0.4);
    // 合法 fraction 保持无原因。
    assert_eq!(valid.normalization_reason(), None);
}

// 验证动态模式选择保留 fraction 且只有不确定模式续订动画。
#[test]
fn indeterminate_mode_preserves_fraction_and_controls_animation() {
    // 先建立合法确定进度，再显式选择不确定模式。
    let mut progress = ProgressBar::new()
        // 保存恢复确定模式所需 fraction。
        .progress(0.4)
        // 动态布尔真选择不确定模式。
        .indeterminate_when(true);
    // 不确定模式必须请求下一帧。
    assert!(WidgetAnimation::update_animation(&mut progress, 0.1));
    // 动画相位必须由组件自身推进。
    assert!(progress.animation_phase() > 0.0);

    // reconcile 到动态布尔假必须恢复同一 fraction。
    progress.sync_from(
        ProgressBar::new()
            // 新声明提供相同 fraction。
            .progress(0.4)
            // 动态布尔假选择确定模式。
            .indeterminate_when(false),
    );
    // 确定模式不得继续请求动画帧。
    assert!(!WidgetAnimation::update_animation(&mut progress, 0.1));
    // 快照必须恢复确定模式与保留 fraction。
    let SnapshotFields::ProgressBar { progress, mode, .. } = progress.snapshot_fields() else {
        // 类型分派错误时立即失败。
        panic!("ProgressBar 必须生成专属快照");
    };
    // fraction 必须跨模式切换保持不变。
    assert_eq!(progress, 0.4);
    // 确定模式必须消费同一 fraction。
    assert_eq!(mode, ProgressMode::Determinate(0.4));
}

// 验证圆形不确定模式不发布确定进度无障碍值。
#[test]
fn circle_indeterminate_accessibility_has_no_numeric_value() {
    // 构造圆形不确定进度条。
    let progress = ProgressBar::new()
        // 切换圆形呈现。
        .circle()
        // 显式选择不确定模式。
        .indeterminate_when(true);
    // 取得组件快照。
    let snapshot = progress.snapshot_fields();
    // 快照必须保留圆形身份。
    let SnapshotFields::ProgressBar { progress_type, .. } = &snapshot else {
        // 类型分派错误时立即失败。
        panic!("ProgressBar 必须生成专属快照");
    };
    // 形态必须是圆形。
    assert_eq!(*progress_type, ProgressType::Circle);
    // 无障碍投影由快照统一生成。
    let accessibility = snapshot.accessibility();
    // 不确定模式不得发布 value_now。
    assert_eq!(accessibility.state.value_now, None);
    // 不确定模式不得发布确定范围下界。
    assert_eq!(accessibility.state.value_min, None);
    // 不确定模式不得发布确定范围上界。
    assert_eq!(accessibility.state.value_max, None);
}

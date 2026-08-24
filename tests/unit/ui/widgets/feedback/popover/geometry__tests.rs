// 复用被测模块中的组件与几何辅助函数。
use super::*;
// 引入事件行为 trait 以驱动焦点与键盘事件。
use crate::ui::widget_runtime::traits::EventHandler;
// 引入组件渲染 trait 以验证浮层登记契约。
use crate::ui::widget_runtime::traits::WidgetRender;
// 引入稳定测试组件身份。
use crate::core::WidgetId;

// 标记表面缩放时缓存几何必须失效的回归契约。
#[test]
// 触发器不动时，缩小表面也必须重新约束气泡。
fn popup_cache_does_not_survive_surface_resize() {
    // 构造向右展开、会在窄表面内水平收敛的气泡。
    let mut popover = Popover::new("contract").placement(PopoverPlacement::Right);
    // 打开气泡以覆盖真实的浮层登记路径。
    popover.open();
    // 固定触发器位置以隔离表面尺寸这一项变量。
    let frame = Rect::new(100.0, 40.0, 40.0, 20.0);
    // 大表面允许气泡保持作者指定的右侧位置。
    let large_surface = Rect::new(0.0, 0.0, 400.0, 240.0);
    // 小表面要求气泡向左约束到可见范围内。
    let small_surface = Rect::new(0.0, 0.0, 240.0, 160.0);
    // 计算大表面下已绘制并写入缓存的气泡矩形。
    let cached = resolve_popover_geometry(
        // 传入固定触发器矩形。
        frame,
        // 传入初始大表面。
        large_surface,
        // 沿用组件作者指定位置。
        PopoverPlacement::Right,
        // 保留箭头间距。
        true,
        // 使用组件 UIX 默认视觉表。
        &DEFAULT_POPOVER_VISUAL,
    )
    // 只取最终气泡矩形。
    .popup;
    // 模拟上一帧绘制留下的触发器缓存键。
    popover.last_frame.set(frame);
    // 模拟上一帧绘制留下的相对气泡缓存。
    popover.popup_rect.set(Rect::new(
        // 保存相对触发器的横坐标。
        cached.x - frame.x,
        // 保存相对触发器的纵坐标。
        cached.y - frame.y,
        // 保存缓存宽度。
        cached.w,
        // 保存缓存高度。
        cached.h,
    ));
    // 先记录与缓存一致的大表面。
    popover.surface_rect.set(large_surface);
    // 通过布局阶段的新能力注入缩小后的当前表面。
    let overlay = crate::ui::widget_runtime::traits::WidgetRender::overlay_entry_for_surface(
        // 传入被测气泡组件。
        &popover,
        // 使用稳定的测试组件标识。
        crate::core::WidgetId::new(1),
        // 保持触发器 frame 不变。
        frame,
        // 仅改变当前逻辑表面。
        small_surface,
    );
    // 打开状态必须生成使用新表面的浮层登记。
    assert!(overlay.is_some());
    // 以新表面直接计算当前帧应使用的几何。
    let expected = resolve_popover_geometry(
        // 触发器保持不变。
        frame,
        // 表面改为缩小后的尺寸。
        small_surface,
        // 位置配置保持不变。
        PopoverPlacement::Right,
        // 箭头配置保持不变。
        true,
        // UIX 视觉配置保持不变。
        &DEFAULT_POPOVER_VISUAL,
    )
    // 只比较最终气泡矩形。
    .popup;

    // 缓存读取必须与新表面下的绘制几何一致。
    assert_eq!(popover.absolute_popup_rect(frame), expected);
}

// 打开状态登记浮层：外部点击关闭与 Esc 路由依赖 OverlayStack。
#[test]
fn open_popover_registers_overlay_with_outside_dismiss() {
    // 构造并打开气泡。
    let mut popover = Popover::new("content");
    popover.open();
    // 打开状态必须生成浮层登记。
    let overlay = WidgetRender::overlay_entry(
        &popover,
        WidgetId::new(7),
        Rect::new(10.0, 10.0, 40.0, 20.0),
    )
    // 在场状态必须返回登记。
    .expect("打开的 Popover 应生成浮层登记");
    // 与既有 Select/Dropdown 弹层一致的 z 顺序。
    assert_eq!(overlay.z_index_value(), 900);
    // 外部点击必须注册为可取消端口。
    assert!(overlay.dismisses_on_outside());
    // 关闭后不再登记浮层：等离场动画结束后验证。
    popover.close();
    // 驱动关闭动画直到完全离场。
    while crate::ui::widget_runtime::traits::WidgetAnimation::update_animation(&mut popover, 0.05) {
    }
    assert!(
        WidgetRender::overlay_entry(
            &popover,
            WidgetId::new(7),
            Rect::new(10.0, 10.0, 40.0, 20.0),
        )
        .is_none()
    );
}

// FocusOut 关闭弹层（与 Dropdown 对齐），hover 触发路径不受影响。
#[test]
fn focus_out_closes_open_popover() {
    // 构造点击触发的气泡。
    let mut popover = Popover::new("content");
    // 模拟 Click 触发打开。
    popover.open();
    // 打开状态可见。
    assert!(popover.is_present());
    // 焦点离开必须关闭弹层。
    assert_eq!(
        EventHandler::on_event(&mut popover, &SystemEvent::FocusOut),
        EventResult::Handled
    );
    // 关闭事实立即生效，离场动画可继续呈现。
    assert!(!popover.visible);
}

// 受控打开状态必须双向同步，且不引入第二可见性事实源。
#[test]
fn controlled_open_synchronizes_external_updates_and_user_close() {
    // 创建由调用方拥有且初始打开的状态句柄。
    let open = State::new(true);
    // 构造绑定该句柄的气泡卡片。
    let mut popover = Popover::new("content").controlled_open(&open);
    // 初始外部事实必须立即驱动可见状态。
    assert!(popover.is_visible());
    // 用户关闭路径必须回写同一外部事实。
    popover.close();
    // 外部状态不得继续声称打开。
    assert!(!open.get());
    // 调用方重新打开同一状态。
    open.set(true);
    // 模拟下一次事件入口同步受控事实。
    popover.sync_bound_open();
    // 运行时必须取消离场并重新进入可见状态。
    assert!(popover.is_visible() && !popover.closing);
}

// View 构建必须经过同目录 UIX，并保留调用方显式视觉覆写。
#[test]
fn view_build_uses_uix_visual_and_preserves_authored_values() {
    let enter = AnimationConfig::zoom_in(0.3);
    let leave = AnimationConfig::zoom_out(0.2);
    let node = crate::ui::view::View::build(
        Popover::new("content")
            .placement(PopoverPlacement::BottomRight)
            .arrow(false)
            .enter_animation(enter)
            .leave_animation(leave)
            .trigger_view(ViewNode::leaf(Popover::new("trigger"))),
    );
    let popover = node
        .widget
        .as_any()
        .downcast_ref::<Popover>()
        .expect("UIX 根必须保留 Popover Rust 内核");
    let declared = UIX_POPOVER_VISUAL
        .get()
        .expect("View 构建必须固化同目录 UIX 视觉");
    assert!(std::ptr::eq(popover.visual, declared));
    assert_eq!(popover.placement, PopoverPlacement::BottomRight);
    assert!(!popover.arrow);
    assert_eq!(popover.enter_animation, enter);
    assert_eq!(popover.leave_animation, leave);
    assert!(popover.custom_trigger);
    assert!(
        popover
            .custom_trigger_view
            .as_ref()
            .is_some_and(|view| view.borrow().is_some())
    );
}

// UIX 默认表必须保持迁移前的触发器、气泡和内容几何。
#[test]
fn uix_visual_preserves_existing_geometry() {
    let popover = Popover::new("content");
    assert_eq!(popover.intrinsic_size(), Size::new(80.0, 28.0));
    assert_eq!(popover.visual.defaults.popup_width, 220.0);
    assert_eq!(popover.visual.defaults.popup_height, 100.0);
    assert_eq!(popover.visual.layout.arrow_gap, 10.0);
    assert_eq!(popover.visual.layout.content_inset, 12.0);
    assert_eq!(popover.visual.layout.title_height, 32.0);
    assert_eq!(popover.visual.layout.arrow_size, 8.0);
}

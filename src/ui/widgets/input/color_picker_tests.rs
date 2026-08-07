// 复用父模块中的颜色选择器私有状态与网格常量。
use super::{ColorPicker, PANEL_CELL, PANEL_PADDING};
// 引入断言和事件构造所需的点与矩形类型。
use crate::core::{Point, Rect};
// 引入指针事件所需的输入类型。
use crate::ui::{KeyMod, MouseButton, SystemEvent};

// 打开一个新的颜色面板呈现周期。
fn open_color(picker: &mut ColorPicker) {
    // 使用组件公开入口启动面板与进入动画。
    picker.open();
}

// 向颜色选择器发送本地左键按下事件。
fn pointer_down(picker: &mut ColorPicker, position: Point) {
    // 构造不带修饰键的左键按下事件。
    let event = SystemEvent::PointerDown {
        // 使用调用方提供的组件本地坐标。
        pos: position,
        // 使用颜色选择器支持的左键。
        button: MouseButton::Left,
        // 测试不附加修饰键。
        mods: KeyMod::NONE,
    };
    // 通过组件事件能力处理点击。
    let _ = crate::ui::component::traits::EventHandler::on_event(picker, &event);
}

// 靠近表面底边时颜色面板必须翻转并按上方空间缩高。
#[test]
// 测试名称说明纵向表面约束职责。
fn overlay_entry_constrains_popup_near_surface_bottom() {
    // 创建颜色选择器。
    let mut picker = ColorPicker::new();
    // 打开颜色面板参与登记。
    open_color(&mut picker);
    // 将触发器放在一百二十像素高表面的底部附近。
    let frame = Rect::new(20.0, 80.0, 32.0, 32.0);
    // 构造不足以容纳自然颜色面板的逻辑表面。
    let surface = Rect::new(0.0, 0.0, 240.0, 120.0);
    // 通过组件树使用的显式表面入口创建登记。
    let overlay = crate::ui::component::traits::WidgetRender::overlay_entry_for_surface(
        &picker,
        crate::core::ComponentId::new(47),
        frame,
        surface,
    )
    // 打开状态必须生成浮层登记。
    .expect("打开的颜色选择器应生成浮层登记")
    // 读取登记的绝对颜色面板矩形。
    .bounds_rect()
    // 颜色面板登记必须声明边界。
    .expect("颜色面板应声明边界");

    // 下方空间不足时面板应位于触发器上方并保留间隙。
    assert!(overlay.y + overlay.h <= frame.y - 4.0);
    // 面板顶边不得越出当前表面。
    assert!(overlay.y >= surface.y);
    // 面板底边不得越出当前表面。
    assert!(overlay.y + overlay.h <= surface.y + surface.h);
}

// 靠近窄表面右边缘时颜色面板必须横向收敛。
#[test]
// 测试名称说明固定八列不能撑破当前表面。
fn overlay_entry_constrains_popup_to_narrow_surface() {
    // 创建颜色选择器。
    let mut picker = ColorPicker::new();
    // 打开颜色面板参与登记。
    open_color(&mut picker);
    // 构造靠近右边界的触发器。
    let frame = Rect::new(70.0, 20.0, 32.0, 32.0);
    // 使用一百像素宽的窄逻辑表面。
    let surface = Rect::new(0.0, 0.0, 100.0, 320.0);
    // 通过显式表面入口创建颜色面板登记。
    let overlay = crate::ui::component::traits::WidgetRender::overlay_entry_for_surface(
        &picker,
        crate::core::ComponentId::new(48),
        frame,
        surface,
    )
    // 打开状态必须生成浮层登记。
    .expect("打开的颜色选择器应生成浮层登记")
    // 读取登记的绝对面板边界。
    .bounds_rect()
    // 颜色面板登记必须声明边界。
    .expect("颜色面板应声明边界");

    // 面板左边不得越出当前表面。
    assert!(overlay.x >= surface.x);
    // 面板右边不得越出当前表面。
    assert!(overlay.x + overlay.w <= surface.x + surface.w);
    // 面板宽度不得超过当前表面宽度。
    assert!(overlay.w <= surface.w);
}

// 缩高并翻转后的色块命中必须复用实际面板网格。
#[test]
// 测试名称说明绘制几何与指针索引映射的一致性。
fn constrained_popup_geometry_drives_color_hit_mapping() {
    // 创建可交互的颜色选择器。
    let mut picker = ColorPicker::new();
    // 打开颜色面板。
    open_color(&mut picker);
    // 保存事件路径使用的本地触发器尺寸。
    picker.last_frame.set(Some(Rect::new(0.0, 0.0, 32.0, 32.0)));
    // 将绝对触发器放在表面底部附近。
    let frame = Rect::new(20.0, 80.0, 32.0, 32.0);
    // 迫使自然高度八十八像素的面板向上缩为七十六像素。
    let surface = Rect::new(0.0, 0.0, 240.0, 120.0);
    // 先通过显式登记缓存翻转且缩高后的本地面板。
    let _ = crate::ui::component::traits::WidgetRender::overlay_entry_for_surface(
        &picker,
        crate::core::ComponentId::new(49),
        frame,
        surface,
    );
    // 保存第二行第二列色块对应的预期颜色。
    let expected = picker.preset_colors[9];
    // 计算自然八十八像素面板缩为七十六像素后的纵向比例。
    let y_scale = 76.0 / 88.0;
    // 计算第二列色块在本地面板中的横向中心。
    let x = PANEL_PADDING + PANEL_CELL * 1.5;
    // 计算第二行色块在翻转本地面板中的纵向中心。
    let y = -80.0 + (PANEL_PADDING + PANEL_CELL * 1.5) * y_scale;
    // 点击实际缩放网格中的目标色块。
    pointer_down(&mut picker, Point::new(x, y));

    // 缩高后的命中必须提交第二行第二列颜色。
    assert_eq!(picker.current_value(), expected);
}

// OverlayStack 登记必须只覆盖当前颜色面板而不并入触发器。
#[test]
// 测试名称说明浮层登记与组件占用区的边界。
fn overlay_entry_bounds_only_current_popup() {
    // 创建颜色选择器。
    let mut picker = ColorPicker::new();
    // 打开颜色面板参与登记。
    open_color(&mut picker);
    // 将触发器放在可完整向下展开的表面内。
    let frame = Rect::new(20.0, 20.0, 32.0, 32.0);
    // 使用足以容纳自然颜色面板的逻辑表面。
    let surface = Rect::new(0.0, 0.0, 320.0, 240.0);
    // 创建当前颜色面板的浮层登记。
    let overlay = crate::ui::component::traits::WidgetRender::overlay_entry_for_surface(
        &picker,
        crate::core::ComponentId::new(50),
        frame,
        surface,
    )
    // 打开状态必须生成浮层登记。
    .expect("打开的颜色选择器应生成浮层登记")
    // 读取当前面板边界。
    .bounds_rect()
    // 颜色面板必须声明边界。
    .expect("颜色面板应声明边界");

    // 当前面板应在四像素间隙后开始。
    assert_eq!(overlay.y, frame.y + frame.h + 4.0);
    // 完整表面中的登记应保持八十八像素自然高度。
    assert_eq!(overlay.h, 88.0);
    // 浮层登记的顶边不得回退到触发器顶边。
    assert!(overlay.y > frame.y);
}

// 表面缩小时脏区必须覆盖翻转前面板留在新表面的尾部。
#[test]
// 测试名称说明历史绝对面板与当前表面的交集职责。
fn surface_change_dirty_covers_previous_popup_tail() {
    // 创建颜色选择器。
    let mut picker = ColorPicker::new();
    // 打开颜色面板参与两次登记。
    open_color(&mut picker);
    // 固定触发器位置，使大表面向下而小表面向上。
    let frame = Rect::new(20.0, 80.0, 32.0, 32.0);
    // 首帧表面允许颜色面板完整向下展开。
    let large_surface = Rect::new(0.0, 0.0, 240.0, 400.0);
    // 先登记向下展开的旧颜色面板。
    let _ = crate::ui::component::traits::WidgetRender::overlay_entry_for_surface(
        &picker,
        crate::core::ComponentId::new(51),
        frame,
        large_surface,
    );
    // 次帧表面迫使颜色面板翻到上方并缩高。
    let small_surface = Rect::new(0.0, 0.0, 240.0, 120.0);
    // 刷新登记并累计翻转前后的颜色面板区域。
    let _ = crate::ui::component::traits::WidgetRender::overlay_entry_for_surface(
        &picker,
        crate::core::ComponentId::new(51),
        frame,
        small_surface,
    );
    // 读取组件对当前表面声明的完整脏区。
    let dirty = crate::ui::component::traits::WidgetRender::dirty_rect(&picker, frame);

    // 当前向上面板与触发器共同从表面顶边开始。
    assert_eq!(dirty.y, small_surface.y);
    // 旧向下面板留在新表面的尾部也必须被清理。
    assert_eq!(dirty.y + dirty.h, small_surface.y + small_surface.h);
}

// 退出动画期间颜色面板仍必须沿用当前表面约束。
#[test]
// 测试名称说明关闭状态与实际呈现状态的边界不同。
fn closing_overlay_remains_surface_constrained() {
    // 创建颜色选择器。
    let mut picker = ColorPicker::new();
    // 打开后立即进入退出动画。
    open_color(&mut picker);
    // 启动关闭动画但保留呈现状态。
    picker.close();
    // 确认交互开放状态已经关闭。
    assert!(!picker.is_open());
    // 确认退出动画期间仍需呈现。
    assert!(picker.is_present());
    // 使用靠近底边的触发器。
    let frame = Rect::new(20.0, 80.0, 32.0, 32.0);
    // 使用不足以容纳自然面板的逻辑表面。
    let surface = Rect::new(0.0, 0.0, 240.0, 120.0);
    // 在退出动画期间刷新同帧表面登记。
    let overlay = crate::ui::component::traits::WidgetRender::overlay_entry_for_surface(
        &picker,
        crate::core::ComponentId::new(52),
        frame,
        surface,
    )
    // 退出动画期间必须保留浮层登记。
    .expect("退出动画期间应保留颜色面板登记")
    // 读取退出动画面板的绝对边界。
    .bounds_rect()
    // 退出动画面板必须声明边界。
    .expect("退出动画颜色面板应声明边界");

    // 退出动画面板仍不得越出表面底边。
    assert!(overlay.y + overlay.h <= surface.y + surface.h);
    // 退出动画面板仍应位于触发器上方。
    assert!(overlay.y + overlay.h <= frame.y - 4.0);
}

// 完整关闭后的下一次打开必须开始新的脏区呈现周期。
#[test]
// 测试名称说明旧锚点不能污染下一次打开的脏区。
fn reopened_picker_resets_previous_popup_damage() {
    // 创建颜色选择器。
    let mut picker = ColorPicker::new();
    // 开始第一次颜色面板呈现周期。
    open_color(&mut picker);
    // 使用位于表面左侧的第一次触发器。
    let first_frame = Rect::new(0.0, 20.0, 32.0, 32.0);
    // 使用可以容纳两次面板的宽逻辑表面。
    let surface = Rect::new(0.0, 0.0, 500.0, 240.0);
    // 缓存第一次面板的绝对脏区。
    let _ = crate::ui::component::traits::WidgetRender::overlay_entry_for_surface(
        &picker,
        crate::core::ComponentId::new(53),
        first_frame,
        surface,
    );
    // 启动第一次面板的退出动画。
    picker.close();
    // 推进足够长时间以完成退出动画。
    let _ = crate::ui::component::traits::WidgetAnimation::update_animation(&mut picker, 10.0);
    // 确认第一次呈现周期已经结束。
    assert!(!picker.is_present());
    // 开始新的颜色面板呈现周期。
    open_color(&mut picker);
    // 将第二次触发器移动到表面右侧。
    let second_frame = Rect::new(260.0, 20.0, 32.0, 32.0);
    // 缓存第二次面板的实际几何。
    let _ = crate::ui::component::traits::WidgetRender::overlay_entry_for_surface(
        &picker,
        crate::core::ComponentId::new(53),
        second_frame,
        surface,
    );
    // 读取第二次呈现周期的脏区。
    let dirty = crate::ui::component::traits::WidgetRender::dirty_rect(&picker, second_frame);

    // 新周期脏区不得继续包含第一次面板的左侧区域。
    assert_eq!(dirty.x, second_frame.x);
}

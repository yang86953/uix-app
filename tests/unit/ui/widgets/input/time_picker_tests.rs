// 复用父模块中的时间选择器私有状态与常量。
use super::{ITEM_HEIGHT, Time, TimePicker};
// 引入断言和事件构造所需的点、矩形基础类型。
use crate::core::{Point, Rect};
// 引入指针事件所需的输入类型。
use crate::ui::{KeyMod, MouseButton, SystemEvent};

// 打开带稳定初始值的时间面板。
fn open_time(picker: &TimePicker) {
    // 开始新的时间面板呈现周期。
    picker.open_popup();
}

// 向时间选择器发送本地左键按下事件。
fn pointer_down(picker: &mut TimePicker, position: Point) {
    // 构造无修饰键的左键按下事件。
    let event = SystemEvent::PointerDown {
        // 使用调用方给出的组件本地坐标。
        pos: position,
        // 使用时间选择器支持的左键。
        button: MouseButton::Left,
        // 测试不附加修饰键。
        mods: KeyMod::NONE,
    };
    // 通过组件事件能力处理点击。
    let _ = crate::ui::component::traits::EventHandler::on_event(picker, &event);
}

// 向时间选择器发送本地滚轮事件。
fn wheel(picker: &mut TimePicker, position: Point, delta_y: f32) {
    // 构造只包含纵向增量的滚轮事件。
    let event = SystemEvent::Wheel {
        // 使用调用方给出的组件本地坐标。
        pos: position,
        // 将滚轮增量编码为逻辑点。
        delta: Point::new(0.0, delta_y),
    };
    // 通过组件事件能力处理滚动。
    let _ = crate::ui::component::traits::EventHandler::on_event(picker, &event);
}

// 靠近表面底边时面板必须翻转并按上方空间缩高。
#[test]
// 测试名称说明纵向表面约束职责。
fn overlay_entry_constrains_popup_near_surface_bottom() {
    // 创建并打开时间选择器。
    let picker = TimePicker::new().default_value(Time::new(5, 10));
    // 打开时间面板参与登记。
    open_time(&picker);
    // 将触发器放在一百二十像素高表面的底部附近。
    let frame = Rect::new(20.0, 80.0, 120.0, 32.0);
    // 构造不足以容纳自然时间面板的逻辑表面。
    let surface = Rect::new(0.0, 0.0, 240.0, 120.0);
    // 通过组件树使用的显式表面入口创建登记。
    let overlay = crate::ui::component::traits::WidgetRender::overlay_entry_for_surface(
        // 传入被测时间选择器。
        &picker,
        // 使用稳定的测试组件标识。
        crate::core::ComponentId::new(40),
        // 传入靠近底边的触发器 frame。
        frame,
        // 传入当前逻辑表面。
        surface,
    )
    // 打开状态必须生成浮层登记。
    .expect("打开的时间选择器应生成浮层登记")
    // 读取登记的绝对时间面板矩形。
    .bounds_rect()
    // 时间面板登记必须声明边界。
    .expect("时间面板应声明边界");

    // 下方空间不足时面板应完整位于触发器上方并保留间隙。
    assert!(overlay.y + overlay.h <= frame.y - 2.0);
    // 面板顶边不得越出当前表面。
    assert!(overlay.y >= surface.y);
    // 面板底边不得越出当前表面。
    assert!(overlay.y + overlay.h <= surface.y + surface.h);
}

// 靠近窄表面右边缘时面板必须横向收敛。
#[test]
// 测试名称说明最小自然宽度不能撑破当前表面。
fn overlay_entry_constrains_popup_to_narrow_surface() {
    // 创建并打开时间选择器。
    let picker = TimePicker::new().default_value(Time::new(5, 10));
    // 打开时间面板参与登记。
    open_time(&picker);
    // 构造宽于表面且靠近右边界的触发器。
    let frame = Rect::new(70.0, 20.0, 120.0, 32.0);
    // 使用一百像素宽的窄逻辑表面。
    let surface = Rect::new(0.0, 0.0, 100.0, 320.0);
    // 通过显式表面入口创建时间面板登记。
    let overlay = crate::ui::component::traits::WidgetRender::overlay_entry_for_surface(
        // 传入被测时间选择器。
        &picker,
        // 使用稳定的测试组件标识。
        crate::core::ComponentId::new(41),
        // 传入靠近右边界的触发器 frame。
        frame,
        // 传入当前窄表面。
        surface,
    )
    // 打开状态必须生成浮层登记。
    .expect("打开的时间选择器应生成浮层登记")
    // 读取登记的绝对时间面板边界。
    .bounds_rect()
    // 时间面板登记必须声明边界。
    .expect("时间面板应声明边界");

    // 面板左边不得越出当前表面。
    assert!(overlay.x >= surface.x);
    // 面板右边不得越出当前表面。
    assert!(overlay.x + overlay.w <= surface.x + surface.w);
    // 面板宽度不得超过当前表面宽度。
    assert!(overlay.w <= surface.w);
}

// 缩高并翻转后的选项命中必须复用实际时间面板。
#[test]
// 测试名称说明绘制视口与指针索引映射的一致性。
fn constrained_popup_geometry_drives_item_hit_mapping() {
    // 创建带稳定初始值的时间选择器。
    let mut picker = TimePicker::new().default_value(Time::new(5, 10));
    // 打开时间面板并同步高亮与滚动。
    open_time(&picker);
    // 将事件路径使用的触发器本地 frame 设为实际尺寸。
    picker
        // 访问最近本地 frame 缓存。
        .last_frame
        // 保存以组件原点为起点的触发器。
        .set(Some(Rect::new(0.0, 0.0, 120.0, 32.0)));
    // 将绝对触发器放在表面底部附近。
    let frame = Rect::new(20.0, 80.0, 120.0, 32.0);
    // 使用一百二十像素高表面迫使面板向上缩为七十八像素。
    let surface = Rect::new(0.0, 0.0, 240.0, 120.0);
    // 先通过显式登记入口缓存翻转后的本地时间面板。
    let _ = crate::ui::component::traits::WidgetRender::overlay_entry_for_surface(
        // 传入被测时间选择器。
        &picker,
        // 使用稳定的测试组件标识。
        crate::core::ComponentId::new(42),
        // 传入靠近底边的绝对触发器。
        frame,
        // 传入当前逻辑表面。
        surface,
    );
    // 读取登记后小时列的实际滚动偏移。
    let scroll = picker.scroll_hour.get();
    // 计算第四个小时选项在翻转本地面板中的中心。
    let y = -80.0 + 4.0 * ITEM_HEIGHT - scroll + ITEM_HEIGHT * 0.5;
    // 点击缩高小时列中的目标选项。
    pointer_down(&mut picker, Point::new(30.0, y));

    // 缩高后的命中必须提交目标小时并保留分钟。
    assert_eq!(picker.current_value(), Time::new(4, 10));
}

// 缩高视口的滚动上限必须允许分钟列最后一项进入可见区域。
#[test]
// 测试名称说明滚轮范围和选项命中共享实际视口高度。
fn constrained_popup_scroll_reaches_last_minute() {
    // 创建零时零分的时间选择器。
    let mut picker = TimePicker::new().default_value(Time::new(0, 0));
    // 打开时间面板。
    open_time(&picker);
    // 保存事件路径使用的本地触发器 frame。
    picker
        // 访问最近本地 frame 缓存。
        .last_frame
        // 保存实际触发器尺寸。
        .set(Some(Rect::new(0.0, 0.0, 120.0, 32.0)));
    // 将绝对触发器放在表面底部附近。
    let frame = Rect::new(20.0, 80.0, 120.0, 32.0);
    // 使用七十八像素可用上方空间。
    let surface = Rect::new(0.0, 0.0, 240.0, 120.0);
    // 缓存翻转并缩高后的实际时间面板。
    let _ = crate::ui::component::traits::WidgetRender::overlay_entry_for_surface(
        // 传入被测时间选择器。
        &picker,
        // 使用稳定的测试组件标识。
        crate::core::ComponentId::new(43),
        // 传入当前绝对触发器。
        frame,
        // 传入当前逻辑表面。
        surface,
    );
    // 在缩高后的分钟列内发送足够大的向下滚动。
    wheel(&mut picker, Point::new(90.0, -40.0), 1000.0);
    // 计算实际七十八像素视口下的分钟列最大滚动。
    let expected_scroll = 60.0 * ITEM_HEIGHT - 78.0;
    // 滚动上限必须由实际视口高度决定。
    assert_eq!(picker.scroll_min.get(), expected_scroll);
    // 计算最后一分钟选项在实际本地视口中的中心。
    let y = -80.0 + 59.0 * ITEM_HEIGHT - expected_scroll + ITEM_HEIGHT * 0.5;
    // 点击缩高分钟列中的最后一项。
    pointer_down(&mut picker, Point::new(90.0, y));

    // 最后一分钟必须能够通过实际视口提交。
    assert_eq!(picker.current_value(), Time::new(0, 59));
}

// 表面缩高后当前小时和分钟高亮仍必须至少部分可见。
#[test]
// 测试名称说明当前值显露依赖实际视口高度。
fn constrained_popup_keeps_current_selection_visible() {
    // 创建选中两列末项的时间选择器。
    let picker = TimePicker::new().default_value(Time::new(23, 59));
    // 打开时间面板并按自然视口同步滚动。
    open_time(&picker);
    // 将触发器放在表面底部附近。
    let frame = Rect::new(20.0, 80.0, 120.0, 32.0);
    // 使用七十八像素可用上方空间。
    let surface = Rect::new(0.0, 0.0, 240.0, 120.0);
    // 缓存翻转并缩高后的实际时间面板。
    let _ = crate::ui::component::traits::WidgetRender::overlay_entry_for_surface(
        // 传入被测时间选择器。
        &picker,
        // 使用稳定的测试组件标识。
        crate::core::ComponentId::new(44),
        // 传入当前绝对触发器。
        frame,
        // 传入当前逻辑表面。
        surface,
    );
    // 计算当前小时行相对实际视口的顶部。
    let hour_top = 23.0 * ITEM_HEIGHT - picker.scroll_hour.get();
    // 计算当前分钟行相对实际视口的顶部。
    let minute_top = 59.0 * ITEM_HEIGHT - picker.scroll_min.get();

    // 当前小时行必须与七十八像素视口相交。
    assert!(hour_top < 78.0 && hour_top + ITEM_HEIGHT > 0.0);
    // 当前分钟行必须与七十八像素视口相交。
    assert!(minute_top < 78.0 && minute_top + ITEM_HEIGHT > 0.0);
}

// OverlayStack 登记必须只覆盖当前时间面板而不并入输入框。
#[test]
// 测试名称说明浮层登记与触发器占用区的边界。
fn overlay_entry_bounds_only_current_popup() {
    // 创建并打开时间选择器。
    let picker = TimePicker::new().default_value(Time::new(5, 10));
    // 打开时间面板参与登记。
    open_time(&picker);
    // 将触发器放在可完整向下展开的表面内。
    let frame = Rect::new(20.0, 20.0, 120.0, 32.0);
    // 使用足以容纳自然时间面板的逻辑表面。
    let surface = Rect::new(0.0, 0.0, 320.0, 400.0);
    // 创建当前时间面板的浮层登记。
    let overlay = crate::ui::component::traits::WidgetRender::overlay_entry_for_surface(
        // 传入被测时间选择器。
        &picker,
        // 使用稳定的测试组件标识。
        crate::core::ComponentId::new(45),
        // 传入当前触发器 frame。
        frame,
        // 传入当前逻辑表面。
        surface,
    )
    // 打开状态必须生成浮层登记。
    .expect("打开的时间选择器应生成浮层登记")
    // 读取当前时间面板边界。
    .bounds_rect()
    // 时间面板必须声明边界。
    .expect("时间面板应声明边界");

    // 当前面板应在两像素间隙后开始。
    assert_eq!(overlay.y, frame.y + frame.h + 2.0);
    // 完整表面中的当前登记应保持两百像素自然高度。
    assert_eq!(overlay.h, 200.0);
}

// 表面缩小时脏区必须覆盖翻转前面板留在新表面的尾部。
#[test]
// 测试名称说明历史绝对面板与当前表面的交集职责。
fn surface_change_dirty_covers_previous_popup_tail() {
    // 创建并打开时间选择器。
    let picker = TimePicker::new().default_value(Time::new(5, 10));
    // 打开时间面板参与两次登记。
    open_time(&picker);
    // 固定触发器位置，使大表面向下、小表面向上。
    let frame = Rect::new(20.0, 80.0, 120.0, 32.0);
    // 首帧表面允许时间面板完整向下展开。
    let large_surface = Rect::new(0.0, 0.0, 240.0, 400.0);
    // 先登记向下展开的旧时间面板。
    let _ = crate::ui::component::traits::WidgetRender::overlay_entry_for_surface(
        // 传入被测时间选择器。
        &picker,
        // 使用稳定的测试组件标识。
        crate::core::ComponentId::new(46),
        // 传入固定触发器 frame。
        frame,
        // 传入可完整向下展开的大表面。
        large_surface,
    );
    // 次帧表面迫使面板翻到上方并缩高。
    let small_surface = Rect::new(0.0, 0.0, 240.0, 120.0);
    // 刷新登记并累计翻转前后的时间面板区域。
    let _ = crate::ui::component::traits::WidgetRender::overlay_entry_for_surface(
        // 传入同一时间选择器。
        &picker,
        // 沿用同一测试组件标识。
        crate::core::ComponentId::new(46),
        // 触发器位置保持不变。
        frame,
        // 传入缩小后的当前表面。
        small_surface,
    );
    // 读取组件对当前表面声明的完整脏区。
    let dirty = crate::ui::component::traits::WidgetRender::dirty_rect(&picker, frame);

    // 当前向上面板与触发器共同从表面顶边开始。
    assert_eq!(dirty.y, small_surface.y);
    // 旧向下面板在新表面内残留的尾部也必须被清理。
    assert_eq!(dirty.y + dirty.h, small_surface.y + small_surface.h);
}

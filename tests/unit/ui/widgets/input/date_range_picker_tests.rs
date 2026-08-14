// 复用父模块中的范围选择器私有状态。
use super::{DateRangePicker, PresetDate};
// 引入断言所需的点与矩形基础类型。
use crate::core::{Point, Rect};
// 引入事件行为 trait 以驱动键盘事件。
use crate::ui::component::traits::EventHandler;
// 引入日期与首日星期辅助。
use crate::ui::widgets::input::date_picker::{Date, first_weekday};
// 引入指针事件所需的输入类型。
use crate::ui::{KeyCode, KeyMod, MouseButton, SystemEvent};

// 构造三个稳定且可预测的具名预设。
fn test_presets() -> [(String, PresetDate); 3] {
    // 返回按日期递增的三个预设。
    [
        // 第一个预设覆盖八月首日。
        (
            "首日".to_owned(),
            PresetDate::new(Date::new(2026, 8, 1), Date::new(2026, 8, 1)),
        ),
        // 第二个预设覆盖八月首周。
        (
            "首周".to_owned(),
            PresetDate::new(Date::new(2026, 8, 1), Date::new(2026, 8, 7)),
        ),
        // 第三个预设覆盖八月全月。
        (
            "全月".to_owned(),
            PresetDate::new(Date::new(2026, 8, 1), Date::new(2026, 8, 31)),
        ),
    ]
}

// 打开范围面板并固定可预测的视图月份。
fn open_august_2026(picker: &DateRangePicker) {
    // 开始新的范围面板呈现周期。
    picker.open_from_current_value();
    // 固定视图年份避免依赖执行当天。
    picker.view_year.set(2026);
    // 固定视图月份供日期命中契约复用。
    picker.view_month.set(8);
}

// 向范围选择器发送本地左键按下事件。
fn pointer_down(picker: &mut DateRangePicker, position: Point) {
    // 构造无修饰键的左键按下事件。
    let event = SystemEvent::PointerDown {
        // 使用调用方给出的组件本地坐标。
        pos: position,
        // 使用范围选择器支持的左键。
        button: MouseButton::Left,
        // 测试不附加修饰键。
        mods: KeyMod::NONE,
    };
    // 通过组件事件能力处理点击。
    let _ = crate::ui::component::traits::EventHandler::on_event(picker, &event);
}

// 靠近表面底边时组合面板必须翻转并按上方空间缩高。
#[test]
// 测试名称说明纵向表面约束职责。
fn overlay_entry_constrains_popup_near_surface_bottom() {
    // 创建带三个预设的范围选择器。
    let picker = DateRangePicker::new().presets(test_presets());
    // 打开范围面板参与登记。
    open_august_2026(&picker);
    // 将触发器放在一百二十像素高表面的底部附近。
    let frame = Rect::new(20.0, 80.0, 120.0, 32.0);
    // 构造不足以容纳自然组合面板的逻辑表面。
    let surface = Rect::new(0.0, 0.0, 240.0, 120.0);
    // 通过组件树使用的显式表面入口创建登记。
    let overlay = crate::ui::component::traits::WidgetRender::overlay_entry_for_surface(
        // 传入被测范围选择器。
        &picker,
        // 使用稳定的测试组件标识。
        crate::core::ComponentId::new(34),
        // 传入靠近底边的触发器 frame。
        frame,
        // 传入当前逻辑表面。
        surface,
    )
    // 打开状态必须生成浮层登记。
    .expect("打开的日期范围选择器应生成浮层登记")
    // 读取登记的绝对组合面板矩形。
    .bounds_rect()
    // 组合面板登记必须声明边界。
    .expect("日期范围面板应声明边界");

    // 下方空间不足时面板应完整位于触发器上方并保留间隙。
    assert!(overlay.y + overlay.h <= frame.y - 2.0);
    // 面板顶边不得越出当前表面。
    assert!(overlay.y >= surface.y);
    // 面板底边不得越出当前表面。
    assert!(overlay.y + overlay.h <= surface.y + surface.h);
}

// 触发器靠近窄表面右边缘时组合面板必须横向收敛。
#[test]
// 测试名称说明月历最小自然宽度不能撑破当前表面。
fn overlay_entry_constrains_popup_to_narrow_surface() {
    // 创建并打开带预设的范围选择器。
    let picker = DateRangePicker::new().presets(test_presets());
    // 打开范围面板参与登记。
    open_august_2026(&picker);
    // 构造宽于表面且靠近右边界的触发器。
    let frame = Rect::new(70.0, 20.0, 120.0, 32.0);
    // 使用一百像素宽的窄逻辑表面。
    let surface = Rect::new(0.0, 0.0, 100.0, 420.0);
    // 通过显式表面入口创建范围面板登记。
    let overlay = crate::ui::component::traits::WidgetRender::overlay_entry_for_surface(
        // 传入被测范围选择器。
        &picker,
        // 使用稳定的测试组件标识。
        crate::core::ComponentId::new(35),
        // 传入靠近右边界的触发器 frame。
        frame,
        // 传入当前窄表面。
        surface,
    )
    // 打开状态必须生成浮层登记。
    .expect("打开的日期范围选择器应生成浮层登记")
    // 读取登记的绝对组合面板矩形。
    .bounds_rect()
    // 组合面板登记必须声明边界。
    .expect("日期范围面板应声明边界");

    // 面板左边不得越出当前表面。
    assert!(overlay.x >= surface.x);
    // 面板右边不得越出当前表面。
    assert!(overlay.x + overlay.w <= surface.x + surface.w);
    // 面板宽度不得超过当前表面宽度。
    assert!(overlay.w <= surface.w);
}

// 缩高并翻转后日期命中必须复用组合面板中的实际月历高度。
#[test]
// 测试名称说明月历绘制与日期交互映射的一致性。
fn constrained_popup_geometry_drives_date_hit_mapping() {
    // 创建带三个预设的范围选择器。
    let mut picker = DateRangePicker::new().presets(test_presets());
    // 固定可预测的视图月份。
    open_august_2026(&picker);
    // 将事件路径使用的触发器本地 frame 设为实际尺寸。
    picker
        .last_frame
        .set(Some(Rect::new(0.0, 0.0, 120.0, 32.0)));
    // 预置开始日期，使下一次日期命中提交完整范围。
    picker.pending_start.set(Some(Date::new(2026, 7, 31)));
    // 将绝对触发器放在表面底部附近。
    let frame = Rect::new(20.0, 80.0, 120.0, 32.0);
    // 使用一百二十像素高表面迫使组合面板向上缩为七十八像素。
    let surface = Rect::new(0.0, 0.0, 240.0, 120.0);
    // 先通过显式登记入口缓存翻转后的本地组合面板。
    let _ = crate::ui::component::traits::WidgetRender::overlay_entry_for_surface(
        // 传入被测范围选择器。
        &picker,
        // 使用稳定的测试组件标识。
        crate::core::ComponentId::new(36),
        // 传入靠近底边的触发器 frame。
        frame,
        // 传入当前逻辑表面。
        surface,
    );
    // 计算三个预设下组合面板的自然总高度。
    let natural_height = 250.0 + 2.0 + 8.0 + 24.0 * 3.0;
    // 计算组合面板缩到七十八像素后的共同纵向比例。
    let y_scale = 78.0 / natural_height;
    // 计算 2026 年 8 月 1 日所在的月历列。
    let column = first_weekday(2026, 8) % 7;
    // 计算自然宽度一百六十像素下目标日期的横向中心。
    let x = 8.0 + (column as f32 + 0.5) * ((160.0 - 16.0) / 7.0);
    // 计算翻转本地起点、缩放表头与首行日期中心。
    let y = -80.0 + (56.0 + 15.0) * y_scale;
    // 点击缩高月历中的目标日期。
    pointer_down(&mut picker, Point::new(x, y));

    // 缩高后的首行日期必须提交正确范围。
    assert_eq!(
        picker.current_range(),
        Some((Date::new(2026, 7, 31), Date::new(2026, 8, 1)))
    );
}

// 缩高并翻转后预设命中必须复用组合面板中的实际页脚指标。
#[test]
// 测试名称说明预设绘制与交互映射的一致性。
fn constrained_popup_geometry_drives_preset_hit_mapping() {
    // 创建带三个预设的范围选择器。
    let mut picker = DateRangePicker::new().presets(test_presets());
    // 固定可预测的视图月份。
    open_august_2026(&picker);
    // 将事件路径使用的触发器本地 frame 设为实际尺寸。
    picker
        .last_frame
        .set(Some(Rect::new(0.0, 0.0, 120.0, 32.0)));
    // 将绝对触发器放在表面底部附近。
    let frame = Rect::new(20.0, 80.0, 120.0, 32.0);
    // 使用一百二十像素高表面迫使组合面板向上缩为七十八像素。
    let surface = Rect::new(0.0, 0.0, 240.0, 120.0);
    // 先通过显式登记入口缓存翻转后的本地组合面板。
    let _ = crate::ui::component::traits::WidgetRender::overlay_entry_for_surface(
        // 传入被测范围选择器。
        &picker,
        // 使用稳定的测试组件标识。
        crate::core::ComponentId::new(37),
        // 传入靠近底边的触发器 frame。
        frame,
        // 传入当前逻辑表面。
        surface,
    );
    // 计算三个预设下组合面板的自然总高度。
    let natural_height = 250.0 + 2.0 + 8.0 + 24.0 * 3.0;
    // 计算组合面板缩到七十八像素后的共同纵向比例。
    let y_scale = 78.0 / natural_height;
    // 计算第二个预设行的本地纵向中心。
    let y = -80.0 + (250.0 + 2.0 + 4.0 + 24.0 * 1.5) * y_scale;
    // 点击缩高页脚中的第二个预设。
    pointer_down(&mut picker, Point::new(20.0, y));

    // 第二个预设必须提交八月首周范围。
    assert_eq!(
        picker.current_range(),
        Some((Date::new(2026, 8, 1), Date::new(2026, 8, 7)))
    );
}

// OverlayStack 登记必须只覆盖当前组合面板而不并入输入框。
#[test]
// 测试名称说明浮层登记与触发器占用区的边界。
fn overlay_entry_bounds_only_current_popup() {
    // 创建带三个预设的范围选择器。
    let picker = DateRangePicker::new().presets(test_presets());
    // 固定可预测的视图月份。
    open_august_2026(&picker);
    // 将触发器放在可完整向下展开的表面内。
    let frame = Rect::new(20.0, 20.0, 160.0, 32.0);
    // 使用足以容纳自然组合面板的逻辑表面。
    let surface = Rect::new(0.0, 0.0, 320.0, 500.0);
    // 创建当前范围面板的浮层登记。
    let overlay = crate::ui::component::traits::WidgetRender::overlay_entry_for_surface(
        // 传入被测范围选择器。
        &picker,
        // 使用稳定的测试组件标识。
        crate::core::ComponentId::new(38),
        // 传入当前触发器 frame。
        frame,
        // 传入当前逻辑表面。
        surface,
    )
    // 打开状态必须生成浮层登记。
    .expect("打开的日期范围选择器应生成浮层登记")
    // 读取当前组合面板边界。
    .bounds_rect()
    // 组合面板必须声明边界。
    .expect("日期范围面板应声明边界");

    // 当前面板应在两像素间隙后开始。
    assert_eq!(overlay.y, frame.y + frame.h + 2.0);
    // 完整表面中的当前登记应包含月历、间隙和三个预设行。
    assert_eq!(overlay.h, 250.0 + 2.0 + 8.0 + 24.0 * 3.0);
}

// 表面缩小时脏区必须覆盖翻转前组合面板仍留在新表面的尾部。
#[test]
// 测试名称说明历史绝对面板与当前表面的交集职责。
fn surface_change_dirty_covers_previous_popup_tail() {
    // 创建带三个预设的范围选择器。
    let picker = DateRangePicker::new().presets(test_presets());
    // 固定可预测的视图月份。
    open_august_2026(&picker);
    // 固定触发器位置，使大表面向下、小表面向上。
    let frame = Rect::new(20.0, 80.0, 120.0, 32.0);
    // 首帧表面允许组合面板完整向下展开。
    let large_surface = Rect::new(0.0, 0.0, 240.0, 500.0);
    // 先登记向下展开的旧组合面板。
    let _ = crate::ui::component::traits::WidgetRender::overlay_entry_for_surface(
        // 传入被测范围选择器。
        &picker,
        // 使用稳定的测试组件标识。
        crate::core::ComponentId::new(39),
        // 传入固定触发器 frame。
        frame,
        // 传入可完整向下展开的大表面。
        large_surface,
    );
    // 次帧表面迫使组合面板翻到上方并缩高。
    let small_surface = Rect::new(0.0, 0.0, 240.0, 120.0);
    // 刷新登记并累计翻转前后的组合面板区域。
    let _ = crate::ui::component::traits::WidgetRender::overlay_entry_for_surface(
        // 传入同一范围选择器。
        &picker,
        // 沿用同一测试组件标识。
        crate::core::ComponentId::new(39),
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

// 打开态 Up/Down 以周为单位移动键盘聚焦日期并同步视图月。
#[test]
fn open_range_picker_arrow_keys_move_focused_date() {
    // 创建并打开范围选择器。
    let mut picker = DateRangePicker::new();
    // 固定可预测的视图月份。
    open_august_2026(&picker);
    // 以既有范围起点为键盘移动锚点。
    picker.commit_range(Date::new(2026, 8, 5), Date::new(2026, 8, 20));

    // Down 在网格中向下移动一周。
    let _ = EventHandler::on_event(
        &mut picker,
        &SystemEvent::KeyDown {
            key: KeyCode::Down,
            mods: KeyMod::NONE,
        },
    );
    // 焦点日期以范围起点为锚顺延七天。
    assert_eq!(picker.hover_date.get(), Some(Date::new(2026, 8, 12)));

    // Up 从当前聚焦日期向上移动一周，回到锚点且不跨月。
    let _ = EventHandler::on_event(
        &mut picker,
        &SystemEvent::KeyDown {
            key: KeyCode::Up,
            mods: KeyMod::NONE,
        },
    );
    // 焦点日期回到范围起点。
    assert_eq!(picker.hover_date.get(), Some(Date::new(2026, 8, 5)));
    // 同月移动不改变视图月份。
    assert_eq!((picker.view_year.get(), picker.view_month.get()), (2026, 8));

    // 从月初附近向上移动一周必然跨月。
    let _ = EventHandler::on_event(
        &mut picker,
        &SystemEvent::KeyDown {
            key: KeyCode::Up,
            mods: KeyMod::NONE,
        },
    );
    // 焦点日期跨月到七月末。
    assert_eq!(picker.hover_date.get(), Some(Date::new(2026, 7, 29)));
    // 视图月份跟随焦点日期同步。
    assert_eq!((picker.view_year.get(), picker.view_month.get()), (2026, 7));
}

// 打开态 Enter 复用指针的两段式选择语义：先定起点、再定终点。
#[test]
fn open_range_picker_enter_drives_two_step_selection() {
    // 创建并打开范围选择器。
    let mut picker = DateRangePicker::new();
    // 固定可预测的视图月份。
    open_august_2026(&picker);
    // 键盘聚焦到起点日期。
    picker.hover_date.set(Some(Date::new(2026, 8, 10)));

    // 第一次 Enter 只确定起点。
    let enter = SystemEvent::KeyDown {
        key: KeyCode::Enter,
        mods: KeyMod::NONE,
    };
    let _ = EventHandler::on_event(&mut picker, &enter);
    // 起点已记录，面板保持打开。
    assert_eq!(picker.pending_start.get(), Some(Date::new(2026, 8, 10)));
    assert!(picker.open.get());

    // 第二次 Enter 以聚焦终点完成范围。
    picker.hover_date.set(Some(Date::new(2026, 8, 15)));
    let _ = EventHandler::on_event(&mut picker, &enter);
    // 范围已提交且起点终点有序化。
    assert_eq!(
        picker.current_range(),
        Some((Date::new(2026, 8, 10), Date::new(2026, 8, 15)))
    );
    // 提交后关闭面板。
    assert!(!picker.open.get());
}

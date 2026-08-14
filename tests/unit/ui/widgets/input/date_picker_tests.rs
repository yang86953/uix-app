// 复用父模块中的日期选择器私有状态。
use super::{first_weekday, Date, DatePicker};
// 引入断言所需的点与矩形基础类型。
use crate::core::{Point, Rect};
// 引入事件行为 trait 以驱动键盘事件。
use crate::ui::component::traits::EventHandler;
// 引入指针事件所需的输入类型。
use crate::ui::{KeyCode, KeyMod, MouseButton, SystemEvent};

// 打开日期面板并固定可预测的视图月份。
fn open_august_2026(picker: &DatePicker) {
    // 开始新的日期面板呈现周期。
    picker.open_popup();
    // 固定视图年份避免依赖执行当天。
    picker.view_year.set(2026);
    // 固定视图月份供日期命中契约复用。
    picker.view_month.set(8);
}

// 靠近表面底边时日期面板必须翻转并按上方空间缩高。
#[test]
// 测试名称说明纵向表面约束职责。
fn overlay_entry_constrains_popup_near_surface_bottom() {
    // 创建日期选择器。
    let picker = DatePicker::new();
    // 打开日期面板参与登记。
    open_august_2026(&picker);
    // 将触发器放在一百二十像素高表面的底部附近。
    let frame = Rect::new(20.0, 80.0, 120.0, 32.0);
    // 构造不足以容纳自然月历高度的当前逻辑表面。
    let surface = Rect::new(0.0, 0.0, 240.0, 120.0);
    // 通过组件树使用的显式表面入口创建登记。
    let overlay = crate::ui::component::traits::WidgetRender::overlay_entry_for_surface(
        // 传入被测日期选择器。
        &picker,
        // 使用稳定的测试组件标识。
        crate::core::ComponentId::new(29),
        // 传入靠近底边的触发器 frame。
        frame,
        // 传入当前逻辑表面。
        surface,
    )
    // 打开状态必须生成浮层登记。
    .expect("打开的日期选择器应生成浮层登记")
    // 读取登记的绝对面板矩形。
    .bounds_rect()
    // 日期面板登记必须声明边界。
    .expect("日期面板应声明边界");

    // 下方空间不足时面板应完整位于触发器上方并保留间隙。
    assert!(overlay.y + overlay.h <= frame.y - 2.0);
    // 面板顶边不得越出当前表面。
    assert!(overlay.y >= surface.y);
    // 面板底边不得越出当前表面。
    assert!(overlay.y + overlay.h <= surface.y + surface.h);
}

// 触发器靠近窄表面右边缘时日期面板必须横向收敛。
#[test]
// 测试名称说明最小自然宽度不能撑破当前表面。
fn overlay_entry_constrains_popup_to_narrow_surface() {
    // 创建并打开日期选择器。
    let picker = DatePicker::new();
    // 打开日期面板参与登记。
    open_august_2026(&picker);
    // 构造宽于表面且靠近右边界的触发器。
    let frame = Rect::new(70.0, 20.0, 120.0, 32.0);
    // 使用一百像素宽的窄逻辑表面。
    let surface = Rect::new(0.0, 0.0, 100.0, 320.0);
    // 通过显式表面入口创建日期面板登记。
    let overlay = crate::ui::component::traits::WidgetRender::overlay_entry_for_surface(
        // 传入被测日期选择器。
        &picker,
        // 使用稳定的测试组件标识。
        crate::core::ComponentId::new(30),
        // 传入靠近右边界的触发器 frame。
        frame,
        // 传入当前窄表面。
        surface,
    )
    // 打开状态必须生成浮层登记。
    .expect("打开的日期选择器应生成浮层登记")
    // 读取登记的绝对面板矩形。
    .bounds_rect()
    // 日期面板登记必须声明边界。
    .expect("日期面板应声明边界");

    // 面板左边不得越出当前表面。
    assert!(overlay.x >= surface.x);
    // 面板右边不得越出当前表面。
    assert!(overlay.x + overlay.w <= surface.x + surface.w);
    // 面板宽度不得超过当前表面宽度。
    assert!(overlay.w <= surface.w);
}

// 缩高并翻转后日期命中必须复用实际面板纵向指标。
#[test]
// 测试名称说明绘制几何与日期交互映射的一致性。
fn constrained_popup_geometry_drives_date_hit_mapping() {
    // 创建并打开日期选择器。
    let mut picker = DatePicker::new();
    // 固定可预测的视图月份。
    open_august_2026(&picker);
    // 将事件路径使用的触发器本地 frame 设为实际尺寸。
    picker
        .last_frame
        .set(Some(Rect::new(0.0, 0.0, 120.0, 32.0)));
    // 将绝对触发器放在表面底部附近。
    let frame = Rect::new(20.0, 80.0, 120.0, 32.0);
    // 使用一百二十像素高表面迫使面板向上缩为七十八像素。
    let surface = Rect::new(0.0, 0.0, 240.0, 120.0);
    // 先通过显式登记入口缓存翻转后的本地面板矩形。
    let _ = crate::ui::component::traits::WidgetRender::overlay_entry_for_surface(
        // 传入被测日期选择器。
        &picker,
        // 使用稳定的测试组件标识。
        crate::core::ComponentId::new(31),
        // 传入靠近底边的触发器 frame。
        frame,
        // 传入当前逻辑表面。
        surface,
    );
    // 计算 2026 年 8 月 1 日所在的月历列。
    let column = first_weekday(2026, 8) % 7;
    // 计算受限面板按七十八除以二百五十缩放后的首行日期中心。
    let y_scale = 78.0 / 250.0;
    // 计算自然宽度一百六十像素下目标日期的横向中心。
    let x = 8.0 + (column as f32 + 0.5) * ((160.0 - 16.0) / 7.0);
    // 计算翻转本地起点、缩放表头与首行单元格的纵向中心。
    let y = -80.0 + 56.0 * y_scale + 15.0 * y_scale;
    // 向缩高月历中的目标日期发送左键按下事件。
    let event = SystemEvent::PointerDown {
        // 使用组件本地坐标命中翻转面板。
        pos: Point::new(x, y),
        // 使用日期选择器支持的左键。
        button: MouseButton::Left,
        // 测试不附加修饰键。
        mods: KeyMod::NONE,
    };
    // 通过组件事件能力处理日期点击。
    let _ = crate::ui::component::traits::EventHandler::on_event(&mut picker, &event);

    // 缩高后的首行日期必须仍能提交正确值。
    assert_eq!(picker.current_value(), Date::new(2026, 8, 1));
}

// OverlayStack 登记必须只覆盖当前日期面板而不并入输入框。
#[test]
// 测试名称说明浮层登记与触发器占用区的边界。
fn overlay_entry_bounds_only_current_popup() {
    // 创建并打开日期选择器。
    let picker = DatePicker::new();
    // 固定可预测的视图月份。
    open_august_2026(&picker);
    // 将触发器放在可完整向下展开的表面内。
    let frame = Rect::new(20.0, 20.0, 160.0, 32.0);
    // 使用足以容纳自然月历的逻辑表面。
    let surface = Rect::new(0.0, 0.0, 320.0, 400.0);
    // 创建当前日期面板的浮层登记。
    let overlay = crate::ui::component::traits::WidgetRender::overlay_entry_for_surface(
        // 传入被测日期选择器。
        &picker,
        // 使用稳定的测试组件标识。
        crate::core::ComponentId::new(32),
        // 传入当前触发器 frame。
        frame,
        // 传入当前逻辑表面。
        surface,
    )
    // 打开状态必须生成浮层登记。
    .expect("打开的日期选择器应生成浮层登记")
    // 读取当前面板边界。
    .bounds_rect()
    // 日期面板必须声明边界。
    .expect("日期面板应声明边界");

    // 当前面板应在两像素间隙后开始。
    assert_eq!(overlay.y, frame.y + frame.h + 2.0);
    // 完整表面中的当前登记只应包含二百五十像素月历。
    assert_eq!(overlay.h, 250.0);
}

// 表面缩小时脏区必须覆盖翻转前面板仍留在新表面的尾部。
#[test]
// 测试名称说明历史绝对面板与当前表面的交集职责。
fn surface_change_dirty_covers_previous_popup_tail() {
    // 创建并打开日期选择器。
    let picker = DatePicker::new();
    // 固定可预测的视图月份。
    open_august_2026(&picker);
    // 固定触发器位置，使大表面向下、小表面向上。
    let frame = Rect::new(20.0, 80.0, 120.0, 32.0);
    // 首帧表面允许月历完整向下展开。
    let large_surface = Rect::new(0.0, 0.0, 240.0, 400.0);
    // 先登记向下展开的旧面板。
    let _ = crate::ui::component::traits::WidgetRender::overlay_entry_for_surface(
        // 传入被测日期选择器。
        &picker,
        // 使用稳定的测试组件标识。
        crate::core::ComponentId::new(33),
        // 传入固定触发器 frame。
        frame,
        // 传入可完整向下展开的大表面。
        large_surface,
    );
    // 次帧表面迫使面板翻到上方并缩高。
    let small_surface = Rect::new(0.0, 0.0, 240.0, 120.0);
    // 刷新登记并累计翻转前后的面板区域。
    let _ = crate::ui::component::traits::WidgetRender::overlay_entry_for_surface(
        // 传入同一日期选择器。
        &picker,
        // 沿用同一测试组件标识。
        crate::core::ComponentId::new(33),
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

// 打开态 Up/Down 以周为单位移动键盘聚焦日期，同月移动保持视图不变。
#[test]
fn open_picker_arrow_keys_move_focused_date_within_month() {
    // 创建并打开日期选择器。
    let mut picker = DatePicker::new();
    // 固定可预测的视图月份。
    open_august_2026(&picker);
    // 以选中值为键盘移动锚点。
    picker.commit_value(Date::new(2026, 8, 15));

    // Down 在网格中向下移动一周。
    let _ = EventHandler::on_event(
        &mut picker,
        &SystemEvent::KeyDown {
            key: KeyCode::Down,
            mods: KeyMod::NONE,
        },
    );
    // 焦点日期顺延七天。
    assert_eq!(picker.hover_date.get(), Some(Date::new(2026, 8, 22)));

    // Up 在网格中向上移动一周。
    let _ = EventHandler::on_event(
        &mut picker,
        &SystemEvent::KeyDown {
            key: KeyCode::Up,
            mods: KeyMod::NONE,
        },
    );
    // 焦点日期回到锚点。
    assert_eq!(picker.hover_date.get(), Some(Date::new(2026, 8, 15)));
    // 同月移动不改变视图月份。
    assert_eq!((picker.view_year.get(), picker.view_month.get()), (2026, 8));
}

// 跨月移动时视图月份必须同步，保证网格内焦点可见。
#[test]
fn open_picker_arrow_keys_sync_view_when_crossing_month() {
    // 创建并打开日期选择器。
    let mut picker = DatePicker::new();
    // 固定可预测的视图月份。
    open_august_2026(&picker);
    // 锚点靠近月初，向上移动一周必然跨月。
    picker.commit_value(Date::new(2026, 8, 5));

    // Up 移动跨月到七月。
    let _ = EventHandler::on_event(
        &mut picker,
        &SystemEvent::KeyDown {
            key: KeyCode::Up,
            mods: KeyMod::NONE,
        },
    );
    // 焦点日期落到七月末。
    assert_eq!(picker.hover_date.get(), Some(Date::new(2026, 7, 29)));
    // 视图月份跟随焦点日期同步。
    assert_eq!((picker.view_year.get(), picker.view_month.get()), (2026, 7));
}

// 打开态 Enter 确认键盘聚焦日期并关闭面板。
#[test]
fn open_picker_enter_confirms_focused_date_and_closes() {
    // 创建并打开日期选择器。
    let mut picker = DatePicker::new();
    // 固定可预测的视图月份。
    open_august_2026(&picker);
    // 键盘聚焦到目标日期。
    picker.hover_date.set(Some(Date::new(2026, 8, 15)));

    // Enter 提交聚焦日期。
    let _ = EventHandler::on_event(
        &mut picker,
        &SystemEvent::KeyDown {
            key: KeyCode::Enter,
            mods: KeyMod::NONE,
        },
    );
    // 值已提交为聚焦日期。
    assert_eq!(picker.current_value(), Date::new(2026, 8, 15));
    // 提交后关闭面板。
    assert!(!picker.open.get());
}

// 打开态 Enter 无聚焦日期时确认当前选中值（与 TimePicker 语义一致）。
#[test]
fn open_picker_enter_without_focus_confirms_current_value() {
    // 创建并打开日期选择器。
    let mut picker = DatePicker::new();
    // 固定可预测的视图月份。
    open_august_2026(&picker);
    // 既有选中值作为 Enter 回退锚点。
    picker.commit_value(Date::new(2026, 8, 20));
    // 打开期间没有键盘聚焦日期。
    picker.hover_date.set(None);

    // Enter 直接确认当前选中值。
    let _ = EventHandler::on_event(
        &mut picker,
        &SystemEvent::KeyDown {
            key: KeyCode::Enter,
            mods: KeyMod::NONE,
        },
    );
    // 值保持不变并已提交。
    assert_eq!(picker.current_value(), Date::new(2026, 8, 20));
    // 提交后关闭面板。
    assert!(!picker.open.get());
}

// 复用父模块中的 AutoComplete 实现与私有方法。
use super::AutoComplete;
// 引入断言所需的点与矩形基础类型。
use crate::core::{Point, Rect};
// 引入受控输入测试所需的响应式状态。
use crate::ui::reactive::state::State;

// 受控绑定必须同步外部文本并接收编辑与候选提交。
#[test]
// 测试名称说明文本状态双向同步契约。
fn controlled_value_reads_edits_selects_and_reconciles() {
    // 初始状态提供待过滤查询文本。
    let value = State::new("ap".to_owned());
    // 构造两个匹配查询的候选项并绑定外部文本。
    let mut autocomplete = AutoComplete::new()
        // 提供稳定的字符串候选集合。
        .options(vec!["apple", "apricot"])
        // 绑定外部输入状态。
        .bind_value(&value);

    // 初始输入必须来自外部状态。
    assert_eq!(autocomplete.value(), "ap");
    // 模拟在光标末尾继续输入一个字符。
    assert!(autocomplete.insert_text("p"));
    // 本地编辑必须立即写回外部状态。
    assert_eq!(value.get(), "app");
    // 选择首个过滤候选必须成功。
    assert!(autocomplete.commit_index(0));
    // 候选提交必须把完整选项写回状态。
    assert_eq!(value.get(), "apple");

    // 模拟业务逻辑从外部切换到另一个候选。
    value.set("apricot".to_owned());
    // 声明式重建必须把最新状态同步进既有实例。
    autocomplete.sync_from(
        // 新声明保持相同候选与绑定句柄。
        AutoComplete::new()
            // 重建候选集合。
            .options(vec!["apple", "apricot"])
            // 重建受控状态绑定。
            .bind_value(&value),
    );
    // 重建后的输入文本必须服从外部状态。
    assert_eq!(autocomplete.value(), "apricot");
}

// 构造指定数量的稳定候选项。
fn options(count: usize) -> Vec<String> {
    // 为每个序号创建不同候选文本。
    (0..count)
        // 将序号映射为候选项。
        .map(|index| format!("候选项{index}"))
        // 收集为 AutoComplete 所需候选列表。
        .collect()
}

// 靠近表面底边时弹层必须翻转并按上方空间缩高。
#[test]
// 测试名称说明纵向表面约束职责。
fn overlay_entry_constrains_popup_near_surface_bottom() {
    // 创建包含二十个候选项的自动完成输入框。
    let mut autocomplete = AutoComplete::new().options(options(20));
    // 打开候选弹层参与登记。
    autocomplete.open();
    // 将触发器放在一百二十像素高表面的底部附近。
    let frame = Rect::new(20.0, 80.0, 120.0, 32.0);
    // 构造不足以容纳自然弹层高度的当前逻辑表面。
    let surface = Rect::new(0.0, 0.0, 240.0, 120.0);
    // 通过组件树使用的显式表面入口创建登记。
    let overlay = crate::ui::component::traits::WidgetRender::overlay_entry_for_surface(
        // 传入被测自动完成组件。
        &autocomplete,
        // 使用稳定的测试组件标识。
        crate::core::ComponentId::new(17),
        // 传入靠近底边的触发器 frame。
        frame,
        // 传入当前逻辑表面。
        surface,
    )
    // 打开状态必须生成浮层登记。
    .expect("打开的自动完成输入框应生成浮层登记")
    // 读取登记的绝对弹层矩形。
    .bounds_rect()
    // 自动完成弹层登记必须声明边界。
    .expect("自动完成弹层应声明边界");

    // 下方空间不足时弹层应完整位于触发器上方。
    assert!(overlay.y + overlay.h <= frame.y);
    // 弹层顶边不得越出当前表面。
    assert!(overlay.y >= surface.y);
    // 弹层底边不得越出当前表面。
    assert!(overlay.y + overlay.h <= surface.y + surface.h);
}

// 触发器靠近窄表面右边缘时弹层必须横向收敛。
#[test]
// 测试名称说明横向表面约束职责。
fn overlay_entry_constrains_popup_to_narrow_surface() {
    // 创建单个候选项的自动完成输入框。
    let mut autocomplete = AutoComplete::new().options(options(1));
    // 打开候选弹层参与登记。
    autocomplete.open();
    // 构造宽于表面且靠近右边界的触发器。
    let frame = Rect::new(70.0, 20.0, 120.0, 32.0);
    // 使用一百像素宽的窄逻辑表面。
    let surface = Rect::new(0.0, 0.0, 100.0, 260.0);
    // 通过显式表面入口创建弹层登记。
    let overlay = crate::ui::component::traits::WidgetRender::overlay_entry_for_surface(
        // 传入被测自动完成组件。
        &autocomplete,
        // 使用稳定的测试组件标识。
        crate::core::ComponentId::new(18),
        // 传入靠近右边界的触发器 frame。
        frame,
        // 传入当前窄表面。
        surface,
    )
    // 打开状态必须生成浮层登记。
    .expect("打开的自动完成输入框应生成浮层登记")
    // 读取登记的绝对弹层矩形。
    .bounds_rect()
    // 自动完成弹层登记必须声明边界。
    .expect("自动完成弹层应声明边界");

    // 弹层左边不得越出当前表面。
    assert!(overlay.x >= surface.x);
    // 弹层右边不得越出当前表面。
    assert!(overlay.x + overlay.w <= surface.x + surface.w);
    // 弹层宽度不得超过当前表面宽度。
    assert!(overlay.w <= surface.w);
}

// 弹层缩高后键盘显露必须使用实际视口而非自然高度。
#[test]
// 测试名称说明缩高视口与虚拟滚动账本的一致性。
fn constrained_popup_height_drives_reveal_scroll_range() {
    // 创建二十个候选项使内容高度达到五百六十像素。
    let mut autocomplete = AutoComplete::new().options(options(20));
    // 打开候选弹层参与表面解析。
    autocomplete.open();
    // 将触发器放在表面底部附近，使上方只有八十像素。
    let frame = Rect::new(20.0, 80.0, 120.0, 32.0);
    // 使用一百二十像素高表面触发向上缩高。
    let surface = Rect::new(0.0, 0.0, 240.0, 120.0);
    // 先通过显式登记入口记录实际受约束视口。
    let _ = crate::ui::component::traits::WidgetRender::overlay_entry_for_surface(
        // 传入被测自动完成组件。
        &autocomplete,
        // 使用稳定的测试组件标识。
        crate::core::ComponentId::new(19),
        // 传入靠近底边的触发器 frame。
        frame,
        // 传入当前逻辑表面。
        surface,
    );
    // 选择最后一个候选项。
    autocomplete.selected_idx = 19;
    // 请求键盘显露当前候选项。
    autocomplete.reveal_selected();

    // 五百六十像素内容减八十像素实际视口应留下四百八十像素范围。
    assert_eq!(autocomplete.dropdown_scroll.scroll_offset(), 480.0);
}

// 弹层翻到上方后行命中必须复用负向本地偏移。
#[test]
// 测试名称说明登记几何与事件映射的一致性。
fn flipped_popup_geometry_drives_row_hit_mapping() {
    // 创建包含二十个候选项的自动完成输入框。
    let mut autocomplete = AutoComplete::new().options(options(20));
    // 打开候选弹层参与表面解析。
    autocomplete.open();
    // 将触发器放在表面底部附近。
    let frame = Rect::new(20.0, 80.0, 120.0, 32.0);
    // 使用一百二十像素高表面迫使弹层向上。
    let surface = Rect::new(0.0, 0.0, 240.0, 120.0);
    // 通过显式表面入口缓存翻转后的本地弹层矩形。
    let _ = crate::ui::component::traits::WidgetRender::overlay_entry_for_surface(
        // 传入被测自动完成组件。
        &autocomplete,
        // 使用稳定的测试组件标识。
        crate::core::ComponentId::new(20),
        // 传入靠近底边的触发器 frame。
        frame,
        // 传入当前逻辑表面。
        surface,
    );

    // 相对触发器原点负七十像素落在向上弹层首行。
    assert_eq!(
        autocomplete.dropdown_row_at(Point::new(10.0, -70.0)),
        Some(0)
    );
}

// 表面缩小时脏区必须覆盖翻转前弹层仍留在新表面的尾部。
#[test]
// 测试名称说明历史绝对弹层与当前表面的交集职责。
fn surface_change_dirty_covers_previous_popup_tail() {
    // 创建足以生成最大自然高度的自动完成输入框。
    let mut autocomplete = AutoComplete::new().options(options(20));
    // 打开候选弹层参与表面解析。
    autocomplete.open();
    // 固定触发器位置，使大表面向下、小表面向上。
    let frame = Rect::new(20.0, 80.0, 120.0, 32.0);
    // 首帧表面允许弹层完整向下展开。
    let large_surface = Rect::new(0.0, 0.0, 240.0, 400.0);
    // 先登记向下展开的旧弹层。
    let _ = crate::ui::component::traits::WidgetRender::overlay_entry_for_surface(
        // 传入被测自动完成组件。
        &autocomplete,
        // 使用稳定的测试组件标识。
        crate::core::ComponentId::new(21),
        // 传入固定触发器 frame。
        frame,
        // 传入可完整向下展开的大表面。
        large_surface,
    );
    // 次帧表面迫使弹层翻到上方并缩高。
    let small_surface = Rect::new(0.0, 0.0, 240.0, 120.0);
    // 刷新登记并累计翻转前后的弹层区域。
    let _ = crate::ui::component::traits::WidgetRender::overlay_entry_for_surface(
        // 传入同一自动完成组件。
        &autocomplete,
        // 沿用同一测试组件标识。
        crate::core::ComponentId::new(21),
        // 触发器位置保持不变。
        frame,
        // 传入缩小后的当前表面。
        small_surface,
    );
    // 读取组件对当前表面声明的完整脏区。
    let dirty = crate::ui::component::traits::WidgetRender::dirty_rect(&autocomplete, frame);

    // 当前向上弹层与触发器从表面顶边开始。
    assert_eq!(dirty.y, small_surface.y);
    // 旧向下弹层在新表面内残留的八像素尾部也必须被清理。
    assert_eq!(dirty.y + dirty.h, small_surface.y + small_surface.h);
}

// 输入过滤缩短候选列表时脏区必须保留过滤前的完整弹层。
#[test]
// 测试名称说明同一呈现周期内候选变化的历史覆盖职责。
fn filtering_shrinks_overlay_but_preserves_previous_popup_damage() {
    // 创建二十个候选项使初始弹层达到最大自然高度。
    let mut candidates = options(20);
    // 将首项改为唯一可被指定查询匹配的文本。
    candidates[0] = "needle".to_string();
    // 使用完整候选集合创建自动完成输入框。
    let mut autocomplete = AutoComplete::new().options(candidates);
    // 打开候选弹层并生成初始过滤结果。
    autocomplete.open();
    // 将触发器放在可完整向下展开的大表面中。
    let frame = Rect::new(20.0, 20.0, 120.0, 32.0);
    // 使用足以容纳最大自然弹层的逻辑表面。
    let surface = Rect::new(0.0, 0.0, 240.0, 400.0);
    // 先登记二十行候选对应的旧弹层。
    let _ = crate::ui::component::traits::WidgetRender::overlay_entry_for_surface(
        // 传入被测自动完成组件。
        &autocomplete,
        // 使用稳定的测试组件标识。
        crate::core::ComponentId::new(22),
        // 传入固定触发器 frame。
        frame,
        // 传入当前逻辑表面。
        surface,
    );
    // 输入查询把候选集合过滤为唯一一行。
    autocomplete.set_value("needle");
    // 刷新浮层登记以取得过滤后的当前几何。
    let overlay = crate::ui::component::traits::WidgetRender::overlay_entry_for_surface(
        // 传入同一自动完成组件。
        &autocomplete,
        // 沿用同一测试组件标识。
        crate::core::ComponentId::new(22),
        // 触发器位置保持不变。
        frame,
        // 表面尺寸保持不变。
        surface,
    )
    // 过滤后组件仍处于打开状态。
    .expect("过滤后的自动完成输入框应继续生成浮层登记")
    // 读取当前一行弹层边界。
    .bounds_rect()
    // 当前浮层必须声明边界。
    .expect("过滤后的自动完成弹层应声明边界");
    // 读取包含历史弹层的完整脏区。
    let dirty = crate::ui::component::traits::WidgetRender::dirty_rect(&autocomplete, frame);

    // 当前 OverlayStack 边界应缩到一行高度。
    assert_eq!(overlay.h, 28.0);
    // 历史脏区仍需覆盖过滤前二百八十像素高弹层的底边。
    assert_eq!(dirty.y + dirty.h, frame.y + frame.h + 280.0);
}

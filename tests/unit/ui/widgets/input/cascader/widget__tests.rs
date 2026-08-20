// 复用被测级联选择组件和选项类型。
use super::{Cascader, CascaderOption};
// 引入命中断言所需的点与矩形基础类型。
use crate::core::{Point, Rect};
// 引入绑定测试所需的状态和值模型。
use crate::ui::{CascaderValue, State};

// 构造指定数量的稳定叶子选项。
fn leaf_options(count: usize) -> Vec<CascaderOption> {
    // 为每个序号创建不同标签和值。
    (0..count)
        // 将序号映射为叶子选项。
        .map(|index| CascaderOption::new(format!("选项{index}"), format!("value-{index}")))
        // 收集为组件构造函数所需列表。
        .collect()
}

// 验证叶路径选择回写声明式业务状态。
#[test]
// 覆盖构造读取和叶选项提交两个方向。
fn value_binding_reads_and_writes_complete_paths() {
    // 准备带既有路径的业务状态。
    let state = State::new(CascaderValue {
        // 初始显示标签由业务状态提供。
        labels: vec!["旧标签".to_owned()],
        // 初始稳定值由业务状态提供。
        values: vec!["old".to_owned()],
    });
    // 创建包含一个可提交叶项的受控级联选择器。
    let mut cascader = Cascader::new(
        // 叶项标签和值相互独立。
        vec![CascaderOption::new("新标签", "new")],
        // 占位文本不影响绑定验证。
        "请选择",
    )
    // 绑定业务状态句柄。
    .value(&state);

    // 构造阶段必须读取既有业务状态。
    assert_eq!(cascader.selected().values, vec!["old".to_owned()]);
    // 选择完整叶路径并触发状态提交。
    cascader.select_option(0, 0);
    // 外部状态必须收到新的稳定值路径。
    assert_eq!(state.get().values, vec!["new".to_owned()]);
    // 外部状态同时保留对应显示标签路径。
    assert_eq!(state.get().labels, vec!["新标签".to_owned()]);
}

// 靠近表面底边时弹层必须翻转并按上方可用空间缩高。
#[test]
// 测试名称说明纵向表面约束职责。
fn overlay_entry_constrains_popup_near_surface_bottom() {
    // 创建包含多行选项的级联选择器。
    let mut cascader = Cascader::new(leaf_options(10), "请选择");
    // 打开级联弹层参与登记。
    cascader.open();
    // 将触发器放在一百二十像素高表面的底部附近。
    let frame = Rect::new(20.0, 80.0, 120.0, 32.0);
    // 构造小于固定弹层高度的当前逻辑表面。
    let surface = Rect::new(0.0, 0.0, 240.0, 120.0);
    // 通过组件树使用的显式表面入口创建登记。
    let overlay = crate::ui::widget_runtime::traits::WidgetRender::overlay_entry_for_surface(
        // 传入被测级联选择组件。
        &cascader,
        // 使用稳定的测试组件标识。
        crate::core::WidgetId::new(8),
        // 传入靠近底边的触发器 frame。
        frame,
        // 传入当前逻辑表面。
        surface,
    )
    // 打开状态必须生成浮层登记。
    .expect("打开的级联选择器应生成浮层登记")
    // 读取登记的绝对弹层矩形。
    .bounds_rect()
    // 级联弹层登记必须声明边界。
    .expect("级联选择弹层应声明边界");

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
    // 创建单列级联选择器。
    let mut cascader = Cascader::new(leaf_options(1), "请选择");
    // 打开级联弹层参与登记。
    cascader.open();
    // 构造宽于表面且靠近右边界的触发器。
    let frame = Rect::new(70.0, 20.0, 120.0, 32.0);
    // 使用一百像素宽的窄逻辑表面。
    let surface = Rect::new(0.0, 0.0, 100.0, 260.0);
    // 通过显式表面入口创建弹层登记。
    let overlay = crate::ui::widget_runtime::traits::WidgetRender::overlay_entry_for_surface(
        // 传入被测级联选择组件。
        &cascader,
        // 使用稳定的测试组件标识。
        crate::core::WidgetId::new(9),
        // 传入靠近右边界的触发器 frame。
        frame,
        // 传入当前窄表面。
        surface,
    )
    // 打开状态必须生成浮层登记。
    .expect("打开的级联选择器应生成浮层登记")
    // 读取登记的绝对弹层矩形。
    .bounds_rect()
    // 级联弹层登记必须声明边界。
    .expect("级联选择弹层应声明边界");

    // 弹层左边不得越出当前表面。
    assert!(overlay.x >= surface.x);
    // 弹层右边不得越出当前表面。
    assert!(overlay.x + overlay.w <= surface.x + surface.w);
    // 弹层宽度不得超过当前表面宽度。
    assert!(overlay.w <= surface.w);
}

// 弹层缩高后滚动范围必须使用实际视口而非固定二百像素。
#[test]
// 测试名称说明缩高视口与滚动账本的一致性。
fn constrained_popup_height_drives_scroll_range() {
    // 创建十行选项使内容高度达到三百二十像素。
    let mut cascader = Cascader::new(leaf_options(10), "请选择");
    // 打开级联弹层参与表面解析。
    cascader.open();
    // 将触发器放在表面底部附近，使上方只有七十八像素。
    let frame = Rect::new(20.0, 80.0, 120.0, 32.0);
    // 使用一百二十像素高表面触发向上缩高。
    let surface = Rect::new(0.0, 0.0, 240.0, 120.0);
    // 先通过显式登记入口记录实际受约束视口。
    let _ = crate::ui::widget_runtime::traits::WidgetRender::overlay_entry_for_surface(
        // 传入被测级联选择组件。
        &cascader,
        // 使用稳定的测试组件标识。
        crate::core::WidgetId::new(10),
        // 传入靠近底边的触发器 frame。
        frame,
        // 传入当前逻辑表面。
        surface,
    );
    // 请求滚动到当前列末端。
    assert!(cascader.scroll_level(0, 1000.0));

    // 三百二十像素内容减七十八像素实际视口应留下二百四十二像素范围。
    assert_eq!(cascader.scroll_offsets[0], 242.0);
}

// 多列总宽被表面压缩后事件映射必须使用实际等分列宽。
#[test]
// 测试名称说明绘制列宽与第二列命中的一致性。
fn constrained_multi_column_width_drives_hit_mapping() {
    // 创建带两项子级的根选项。
    let root = CascaderOption::new("根", "root")
        // 子级在选择根项后形成第二列。
        .children(leaf_options(2));
    // 创建只含该根项的级联选择器。
    let mut cascader = Cascader::new(vec![root], "请选择");
    // 打开级联弹层并初始化根列。
    cascader.open();
    // 选择根项以展开第二列但不关闭弹层。
    cascader.select_option(0, 0);
    // 使用宽一百二十像素的触发器。
    let frame = Rect::new(20.0, 20.0, 120.0, 32.0);
    // 三百像素宽表面不足以容纳两列各二百像素自然宽。
    let surface = Rect::new(0.0, 0.0, 300.0, 260.0);
    // 通过显式表面入口解析并缓存两列最终几何。
    let overlay = crate::ui::widget_runtime::traits::WidgetRender::overlay_entry_for_surface(
        // 传入被测级联选择组件。
        &cascader,
        // 使用稳定的测试组件标识。
        crate::core::WidgetId::new(11),
        // 传入当前触发器绝对 frame。
        frame,
        // 传入当前逻辑表面。
        surface,
    )
    // 打开状态必须生成浮层登记。
    .expect("打开的两列级联选择器应生成浮层登记")
    // 读取最终绝对弹层矩形。
    .bounds_rect()
    // 两列弹层必须声明边界。
    .expect("两列级联弹层应声明边界");

    // 两列总宽应收敛为三百像素表面宽度。
    assert_eq!(overlay.w, 300.0);
    // 使用组件本地坐标命中压缩后的第二列首行。
    let hit = cascader.option_at(
        // 事件路径中的触发器 frame 以组件原点为基准。
        Rect::new(0.0, 0.0, frame.w, frame.h),
        // 横坐标落在实际一百五十像素列宽的第二列。
        Point::new(160.0, 50.0),
    );
    // 命中必须映射到第二层首项而不是自然二百像素宽的第一列。
    assert_eq!(hit, Some((1, 0)));
}

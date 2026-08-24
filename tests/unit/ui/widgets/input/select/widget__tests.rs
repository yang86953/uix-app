// 复用被测选择组件和结构化选项模型。
use super::{OptGroup, Select, SelectOption, VisibleRow};
// 引入布局断言所需的基础几何类型。
use crate::core::{Rect, Size};
// 引入根节点 frame 读写所需的组件核心 trait。
use crate::ui::widget_runtime::widget::WidgetCore;
// 引入直接执行组件布局与设置根表面所需的树接口。
use crate::ui::{LayoutChild, LayoutEngineScratch, WidgetId, WidgetLayout, WidgetTree};

// 分组搜索的无分配遍历必须保持既有组标题与选项顺序。
#[test]
fn visible_row_traversal_preserves_grouped_search_semantics() {
    let mut select = Select::searchable().optgroups(vec![
        OptGroup::new("甲组").add("Alpha").add("Beta"),
        OptGroup::new("乙组").add("Gamma").add("Delta"),
    ]);
    select.search_query = "TA".to_owned();

    let mut rows = Vec::new();
    select.for_each_visible_row(|row| rows.push(row));

    assert_eq!(
        rows,
        vec![
            VisibleRow::Group(0),
            VisibleRow::Option(1),
            VisibleRow::Group(1),
            VisibleRow::Option(3),
        ]
    );
    assert_eq!(select.visible_row_count(), rows.len());
}

// 调用方缓冲入口必须与拥有型兼容入口返回相同的过滤后几何。
#[test]
fn custom_option_layout_buffer_matches_owned_geometry() {
    let mut select = Select::searchable().options((0..20).map(|index| format!("Option {index}")));
    select.open();
    select.search_query = "OPTION 1".to_owned();
    select.custom_option_views = true;
    // 刻意使用非声明顺序，覆盖兼容回退而不影响常态线性扫描。
    select.mark_custom_options_materialized(vec![10, 1]);

    let mut tree = WidgetTree::new();
    tree.set_root(Box::new(Select::new()));
    tree.root_mut()
        .expect("测试组件树应包含根节点")
        .set_frame(Rect::new(0.0, 0.0, 400.0, 400.0));
    let ids = [WidgetId::new(31), WidgetId::new(32)];
    let frame = Rect::new(20.0, 20.0, 180.0, 32.0);

    let owned_measurements = select.measure_children(frame, &ids, &tree);
    let mut reused_measurements = Vec::with_capacity(ids.len());
    select.measure_children_into(frame, &ids, &tree, &mut reused_measurements);
    assert_eq!(reused_measurements.len(), owned_measurements.len());
    for (reused, owned) in reused_measurements.iter().zip(&owned_measurements) {
        assert_eq!(reused.id, owned.id);
        assert_eq!(reused.measured_size, owned.measured_size);
    }

    let owned_layout = select.layout_children(frame, &owned_measurements, &tree);
    let mut reused_layout = Vec::with_capacity(ids.len());
    select.layout_children_into(
        frame,
        &reused_measurements,
        &tree,
        &mut LayoutEngineScratch::default(),
        &mut reused_layout,
    );
    assert_eq!(reused_layout, owned_layout);
    assert_eq!(reused_layout.len(), 2);
    // Option 1 是首行，Option 10 是次行；输出仍按物化子项顺序排列。
    assert!(reused_layout[0].1.y > reused_layout[1].1.y);
}

// 固有宽度只在文案配置变化后失效并重新计算。
#[test]
fn intrinsic_width_cache_invalidates_when_labels_change() {
    let select = Select::new().options(["短"]);
    let short = select.intrinsic_size();
    assert_eq!(select.intrinsic_width.get(), Some(short.w));

    let select = select.options(["一段明显更长的选项文案"]);
    assert_eq!(select.intrinsic_width.get(), None);
    let long = select.intrinsic_size();
    assert!(long.w > short.w);
    assert_eq!(select.intrinsic_width.get(), Some(long.w));
}

// 验证显示文案、状态值和快照观测保持各自契约。
#[test]
// 覆盖结构化选项的读取与观测路径。
fn structured_options_keep_labels_separate_from_bound_values() {
    // 使用不同的显示文案和稳定值构造两个选项。
    let mut select = Select::new().select_options([
        // 中文文案映射到稳定地区代码。
        SelectOption::new("中国", "cn"),
        // 另一项文案映射到另一个稳定地区代码。
        SelectOption::new("美国", "us"),
    ]);
    // 选择第二项以覆盖读取稳定值的路径。
    select.select_single(1);

    // 当前业务值必须是稳定值，而不是显示文案。
    assert_eq!(select.current_value().as_deref(), Some("us"));
    // 绘制和搜索入口必须仍然返回显示文案。
    assert_eq!(select.option_label(1), Some("美国"));
    // 快照必须保留既有的显示文案观测语义。
    let crate::ui::SnapshotFields::Select { options, .. } = select.snapshot_fields() else {
        // 选择器只能生成选择器快照分支。
        panic!("选择器应生成 Select 快照");
    };
    // 快照列表不应暴露内部稳定值。
    assert_eq!(options, vec!["中国".to_owned(), "美国".to_owned()]);
}

// 自定义选项必须在绘制前的布局阶段直接使用组件树根表面。
#[test]
// 测试名称说明布局阶段的同帧表面约束职责。
fn custom_option_layout_uses_current_tree_surface() {
    // 创建三行自定义选项以产生八十四像素自然弹层。
    let mut select = Select::new().options(["一", "二", "三"]);
    // 打开选择弹层参与子项布局。
    select.open();
    // 模拟声明了自定义选项渲染器的组件状态。
    select.custom_option_views = true;
    // 模拟当前仅物化首个可见自定义选项。
    select.mark_custom_options_materialized(vec![0]);
    // 将控件放在一百二十像素高表面的底部附近。
    let frame = Rect::new(20.0, 80.0, 120.0, 32.0);
    // 创建组件树作为布局阶段的表面来源。
    let mut tree = WidgetTree::new();
    // 放入一个最小根组件以承载窗口表面 frame。
    tree.set_root(Box::new(Select::new()));
    // 取得刚创建的根节点并设置当前逻辑表面。
    tree.root_mut()
        // 测试树必须包含刚设置的根节点。
        .expect("测试组件树应包含根节点")
        // 将根节点布局结果设置为当前窗口表面。
        .set_frame(Rect::new(0.0, 0.0, 200.0, 120.0));
    // 创建与物化索引一一对应的自定义选项布局描述。
    let child = LayoutChild::new(WidgetId::new(7), Size::new(40.0, 20.0));
    // 在尚未执行绘制的情况下直接运行选择组件子项布局。
    let positions = select.layout_children(frame, &[child], &tree);
    // 当前物化的一项必须获得唯一布局结果。
    assert_eq!(positions.len(), 1);
    // 读取首项最终绝对布局矩形。
    let option = positions[0].1;

    // 底边空间不足时首项应随弹层翻转到控件上方。
    assert!(option.y < frame.y);
    // 受约束后的首项不得越出根表面顶边。
    assert!(option.y >= 0.0);
    // 布局阶段应已经缓存缩高到八十像素的实际弹层。
    assert_eq!(select.dropdown_rect.get().h, 80.0);
}

// 过滤后实际弹层与保守脏区翻转方向不同时必须同时覆盖上下两侧。
#[test]
// 测试名称说明过滤状态切换时的重绘覆盖职责。
fn dirty_popup_covers_current_and_conservative_directions() {
    // 创建十行可搜索选项使保守弹层高度超过任一侧空间。
    let mut select = Select::searchable().options([
        // 唯一匹配项用于形成一行实际弹层。
        "匹配", // 其余九项用于扩大过滤前保守高度。
        "二",   // 保留第三个不匹配选项。
        "三",   // 保留第四个不匹配选项。
        "四",   // 保留第五个不匹配选项。
        "五",   // 保留第六个不匹配选项。
        "六",   // 保留第七个不匹配选项。
        "七",   // 保留第八个不匹配选项。
        "八",   // 保留第九个不匹配选项。
        "九",   // 保留第十个不匹配选项。
        "十",
    ]);
    // 打开选择弹层参与脏区解析。
    select.open();
    // 输入只匹配首项的搜索词。
    select.search_query = "匹配".to_owned();
    // 将控件放在表面中部偏下，使短弹层向下而保守弹层向上。
    let frame = Rect::new(20.0, 150.0, 120.0, 32.0);
    // 使用三百像素高表面制造两个不同放置方向。
    let surface = Rect::new(0.0, 0.0, 240.0, 300.0);
    // 解析同时服务当前状态与状态切换的局部脏区。
    let damage = select.dropdown_damage_rect(frame, surface);

    // 保守弹层必须覆盖控件上方区域。
    assert!(damage.y < 0.0);
    // 当前一行弹层必须仍缓存为控件下方二十八像素视口。
    assert_eq!(
        select.dropdown_rect.get(),
        Rect::new(0.0, 32.0, 120.0, 28.0)
    );
    // 合并脏区必须覆盖当前向下弹层的完整底边。
    assert!(damage.y + damage.h >= 60.0);
}

// 靠近表面底边时，弹层必须翻转或缩高后完整留在表面内。
#[test]
// 测试名称说明纵向表面约束职责。
fn overlay_entry_constrains_popup_near_surface_bottom() {
    // 创建三行选项以产生八十四像素自然弹层。
    let mut select = Select::new().options(["一", "二", "三"]);
    // 打开选择弹层参与登记。
    select.open();
    // 将控件放在一百二十像素表面的底部附近。
    let frame = Rect::new(20.0, 80.0, 120.0, 32.0);
    // 构造比自然弹层更矮的当前逻辑表面。
    let surface = Rect::new(0.0, 0.0, 200.0, 120.0);
    // 通过组件树使用的显式表面入口创建登记。
    let overlay = crate::ui::widget_runtime::traits::WidgetRender::overlay_entry_for_surface(
        // 传入被测选择组件。
        &select,
        // 使用稳定的测试组件标识。
        crate::core::WidgetId::new(5),
        // 传入靠近底边的控件 frame。
        frame,
        // 传入当前逻辑表面。
        surface,
    )
    // 打开状态必须产生浮层登记。
    .expect("打开的选择器应生成浮层登记")
    // 读取登记的绝对弹层矩形。
    .bounds_rect()
    // 选择弹层登记必须声明边界。
    .expect("选择弹层应声明边界");

    // 下方空间不足时弹层应位于控件上方。
    assert!(overlay.y + overlay.h <= frame.y);
    // 最终弹层不得越出当前表面底边。
    assert!(overlay.y + overlay.h <= surface.y + surface.h);
    // 最终弹层不得越出当前表面顶边。
    assert!(overlay.y >= surface.y);
}

// 控件靠近窄表面右边缘时，弹层必须横向收敛到表面内。
#[test]
// 测试名称说明横向表面约束职责。
fn overlay_entry_constrains_popup_to_narrow_surface() {
    // 创建一行选项以保持纵向场景简单。
    let mut select = Select::new().options(["一"]);
    // 打开选择弹层参与登记。
    select.open();
    // 构造宽于表面且靠近右边缘的控件。
    let frame = Rect::new(70.0, 20.0, 120.0, 32.0);
    // 使用一百像素宽的窄逻辑表面。
    let surface = Rect::new(0.0, 0.0, 100.0, 120.0);
    // 通过组件树使用的显式表面入口创建登记。
    let overlay = crate::ui::widget_runtime::traits::WidgetRender::overlay_entry_for_surface(
        // 传入被测选择组件。
        &select,
        // 使用稳定的测试组件标识。
        crate::core::WidgetId::new(6),
        // 传入靠近右边缘的控件 frame。
        frame,
        // 传入当前逻辑表面。
        surface,
    )
    // 打开状态必须产生浮层登记。
    .expect("打开的选择器应生成浮层登记")
    // 读取登记的绝对弹层矩形。
    .bounds_rect()
    // 选择弹层登记必须声明边界。
    .expect("选择弹层应声明边界");

    // 最终弹层不得越出当前表面左边。
    assert!(overlay.x >= surface.x);
    // 最终弹层不得越出当前表面右边。
    assert!(overlay.x + overlay.w <= surface.x + surface.w);
    // 最终弹层宽度不得超过当前表面。
    assert!(overlay.w <= surface.w);
}

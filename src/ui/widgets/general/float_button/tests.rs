// 引入父模块的私有契约与公开构建器。
use super::*;
// 引入表面安全边距常量以验证 overlay 所有权。
use super::geometry::FLOAT_BUTTON_SURFACE_INSET;
// 引入真实声明树构建与语义点击载荷以验证事件所有权。
use crate::ui::adapter::ViewAdapter;
// 引入组件事件与渲染窄契约。
use crate::ui::component::traits::{EventHandler, WidgetRender};
// 引入运行时节点直接子项读取契约。
use crate::ui::component::widget::WidgetCore;
// 引入完整子声明节点以调用保留事件的组构建入口。
use crate::ui::view::ViewNode;
// 引入标准点击事件契约与键盘修饰状态。
use crate::ui::{ClickEvent, KeyMod, SemanticEvent};

// 验证四角 placement、窗口缩放和兼容 frame-relative 路径。
#[test]
fn placement_uses_current_surface_without_breaking_legacy_position() {
    // 构造带非零原点的窗口逻辑表面。
    let surface = Rect::new(10.0, 20.0, 200.0, 160.0);
    // 使用零布局槽隔离窗口锚定语义。
    let frame = Rect::zero();
    // 列出四个文档化角落及其预期起点。
    for (placement, expected_x, expected_y) in [
        // 左上角保留安全边距。
        (Placement::TopLeft, 34.0, 44.0),
        // 左下角保留安全边距。
        (Placement::BottomLeft, 34.0, 116.0),
        // 右上角保留安全边距。
        (Placement::TopRight, 146.0, 44.0),
        // 右下角保留安全边距。
        (Placement::BottomRight, 146.0, 116.0),
    ] {
        // 构造显式窗口 placement 按钮。
        let button = FloatButton::new("plus").placement(placement);
        // 使用本帧表面解析共享几何。
        let geometry = button.geometry_for_surface(frame, surface);
        // 横向起点必须匹配 overlay Placement。
        assert_eq!(geometry.control.x, expected_x);
        // 纵向起点必须匹配 overlay Placement。
        assert_eq!(geometry.control.y, expected_y);
        // 无说明时保持圆形直径。
        assert_eq!(geometry.control.w, 40.0);
        // 无说明时保持圆形直径。
        assert_eq!(geometry.control.h, 40.0);
    }
    // 构造右下角按钮以覆盖 surface resize。
    let resized = FloatButton::new("plus").placement(Placement::BottomRight);
    // 使用更小的当前逻辑表面重新解析。
    let resized_geometry = resized.geometry_for_surface(frame, Rect::new(0.0, 0.0, 100.0, 80.0));
    // 横向起点必须相对新表面重算。
    assert_eq!(resized_geometry.control.x, 36.0);
    // 纵向起点必须相对新表面重算。
    assert_eq!(resized_geometry.control.y, 16.0);
    // 构造未显式 placement 的旧式 frame-relative 按钮。
    let legacy = FloatButton::new("plus").position(3.0, 4.0);
    // 传入有效表面也不能改变旧式调用语义。
    let legacy_geometry = legacy.geometry_for_surface(Rect::new(5.0, 7.0, 0.0, 0.0), surface);
    // 横向位置继续等于 frame 起点加作者偏移。
    assert_eq!(legacy_geometry.control.x, 8.0);
    // 纵向位置继续等于 frame 起点加作者偏移。
    assert_eq!(legacy_geometry.control.y, 11.0);
    // 常量本身必须保持文档化安全距离。
    assert_eq!(FLOAT_BUTTON_SURFACE_INSET, 24.0);
}

// 验证 description、徽标、命中、damage 与 overlay 共用同一几何。
#[test]
fn description_badge_hit_damage_and_overlay_share_geometry() {
    // 构造带完整 authored config 的窗口锚定按钮。
    let button = FloatButton::new("message")
        // 增加展开说明以扩大交互区域。
        .description("意见反馈")
        // 增加提示框以扩大绘制损伤区域。
        .tooltip("打开反馈")
        // 增加数字值以验证 dot 优先语义。
        .badge(7)
        // 启用圆点徽标。
        .badge_dot(true)
        // 锚定到窗口右上角。
        .placement(Placement::TopRight);
    // 使用稳定布局槽。
    let frame = Rect::zero();
    // 使用足以容纳完整控件的窗口表面。
    let surface = Rect::new(0.0, 0.0, 320.0, 180.0);
    // 直接计算预期共享几何。
    let expected = button.geometry_for_surface(frame, surface);
    // 通过组件树真实入口登记当前表面。
    let overlay = WidgetRender::overlay_entry_for_surface(
        // 借用被测按钮。
        &button,
        // 使用稳定测试组件标识。
        crate::core::ComponentId::new(9),
        // 传入组件 frame。
        frame,
        // 传入当前逻辑表面。
        surface,
    )
    // 非占位 FloatButton 必须登记 Custom overlay。
    .expect("窗口浮动按钮应登记 overlay");
    // overlay 命中区域必须等于共享控件区域。
    assert_eq!(overlay.bounds_rect(), Some(expected.control));
    // 事件命中入口必须复用刚缓存的共享控件区域。
    assert_eq!(
        EventHandler::hit_test_frame(&button, frame),
        expected.control
    );
    // damage 入口必须复用刚缓存的共享绘制区域。
    assert_eq!(
        WidgetRender::dirty_rect(&button, frame),
        expected.paint_bounds
    );
    // 展开说明必须扩大控件宽度。
    assert!(expected.control.w > expected.control.h);
    // 说明文字必须拥有非空绘制区域。
    assert!(expected.description.is_some_and(|rect| rect.w > 0.0));
    // 圆点徽标必须使用紧凑直径。
    assert_eq!(expected.badge.map(|rect| rect.w), Some(8.0));
    // 提示框必须保持在当前表面内。
    assert!(
        expected
            // 借用可选提示框。
            .tooltip
            // 验证提示框与表面的交集仍等于自身。
            .is_some_and(|rect| rect.intersect(&surface) == Some(rect))
    );
}

// 验证 authored config 能通过 snapshot 与 reconcile 同步完整保留。
#[test]
fn snapshot_sync_and_accessibility_preserve_new_contract_fields() {
    // 构造包含全部新增字段的下一帧组件。
    let next = FloatButton::new("message")
        // 设置首选无障碍说明。
        .description("反馈入口")
        // 设置次选无障碍提示。
        .tooltip("打开反馈")
        // 设置数字徽标。
        .badge(12)
        // 同时设置圆点徽标以验证状态优先级。
        .badge_dot(true)
        // 设置窗口放置方向。
        .placement(Placement::BottomLeft)
        // 设置有限作者偏移。
        .position(2.0, -3.0);
    // 在移动组件前保存期望快照。
    let expected = next.snapshot_fields();
    // 构造具有不同初始 authored config 的当前组件。
    let mut current = FloatButton::new("plus");
    // 执行真实 reconcile 字段同步。
    current.sync_from(next);
    // 同步后的快照必须与下一帧快照完全一致。
    assert_eq!(current.snapshot_fields(), expected);
    // 分解快照以核对每个新增字段。
    match &expected {
        // 匹配 FloatButton authored config。
        SnapshotFields::FloatButton {
            // 借用展开说明。
            description,
            // 借用数字徽标。
            badge_count,
            // 借用圆点徽标。
            badge_dot,
            // 借用可选窗口 placement。
            placement,
            // 忽略已由完整相等断言覆盖的其余字段。
            ..
        } => {
            // 展开说明必须保持不变。
            assert_eq!(description, "反馈入口");
            // 数字徽标必须保持不变。
            assert_eq!(*badge_count, 12);
            // 圆点徽标必须保持不变。
            assert!(*badge_dot);
            // placement 必须保持不变。
            assert_eq!(*placement, Some(Placement::BottomLeft));
        }
        // 其他变体表示快照路由错误。
        _ => panic!("FloatButton 必须生成对应 authored config 快照"),
    }
    // 把 authored config 投影为无障碍快照。
    let accessibility = expected.accessibility();
    // 展开说明必须优先成为可读名称。
    assert_eq!(accessibility.name.as_deref(), Some("反馈入口"));
    // 圆点徽标必须转成非视觉状态文案。
    assert_eq!(accessibility.state.value_text.as_deref(), Some("有新通知"));
}

// 验证非有限作者输入与组内 description 命中边界。
#[test]
fn non_finite_values_are_stable_and_group_includes_description_hit_bounds() {
    // 构造包含非有限尺寸与偏移的按钮。
    let sanitized = FloatButton::new("plus")
        // 非有限尺寸必须回退为默认直径。
        .size(f32::NAN)
        // 非有限偏移必须回退为零。
        .position(f32::INFINITY, f32::NEG_INFINITY)
        // 显式设置右下角窗口 placement。
        .placement(Placement::BottomRight);
    // 使用有效表面解析清洗后的几何。
    let sanitized_geometry = sanitized.geometry_for_surface(
        // 使用零布局槽。
        Rect::zero(),
        // 使用稳定窗口表面。
        Rect::new(0.0, 0.0, 100.0, 100.0),
    );
    // 默认直径必须恢复为四十。
    assert_eq!(sanitized_geometry.control.w, 40.0);
    // 清洗后的控件必须保持有限横坐标。
    assert!(sanitized_geometry.control.x.is_finite());
    // 清洗后的控件必须保持有限纵坐标。
    assert!(sanitized_geometry.control.y.is_finite());
    // 构造带 description 的组内按钮。
    let mut group = FloatButtonGroup::new().buttons(vec![
        // placement 在组内被父布局覆盖，description 仍扩大命中区域。
        FloatButton::new("edit")
            // 设置展开说明。
            .description("编辑")
            // 设置一个应被组内布局忽略的窗口 placement。
            .placement(Placement::TopRight),
    ]);
    // 打开组并建立展开动画。
    group.open();
    // 把动画推进到完全展开。
    group.transition.update(1.0);
    // 计算父组件交互边界。
    let bounds = group.interaction_bounds(Rect::zero());
    // description 必须让父级命中宽度大于基础直径。
    assert!(bounds.w > 40.0);
}

// 验证 FloatButtonGroup 组合完整子 View 时保留标准点击处理器与父级几何。
#[test]
fn group_button_views_preserve_child_click_handler_and_parent_layout() {
    // 建立跨事件回调共享的点击计数。
    let clicks = Rc::new(Cell::new(0));
    // 为子 View 闭包克隆独立共享句柄。
    let child_clicks = Rc::clone(&clicks);
    // 构造点击触发组，并让直接 FloatButton View 独占标准点击处理器。
    let group = FloatButtonGroup::new()
        // 保持父组件只处理组开关触发语义。
        .trigger(TriggerMode::Click)
        // 传入完整 ViewNode，覆盖 handler 不被重建的公开入口。
        .button_views(vec![
            ViewNode::leaf(
                // 使用带说明的按钮验证父级几何仍由子配置派生。
                FloatButton::new("edit").description("编辑"),
            )
            // 把业务点击处理器登记在子节点而不是组父节点。
            .on_click_fn(move || {
                // 累加计数以证明真实 HandlerTable 已执行子处理器。
                child_clicks.set(child_clicks.get() + 1);
            }),
        ]);
    // 通过真实 ViewAdapter 发布父子组件和 HandlerTable。
    let mut tree = ViewAdapter::build(group);
    // 取得稳定根组件标识。
    let root = tree.root_id().expect("FloatButtonGroup 根必须存在");
    // 读取组的唯一直接子按钮标识。
    let child = tree.get(root).expect("组根必须可读取").children()[0];
    // 父组件必须从子 authored config 派生一条布局记录。
    let runtime_group = tree
        // 读取运行时父节点。
        .get(root)
        // 父节点在建树完成后必须仍然存在。
        .expect("组根必须可读取")
        // 借用父组件对象。
        .component()
        // 取得运行时类型视图。
        .as_any()
        // 窄化为 FloatButtonGroup。
        .downcast_ref::<FloatButtonGroup>()
        // 类型变化表示包装器发布了错误根组件。
        .expect("根组件必须是 FloatButtonGroup");
    // 父布局记录必须与直接子节点数量一致。
    assert_eq!(runtime_group.item_layouts.len(), 1);
    // 子组件必须被标记为组内布局参与者。
    let runtime_button = tree
        // 读取运行时子节点。
        .get(child)
        // 子节点在建树完成后必须存在。
        .expect("组内按钮必须可读取")
        // 借用子组件对象。
        .component()
        // 取得运行时类型视图。
        .as_any()
        // 窄化为 FloatButton。
        .downcast_ref::<FloatButton>()
        // 类型变化表示直接子根验证失效。
        .expect("直接子组件必须是 FloatButton");
    // placement 覆盖标记必须在发布前完成。
    assert!(runtime_button.in_group);
    // 向真实子 ComponentId 分发标准主键点击。
    let result = tree.dispatch_semantic(SemanticEvent::click(
        // 点击目标是子按钮，不是组父节点。
        child,
        // 构造标准主键点击载荷。
        ClickEvent {
            // 使用左键满足 on_click_fn 的主点击过滤。
            button: MouseButton::Left,
            // 语义分发不依赖布局位置，使用稳定零点。
            pos: Point::new(0.0, 0.0),
            // 本次点击不携带修饰键。
            modifiers: KeyMod::NONE,
        },
    ));
    // 子 HandlerTable 必须报告已处理。
    assert_eq!(result, EventResult::Handled);
    // 业务处理器必须且只执行一次。
    assert_eq!(clicks.get(), 1);
}

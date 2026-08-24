// 引入父模块的私有契约与公开构建器。
use super::*;
// 引入表面安全边距常量以验证 overlay 所有权。
use super::geometry::FLOAT_BUTTON_SURFACE_INSET;
// 引入真实声明树构建与语义点击载荷以验证事件所有权。
use crate::ui::adapter::ViewAdapter;
// 引入组件事件与渲染窄契约。
use crate::ui::widget_runtime::traits::{EventHandler, WidgetRender};
// 引入运行时节点直接子项读取契约。
use crate::ui::widget_runtime::widget::WidgetCore;
// 引入完整子声明节点以调用保留事件的组构建入口。
use crate::ui::view::ViewNode;
// 引入标准点击事件契约与键盘修饰状态。
use crate::ui::{ClickEvent, KeyMod, SemanticEvent};
// 引入根替换时使用的无业务处理器叶组件。
use crate::ui::widgets::Label;

// 构造带稳定 key、独立点击计数与释放探针的组内按钮 View。
fn tracked_group_button(
    // 接收按钮图标名称。
    icon: &str,
    // 接收同父级协调身份。
    key: &str,
    // 接收该按钮独占的点击计数。
    clicks: Rc<Cell<usize>>,
    // 接收只由处理器闭包继续持有的释放探针。
    lifetime: Rc<()>,
    // 返回保留全部声明事件与身份的 ViewNode。
) -> ViewNode {
    // 构造组内按钮声明并登记稳定协调 key。
    ViewNode::leaf(FloatButton::new(icon))
        // 同 key reconcile 必须保留对应 WidgetId。
        .key(key)
        // 无 State 闭包按公开契约在每次 reconcile 时保守替换。
        .on_click_fn(move || {
            // 读取探针以确保闭包真实拥有其强引用。
            let _lifetime = &lifetime;
            // 只更新当前按钮自己的计数器。
            clicks.set(clicks.get() + 1);
        })
}

// 通过真实指针命中与默认语义生成路径点击指定窗口坐标。
fn pointer_click(tree: &mut WidgetTree, pos: Point) {
    // 左键按下必须被命中的交互组件处理。
    let down = tree.dispatch_event(&SystemEvent::PointerDown {
        // 使用目标组件当前窗口坐标。
        pos,
        // 标准业务点击使用鼠标左键。
        button: MouseButton::Left,
        // 本次输入不携带修饰键。
        mods: KeyMod::NONE,
    });
    // 按下路径必须报告已处理。
    assert_eq!(down, EventResult::Handled);
    // 在同一目标坐标释放以生成标准 Click 语义事件。
    let up = tree.dispatch_event(&SystemEvent::PointerUp {
        // 释放位置与按下位置保持一致。
        pos,
        // 使用配对的鼠标左键。
        button: MouseButton::Left,
        // 本次输入不携带修饰键。
        mods: KeyMod::NONE,
    });
    // 释放路径必须报告已处理。
    assert_eq!(up, EventResult::Handled);
}

// 直接向稳定子身份发送主点击，用于隔离 reconcile 后的 HandlerTable 绑定。
fn semantic_click(tree: &mut WidgetTree, target: crate::core::WidgetId) -> EventResult {
    // 通过树拥有的语义路由表分发主点击。
    tree.dispatch_semantic(SemanticEvent::click(
        // 目标必须是当前或已经失效的具体子 WidgetId。
        target,
        // 构造不依赖布局坐标的标准主点击载荷。
        ClickEvent {
            // 左键满足 on_click_fn 主点击过滤器。
            button: MouseButton::Left,
            // 语义测试使用稳定零点。
            pos: Point::new(0.0, 0.0),
            // 本次输入不携带修饰键。
            modifiers: KeyMod::NONE,
        },
    ))
}

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
        crate::core::WidgetId::new(9),
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

// 验证浮动按钮组以单次前缀累加保持异构尺寸的原有几何。
#[test]
fn group_child_frames_accumulate_prefix_once_without_geometry_drift() {
    // 使用三种直径让前缀偏移可精确区分。
    let group = FloatButtonGroup::new().buttons(vec![
        // 首项使用 32 像素直径。
        FloatButton::new("a").size(32.0),
        // 次项使用 40 像素直径。
        FloatButton::new("b").size(40.0),
        // 末项使用 48 像素直径。
        FloatButton::new("c").size(48.0),
    ]);
    // 完全展开时收集线性迭代器产生的全部 frame。
    let frames = group
        // 使用非零父 frame 证明坐标原点也被保留。
        .child_frames(Rect::new(3.0, 10.0, 40.0, 40.0), 1.0)
        // 测试只关心实际子项 frame。
        .map(|(_item, frame)| frame)
        // 物化稳定顺序便于精确断言。
        .collect::<Vec<_>>();
    // 线性迭代不得丢失子项。
    assert_eq!(frames.len(), 3);
    // 首项在触发器与固定间距之后开始。
    assert_eq!(frames[0], Rect::new(7.0, 58.0, 32.0, 32.0));
    // 次项只累加首项直径与一个间距。
    assert_eq!(frames[1], Rect::new(3.0, 98.0, 40.0, 40.0));
    // 末项再累加次项直径与一个间距。
    assert_eq!(frames[2], Rect::new(-1.0, 146.0, 48.0, 48.0));
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
        .widget()
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
        .widget()
        // 取得运行时类型视图。
        .as_any()
        // 窄化为 FloatButton。
        .downcast_ref::<FloatButton>()
        // 类型变化表示直接子根验证失效。
        .expect("直接子组件必须是 FloatButton");
    // placement 覆盖标记必须在发布前完成。
    assert!(runtime_button.in_group);
    // 向真实子 WidgetId 分发标准主键点击。
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

// 验证组内子事件在命中、协调、移除、换根与关闭期间的完整所有权生命周期。
#[test]
fn group_child_handlers_reconcile_remove_replace_and_shutdown_cleanly() {
    // 建立两个初始子按钮各自独占的点击计数。
    let first_initial_clicks = Rc::new(Cell::new(0));
    // 建立第二个初始子按钮的独立点击计数。
    let second_initial_clicks = Rc::new(Cell::new(0));
    // 建立仅由首个旧处理器持有的释放探针。
    let first_initial_lifetime = Rc::new(());
    // 保存首个旧处理器释放状态的弱观察句柄。
    let first_initial_weak = Rc::downgrade(&first_initial_lifetime);
    // 建立仅由第二个旧处理器持有的释放探针。
    let second_initial_lifetime = Rc::new(());
    // 保存第二个旧处理器释放状态的弱观察句柄。
    let second_initial_weak = Rc::downgrade(&second_initial_lifetime);
    // 构造父组与两个源码顺序稳定的直接子按钮。
    let initial_group = FloatButtonGroup::new()
        // 使用点击触发以覆盖父组件真实展开命中路径。
        .trigger(TriggerMode::Click)
        // 每个子 View 独占业务处理器、key 与生命周期探针。
        .button_views(vec![
            // 首项使用稳定 first 身份。
            tracked_group_button(
                // 首项图标不参与身份判定。
                "edit",
                // 首项同父级 key。
                "first",
                // 克隆首项计数供闭包独占。
                Rc::clone(&first_initial_clicks),
                // 把首项探针强引用移交给闭包。
                first_initial_lifetime,
            ),
            // 第二项使用稳定 second 身份。
            tracked_group_button(
                // 第二项图标不参与身份判定。
                "share",
                // 第二项同父级 key。
                "second",
                // 克隆第二项计数供闭包独占。
                Rc::clone(&second_initial_clicks),
                // 把第二项探针强引用移交给闭包。
                second_initial_lifetime,
            ),
        ]);
    // 发布真实父子树与 HandlerTable 绑定。
    let mut tree = ViewAdapter::build(initial_group);
    // 完成首轮布局以建立父触发器与收起子项 frame。
    tree.layout();
    // 取得稳定父组身份。
    let root = tree.root_id().expect("FloatButtonGroup 根必须存在");
    // 保存初始两个子按钮身份以验证同 key 协调与移除失效。
    let initial_children = tree
        // 读取父组运行时节点。
        .get(root)
        // 初始建树后父组必须存在。
        .expect("FloatButtonGroup 根必须可读取")
        // 复制直接子身份，释放树借用后再分发事件。
        .children()
        // 转成独立向量。
        .to_vec();
    // 初始组必须包含两个直接子按钮。
    assert_eq!(initial_children.len(), 2);
    // 在父触发器中心执行真实指针点击以启动展开。
    pointer_click(&mut tree, Point::new(20.0, 20.0));
    // 父组件必须已经取得展开运行态所有权。
    assert!(
        tree
            // 读取当前根节点。
            .get(root)
            // 根节点必须仍然存在。
            .expect("展开后父组必须存在")
            // 借用父组件。
            .widget()
            // 取得类型视图。
            .as_any()
            // 窄化为 FloatButtonGroup。
            .downcast_ref::<FloatButtonGroup>()
            // 根类型必须保持不变。
            .expect("展开根必须是 FloatButtonGroup")
            // 读取父组件独占的展开状态。
            .expanded
    );
    // 把父组过渡推进到完全展开以开放子项命中。
    let _ = tree.update_animations(1.0);
    // 重新布局展开后的直接子按钮。
    tree.layout();
    // 读取首项展开后的窗口 frame。
    let first_frame = tree
        // 读取首项运行时节点。
        .get(initial_children[0])
        // 首项必须仍然存在。
        .expect("首个组内按钮必须存在")
        // 读取布局完成的 frame。
        .frame();
    // 读取第二项展开后的窗口 frame。
    let second_frame = tree
        // 读取第二项运行时节点。
        .get(initial_children[1])
        // 第二项必须仍然存在。
        .expect("第二个组内按钮必须存在")
        // 读取布局完成的 frame。
        .frame();
    // 通过真实父级扩展命中区域点击首个子按钮。
    pointer_click(
        // 复用同一运行时树。
        &mut tree,
        // Rect 没有中心 helper，显式计算首项中心窗口坐标。
        Point::new(
            // 横坐标取 frame 中点。
            first_frame.x + first_frame.w * 0.5,
            // 纵坐标取 frame 中点。
            first_frame.y + first_frame.h * 0.5,
        ),
    );
    // 通过真实父级扩展命中区域点击第二个子按钮。
    pointer_click(
        // 复用同一运行时树。
        &mut tree,
        // 显式计算第二项中心窗口坐标。
        Point::new(
            // 横坐标取 frame 中点。
            second_frame.x + second_frame.w * 0.5,
            // 纵坐标取 frame 中点。
            second_frame.y + second_frame.h * 0.5,
        ),
    );
    // 首项只能触发自己的业务处理器。
    assert_eq!(first_initial_clicks.get(), 1);
    // 第二项只能触发自己的业务处理器。
    assert_eq!(second_initial_clicks.get(), 1);

    // 建立协调后首项的新处理器计数。
    let first_next_clicks = Rc::new(Cell::new(0));
    // 建立协调后第二项的新处理器计数。
    let second_next_clicks = Rc::new(Cell::new(0));
    // 建立只由协调后首项处理器持有的探针。
    let first_next_lifetime = Rc::new(());
    // 保存首项新处理器释放状态的弱观察句柄。
    let first_next_weak = Rc::downgrade(&first_next_lifetime);
    // 建立只由协调后第二项处理器持有的探针。
    let second_next_lifetime = Rc::new(());
    // 保存第二项新处理器释放状态的弱观察句柄。
    let second_next_weak = Rc::downgrade(&second_next_lifetime);
    // 以相同父类型、触发方式、子类型与 key 提交等价结构的新闭包。
    ViewAdapter::reconcile(
        // 协调到现有树以保留 WidgetId。
        &mut tree,
        // 构造等价父组声明。
        FloatButtonGroup::new()
            // 触发方式不变，父展开运行态不应被声明覆盖。
            .trigger(TriggerMode::Click)
            // 用新闭包替换两个子节点的无 State handler。
            .button_views(vec![
                // 首项继续使用 first key。
                tracked_group_button(
                    // 首项 authored 图标保持稳定。
                    "edit",
                    // 首项协调 key 保持稳定。
                    "first",
                    // 克隆新首项计数。
                    Rc::clone(&first_next_clicks),
                    // 移交新首项探针。
                    first_next_lifetime,
                ),
                // 第二项继续使用 second key。
                tracked_group_button(
                    // 第二项 authored 图标保持稳定。
                    "share",
                    // 第二项协调 key 保持稳定。
                    "second",
                    // 克隆新第二项计数。
                    Rc::clone(&second_next_clicks),
                    // 移交新第二项探针。
                    second_next_lifetime,
                ),
            ]),
    );
    // 重新读取协调后的直接子身份。
    let reconciled_children = tree
        // 父组必须原位保留。
        .get(root)
        // 协调成功后根仍可读取。
        .expect("协调后父组必须存在")
        // 复制当前直接子身份。
        .children()
        // 转成独立向量。
        .to_vec();
    // 同 key 同类型的两个子 WidgetId 必须原位保留。
    assert_eq!(reconciled_children, initial_children);
    // 无 State 旧首项闭包必须在协调线性化点释放。
    assert!(first_initial_weak.upgrade().is_none());
    // 无 State 旧第二项闭包必须在协调线性化点释放。
    assert!(second_initial_weak.upgrade().is_none());
    // 新首项闭包必须由当前 HandlerTable 持有。
    assert!(first_next_weak.upgrade().is_some());
    // 新第二项闭包必须由当前 HandlerTable 持有。
    assert!(second_next_weak.upgrade().is_some());
    // 向保留的首项身份分发点击必须调用新闭包。
    assert_eq!(
        semantic_click(&mut tree, reconciled_children[0]),
        EventResult::Handled
    );
    // 向保留的第二项身份分发点击必须调用新闭包。
    assert_eq!(
        semantic_click(&mut tree, reconciled_children[1]),
        EventResult::Handled
    );
    // 新首项闭包必须执行一次。
    assert_eq!(first_next_clicks.get(), 1);
    // 新第二项闭包必须执行一次。
    assert_eq!(second_next_clicks.get(), 1);
    // 旧首项计数不能再变化。
    assert_eq!(first_initial_clicks.get(), 1);
    // 旧第二项计数不能再变化。
    assert_eq!(second_initial_clicks.get(), 1);

    // 只为保留的首项建立第三版处理器探针。
    let first_retained_lifetime = Rc::new(());
    // 保存第三版首项处理器释放状态的弱观察句柄。
    let first_retained_weak = Rc::downgrade(&first_retained_lifetime);
    // 协调删除 second 子项并替换保留首项的处理器。
    ViewAdapter::reconcile(
        // 发布到同一父组运行时。
        &mut tree,
        // 父组类型与触发方式保持稳定。
        FloatButtonGroup::new()
            // 继续使用点击触发。
            .trigger(TriggerMode::Click)
            // 只声明 first 子项。
            .button_views(vec![tracked_group_button(
                // 保留首项 authored 图标。
                "edit",
                // 保留首项 key。
                "first",
                // 后续计数继续写入同一首项新计数器。
                Rc::clone(&first_next_clicks),
                // 移交第三版首项处理器探针。
                first_retained_lifetime,
            )]),
    );
    // 第二版首项闭包已被第三版替换，必须释放。
    assert!(first_next_weak.upgrade().is_none());
    // 被移除的第二项闭包必须同步释放。
    assert!(second_next_weak.upgrade().is_none());
    // 保留首项第三版闭包必须仍由树拥有。
    assert!(first_retained_weak.upgrade().is_some());
    // 已移除 second 的旧身份不能再进入 HandlerTable。
    assert_eq!(
        semantic_click(&mut tree, reconciled_children[1]),
        EventResult::NotHandled
    );
    // 用无处理器 Label 替换整组根节点以覆盖组卸载。
    ViewAdapter::reconcile(&mut tree, ViewNode::leaf(Label::new("replacement")));
    // 整组卸载必须释放最后一个子处理器。
    assert!(first_retained_weak.upgrade().is_none());
    // 整组卸载后首项旧身份也不能再分发。
    assert_eq!(
        semantic_click(&mut tree, reconciled_children[0]),
        EventResult::NotHandled
    );

    // 建立只用于 shutdown 释放证明的处理器探针。
    let shutdown_lifetime = Rc::new(());
    // 保存 shutdown 前后处理器存活状态的弱观察句柄。
    let shutdown_weak = Rc::downgrade(&shutdown_lifetime);
    // 构建独立关闭树，避免换根路径替代 shutdown 证据。
    let mut shutdown_tree = ViewAdapter::build(
        // 构造最小点击触发组。
        FloatButtonGroup::new()
            // 父组件使用点击触发。
            .trigger(TriggerMode::Click)
            // 单个子项持有 shutdown 探针。
            .button_views(vec![tracked_group_button(
                // 使用稳定关闭测试图标。
                "close",
                // 使用独立关闭测试 key。
                "shutdown",
                // 点击计数不参与释放断言。
                Rc::new(Cell::new(0)),
                // 把唯一强引用移交给 HandlerTable 闭包。
                shutdown_lifetime,
            )]),
    );
    // 保存关闭前的子身份以验证停止树拒绝旧事件。
    let shutdown_child = shutdown_tree
        // 读取关闭树根身份。
        .root_id()
        // 关闭前根必须存在。
        .and_then(|id| shutdown_tree.get(id))
        // 读取唯一直接子项。
        .and_then(|root| root.children().first().copied())
        // 关闭前必须拥有一个子按钮。
        .expect("shutdown 测试子按钮必须存在");
    // 关闭前 HandlerTable 必须持有探针强引用。
    assert!(shutdown_weak.upgrade().is_some());
    // 第一次 shutdown 线性化并释放全部树拥有 handler。
    shutdown_tree.shutdown();
    // shutdown 后处理器探针必须已经释放。
    assert!(shutdown_weak.upgrade().is_none());
    // shutdown 后旧 WidgetId 不能再触发语义处理器。
    assert_eq!(
        semantic_click(&mut shutdown_tree, shutdown_child),
        EventResult::NotHandled
    );
    // 重复 shutdown 必须幂等且不能恢复任何绑定。
    shutdown_tree.shutdown();
    // 重复关闭后探针仍保持释放。
    assert!(shutdown_weak.upgrade().is_none());
}

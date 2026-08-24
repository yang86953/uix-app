// 引入被测 Badge 与其私有几何契约。
use super::*;
// 引入布局测试使用的边距和指针坐标。
use crate::core::{EdgeInsets, Point};
// 引入真实按钮指针事件所需的原生输入类型。
use crate::platform::windowing::{KeyMod, MouseButton};
// 引入适配器、组件与布局 trait 入口。
use crate::ui::widget_runtime::traits::{Widget, WidgetLayout};
// 引入测试读取子身份与写入根 frame 所需的组件树核心契约。
use crate::ui::widget_runtime::widget::WidgetCore;
// 引入 Badge 独立状态节点的无障碍角色。
use crate::ui::widget_snapshot::AccessibilityRole;
// 引入真实声明树物化入口。
use crate::ui::adapter::ViewAdapter;
// 引入事件结果、系统事件、ViewNode 与公开按钮。
use crate::ui::{EventResult, SystemEvent, ViewNode};
// 引入公开声明构建契约以验证同目录 UIX 根。
use crate::ui::view::View;
// 引入可定制主题 token 以验证预设颜色延迟解析。
use crate::ui::theme::DesignTokens;
// 引入公开按钮组件与水平组合器。
use crate::ui::widgets::{Button, row};
// 引入点击次数共享单元。
use std::cell::Cell as CounterCell;
// 引入点击处理器共享所有权。
use std::rc::Rc as Shared;

// 验证 UIX 根保持 Badge 内核及其一次性交接子树，不增加运行时包装节点。
#[test]
fn uix_root_preserves_badge_kernel_and_owned_child() {
    // 构造带唯一真实按钮子树的组合徽章。
    let node = View::build(
        Badge::new()
            .count(6)
            .child(ViewNode::leaf(Button::new("通知"))),
    );
    // 声明根本身仍是叶节点；真实子树由 Badge 生命周期端口物化。
    assert!(node.children.is_empty());
    // 根动态类型必须继续是拥有布局、绘制与子树交接机制的 Badge。
    let kernel = node
        .widget
        .as_any()
        .downcast_ref::<Badge>()
        .expect("UIX 根必须保留 Badge 内核");
    // 组合配置与待交接子树必须无损保留在同一内核中。
    assert_eq!(kernel.count, 6);
    assert!(kernel.composite);
    assert!(kernel.child_view.is_some());
    assert_eq!(kernel.visual.layout.marker_diameter, 10.0);
    assert_eq!(kernel.visual.layout.ribbon_height, 24.0);
}

// 验证完整 UIX 视觉表由 Badge 实例共享。
#[test]
fn uix_visual_configuration_is_shared_between_badges() {
    assert!(std::ptr::eq(Badge::new().visual, BADGE_VISUAL_REF));
    let first = View::build(Badge::new().count(1));
    let second = View::build(Badge::new().dot());
    let first = first
        .widget
        .as_any()
        .downcast_ref::<Badge>()
        .expect("第一个 UIX 根必须保留 Badge 内核");
    let second = second
        .widget
        .as_any()
        .downcast_ref::<Badge>()
        .expect("第二个 UIX 根必须保留 Badge 内核");
    assert!(std::ptr::eq(first.visual, second.visual));
    assert!(std::ptr::eq(first.visual, BADGE_VISUAL_REF));
}

// 验证无分配计数缓冲保持零值、边界、上限后缀与最大整数文本。
#[test]
fn stack_count_label_preserves_public_text_contract() {
    // 显示零值时仍必须生成单个零字符。
    assert_eq!(Badge::new().show_zero(true).count_label().as_str(), "0");
    // 未超过默认上限时保留完整数字。
    assert_eq!(Badge::new().count(99).count_label().as_str(), "99");
    // 超过上限时显示截断上限与加号。
    assert_eq!(Badge::new().count(100).count_label().as_str(), "99+");
    // 十位最大正整数与自定义最大值必须完整放入固定缓冲。
    assert_eq!(
        Badge::new()
            .count(i32::MAX)
            .max(i32::MAX)
            .count_label()
            .as_str(),
        "2147483647"
    );
}

// 验证预设颜色跟随主题而任意颜色保持调用方所有权。
#[test]
fn preset_color_resolves_theme_without_rewriting_custom_color() {
    // 从完整亮色主题建立测试 token。
    let mut tokens = DesignTokens::antd_light();
    // 覆写品牌主色以证明蓝色预设不是固定色表。
    tokens.color_primary = Color::from_rgb(1, 2, 3);
    // 覆写默认错误色以验证无显式颜色路径。
    tokens.color_error = Color::from_rgb(4, 5, 6);
    // 使用预设兼容值模拟公开 BadgeColor 构建器结果。
    let preset = Some(BadgeColor::Blue.to_color());
    // 预设标记存在时必须解析当前品牌 token。
    assert_eq!(
        resolve_badge_background(preset, true, &BADGE_VISUAL.palette, &tokens),
        tokens.color_primary
    );
    // 相同数值由任意 Color 输入时仍归调用方所有。
    assert_eq!(
        // 关闭预设标记以模拟 Badge::color(Color)。
        resolve_badge_background(preset, false, &BADGE_VISUAL.palette, &tokens),
        // 原兼容颜色不得被主题重写。
        BadgeColor::Blue.to_color()
    );
    // 未指定颜色时必须使用当前主题错误色。
    assert_eq!(
        resolve_badge_background(None, false, &BADGE_VISUAL.palette, &tokens),
        tokens.color_error
    );
}

// UIX 色表顺序必须逐项对应全部公开预设色与状态色。
#[test]
fn uix_palette_keeps_all_badge_color_and_status_mappings() {
    let tokens = DesignTokens::antd_light();
    let presets = [
        (BadgeColor::Blue, tokens.color_primary),
        (BadgeColor::Green, tokens.color_success),
        (BadgeColor::Orange, PrimaryHue::Orange.primary()),
        (BadgeColor::Red, tokens.color_error),
        (BadgeColor::Purple, PrimaryHue::Purple.primary()),
    ];
    for (preset, expected) in presets {
        assert_eq!(
            preset.resolve(&BADGE_VISUAL.palette, &tokens),
            expected,
            "{preset:?}"
        );
    }
    let statuses = [
        (BadgeStatus::Success, tokens.color_success),
        (BadgeStatus::Processing, tokens.color_primary),
        (BadgeStatus::Default, tokens.color_text_quaternary),
        (BadgeStatus::Error, tokens.color_error),
        (BadgeStatus::Warning, tokens.color_warning),
    ];
    for (status, expected) in statuses {
        assert_eq!(
            BADGE_VISUAL.palette.status(status).resolve(&tokens),
            expected,
            "{status:?}"
        );
    }
}

// 验证零子节点仍使用旧固有尺寸，而组合模式由真实子外尺寸参与布局。
#[test]
// 测试名称陈述叶兼容与单子正常流契约。
fn zero_child_compatibility_and_single_child_margin_layout() {
    // 构造旧式独立数字徽章。
    let standalone = Badge::new().count(8);
    // 独立徽章仍必须拥有既有胶囊尺寸。
    assert_eq!(
        // 通过布局 trait 测量公开组件。
        WidgetLayout::measure(&standalone, Constraints::unconstrained()).h,
        // 旧数字胶囊高度固定为二十逻辑像素。
        BADGE_VISUAL.layout.pill_height
    );
    // 动态关闭圆点不能残留伪造数字胶囊。
    assert_eq!(Badge::new().dot_when(false).intrinsic_size(), Size::zero());
    // 动态启用圆点仍必须生成标准 marker 尺寸。
    assert_eq!(
        Badge::new().dot_when(true).intrinsic_size(),
        Size::new(
            BADGE_VISUAL.layout.marker_diameter,
            BADGE_VISUAL.layout.marker_diameter,
        )
    );

    // 构造拥有真实按钮子树的组合徽章。
    let mut composite = Badge::new()
        // 配置可见数字装饰。
        .count(3)
        // 使用显式 ViewNode 保留完整子树身份。
        .child(ViewNode::leaf(Button::new("消息")));
    // 模拟组件树登记唯一直接子节点。
    Widget::on_children_changed(&mut composite, 1);
    // 创建一个稳定的测试子身份与自然尺寸。
    let mut child = LayoutChild::new(WidgetId::new(7), Size::new(40.0, 30.0));
    // 设置四侧 margin 以验证正常流外尺寸。
    child.margin = EdgeInsets::new(2.0, 3.0, 4.0, 5.0);
    // 空树足以满足当前布局入口未读取树的契约。
    let tree = WidgetTree::new();
    // 在首轮零 frame 中安排自然子尺寸。
    let positions = WidgetLayout::layout_children(&composite, Rect::zero(), &[child], &tree);
    // 子 border-box 必须从左上 margin 后开始并保持自然尺寸。
    assert_eq!(positions[0].1, Rect::new(2.0, 3.0, 40.0, 30.0));
    // 下一测量阶段必须包含四侧 margin 的正常流外尺寸。
    assert_eq!(
        // 读取布局收敛缓存形成的组件期望尺寸。
        WidgetLayout::measure(&composite, Constraints::unconstrained()),
        // 四十加左右 margin、三十加上下 margin。
        Size::new(46.0, 38.0)
    );
}

// 验证真实父级布局收敛时由子节点撑开 Badge 正常流宽度。
#[test]
// 测试名称陈述父 Row、Badge 和真实按钮的尺寸投影关系。
fn child_natural_size_drives_badge_in_parent_flow() {
    // 构造 Badge 与普通兄弟按钮组成的真实水平布局。
    let view = row((
        // Badge 初始 measure 为零，后续必须由真实子按钮撑开。
        ViewNode::leaf(
            Badge::new()
                // 配置可见数字装饰。
                .count(4)
                // 使用长文本按钮暴露固定或零宽回归。
                .child(ViewNode::leaf(Button::new("很长的通知按钮"))),
        ),
        // 兄弟按钮用于核对 Badge 的正常流占位。
        ViewNode::leaf(Button::new("兄弟")),
    ));
    // 通过真实适配器物化整棵父子树。
    let mut tree = ViewAdapter::build(view);
    // 取得 Row 根身份。
    let root = tree.root_id().expect("Row 根必须存在");
    // 取得 Badge 在 Row 中的稳定身份。
    let badge = tree.get(root).expect("Row 根必须可读").children()[0];
    // 取得 Badge 唯一真实按钮身份。
    let child = tree.get(badge).expect("Badge 必须可读").children()[0];
    // 取得 Row 中的兄弟按钮身份。
    let sibling = tree.get(root).expect("Row 根必须可读").children()[1];
    // 为根分配足够大的水平布局 frame。
    tree.get_mut(root)
        // Row 根必须保持可写。
        .expect("Row 根必须存在")
        // 固定表面隔离子自然宽度收敛。
        .set_frame(Rect::new(0.0, 0.0, 420.0, 60.0));
    // 执行框架真实多阶段布局收敛。
    tree.layout();
    // 读取最终 Badge 正常流 frame。
    let badge_frame = tree.get(badge).expect("Badge 必须存在").frame();
    // 读取最终真实子按钮 frame。
    let child_frame = tree.get(child).expect("子按钮必须存在").frame();
    // 读取最终兄弟按钮 frame。
    let sibling_frame = tree.get(sibling).expect("兄弟按钮必须存在").frame();
    // 长文本真实子按钮必须把 Badge 撑出正宽度。
    assert!(badge_frame.w > 80.0, "{badge_frame:?}");
    // 无 margin 时真实子按钮获得 Badge 的完整内容 frame。
    assert_eq!(child_frame, badge_frame);
    // 兄弟按钮必须排在 Badge 正常流占位之后而非与其重叠。
    assert!(sibling_frame.x >= badge_frame.x + badge_frame.w);
}

// 验证装饰几何锚定真实子节点右上角且隐藏状态不生成装饰面积。
#[test]
// 测试名称覆盖数字、marker、丝带与隐藏几何。
fn composite_decoration_anchors_child_top_right_without_expanding_layout() {
    // 构造数字装饰组合模式。
    let numeric = Badge::new()
        // 配置可见数字装饰。
        .count(5)
        // 使用显式 ViewNode 保留完整子树身份。
        .child(ViewNode::leaf(Button::new("收件箱")));
    // 写入实际子 border-box 作为几何锚点。
    numeric.child_frame.set(Rect::new(10.0, 20.0, 40.0, 30.0));
    // 计算无偏移数字胶囊矩形。
    let numeric_frame = numeric.composite_decoration_frame(Rect::zero(), 0.0, 0.0);
    // 数字胶囊中心横坐标必须落在子节点右边缘。
    assert_eq!(numeric_frame.x + numeric_frame.w * 0.5, 50.0);
    // 数字胶囊中心纵坐标必须落在子节点顶部。
    assert_eq!(numeric_frame.y + numeric_frame.h * 0.5, 20.0);

    // 构造带文字的状态 marker。
    let marker = Badge::new()
        // 使用成功状态 marker。
        .status(BadgeStatus::Success)
        // 添加可播报文字。
        .text("在线")
        // 启用组合模式。
        .child(ViewNode::leaf(Button::new("用户")));
    // 写入与数字测试相同的真实子 frame。
    marker.child_frame.set(Rect::new(10.0, 20.0, 40.0, 30.0));
    // 计算 marker 与文字整体矩形。
    let marker_frame = marker.composite_decoration_frame(Rect::zero(), 0.0, 0.0);
    // marker 圆心而非整段文字中心必须落在右上角。
    assert_eq!(
        marker_frame.x
            + BADGE_VISUAL.layout.marker_diameter * BADGE_VISUAL.layout.pill_radius_ratio,
        50.0
    );

    // 构造默认隐藏的零计数组合徽章。
    let hidden = Badge::new().child(ViewNode::leaf(Button::new("空收件箱")));
    // 隐藏装饰不得产生任何绘制面积。
    assert_eq!(
        hidden.composite_decoration_frame(Rect::zero(), 0.0, 0.0),
        Rect::zero()
    );
}

// 验证纯视觉圆点不产生空播报，状态文字和组合快照仍独立发布。
#[test]
// 测试名称覆盖无障碍与快照去重契约。
fn accessibility_and_snapshot_keep_child_semantics_independent() {
    // 构造没有文字的纯视觉圆点。
    let dot = Badge::new().dot();
    // 纯圆点不得发布空名称的 Status 节点。
    assert_eq!(
        dot.snapshot_fields().accessibility().role,
        AccessibilityRole::None
    );

    // 构造带明确状态与真实子 View 的组合 Badge。
    let mut status = Badge::new()
        // 发布成功状态语义。
        .status(BadgeStatus::Success)
        // 交给真实按钮保留自己的名称。
        .child(ViewNode::leaf(Button::new("账户")));
    // 模拟组件树已经登记唯一真实子节点。
    Widget::on_children_changed(&mut status, 1);
    // 读取组合 Badge 自己的快照字段。
    let fields = status.snapshot_fields();
    // 状态装饰必须作为独立 Status 语义发布。
    assert_eq!(fields.accessibility().role, AccessibilityRole::Status);
    // 无显式文字时必须发布稳定的状态名称。
    assert_eq!(fields.accessibility().name.as_deref(), Some("成功"));
    // 组合快照只记录模式、子存在事实与装饰边界，不复制子快照。
    match fields {
        // 解构新增组合事实字段。
        SnapshotFields::Badge {
            composite,
            child_present,
            decoration_bounds,
            ..
        } => {
            // 声明必须标记组合模式。
            assert!(composite);
            // 精确一个已登记子节点必须标记存在。
            assert!(child_present);
            // 尚未绘制时装饰边界保持空值而非伪造子边界。
            assert_eq!(decoration_bounds, Rect::zero());
        }
        // 具体 Badge 不得产生其他快照变体。
        _ => panic!("Badge 必须发布 Badge 快照变体"),
    }
}

// 验证透明 owner 不夺取按钮点击，且同 key 协调保留真实子身份。
#[test]
// 测试名称覆盖交互透明、焦点顺序、协调保留和移除释放。
fn child_keeps_click_focus_and_keyed_reconcile_identity() {
    // 建立真实按钮独占的点击计数。
    let clicks = Shared::new(CounterCell::new(0));
    // 为按钮处理器克隆共享句柄。
    let child_clicks = Shared::clone(&clicks);
    // 构造带稳定 key 和业务点击处理器的真实子 View。
    let child = ViewNode::leaf(Button::new("通知"))
        .key("badge-child")
        .on_click_fn(move || {
            // 只有真实按钮负责累加业务点击。
            child_clicks.set(child_clicks.get() + 1);
        });
    // 通过真实适配器物化 Badge 与子按钮。
    let mut tree = ViewAdapter::build(ViewNode::leaf(Badge::new().count(2).child(child)));
    // 取得稳定 Badge 根身份。
    let root = tree.root_id().expect("Badge 根必须存在");
    // 取得唯一真实子按钮身份。
    let first_child = tree.get(root).expect("Badge 根必须可读").children()[0];
    // 为根分配确定内容 frame。
    tree.get_mut(root)
        // 根节点必须可写。
        .expect("Badge 根必须存在")
        // 子按钮应获得完整 Badge 内容 frame。
        .set_frame(Rect::new(0.0, 0.0, 120.0, 40.0));
    // 执行一次真实父子布局。
    tree.layout();
    // 组合 Badge 自身不得进入 Tab 顺序。
    assert_eq!(Widget::tab_index(tree.get(root).unwrap().widget()), 0);
    // 在真实子按钮内部按下主指针。
    assert_eq!(
        tree.dispatch_event(&SystemEvent::PointerDown {
            // 坐标落在子按钮 border-box 内。
            pos: Point::new(20.0, 20.0),
            // 使用标准左键。
            button: MouseButton::Left,
            // 不使用修饰键。
            mods: KeyMod::NONE,
        }),
        // 子按钮必须处理按下。
        EventResult::Handled
    );
    // 在同一真实子按钮内部释放主指针。
    assert_eq!(
        tree.dispatch_event(&SystemEvent::PointerUp {
            // 使用与按下相同的位置。
            pos: Point::new(20.0, 20.0),
            // 使用配对左键。
            button: MouseButton::Left,
            // 不使用修饰键。
            mods: KeyMod::NONE,
        }),
        // 子按钮必须处理释放并合成 Click。
        EventResult::Handled
    );
    // Badge 不得复制或吞掉真实子按钮的业务点击。
    assert_eq!(clicks.get(), 1);

    // 用相同子 key 和新的装饰配置协调同一 Badge。
    ViewAdapter::reconcile(
        // 原位更新现有组件树。
        &mut tree,
        // 装饰配置变化不应重建真实子实例。
        ViewNode::leaf(
            Badge::new()
                // 更新数字装饰。
                .count(9)
                // 保持子 key 稳定。
                .child(ViewNode::leaf(Button::new("新通知")).key("badge-child")),
        ),
    );
    // 相同 key 必须保留原真实子 WidgetId。
    assert_eq!(tree.get(root).unwrap().children()[0], first_child);
    // 协调为零子节点叶 Badge。
    ViewAdapter::reconcile(&mut tree, ViewNode::leaf(Badge::new().count(1)));
    // 已移除真实子身份必须完成正常树清理。
    assert!(tree.get(first_child).is_none());
}

// 验证批次 8 账本映射对应的现行公开入口。
use std::cell::Cell;
// 验证 on_change 闭包可以跨事件保留观察状态。
use std::rc::Rc;

// 引入批次 8 的公开组件与基础类型。
use uix::prelude::*;
// 引入组件能力 trait 以验证布局、事件和能力入口。
use uix::ui::{EventHandler, WidgetCapabilities, WidgetComponent, WidgetLayout};

// 验证 Skeleton::paragraph 的公开配置进入实际测量高度。
#[test]
fn ledger_8_7_skeleton_paragraph_uses_requested_rows() {
    // 构造三行文字骨架屏。
    let skeleton = Skeleton::new().paragraph(3);
    // 三行默认行高应形成 48 logical 像素的固有高度。
    assert_eq!(
        WidgetLayout::measure(&skeleton, Constraints::unconstrained()),
        Size::new(200.0, 48.0)
    );
}

// 验证 Skeleton::active 暴露动画能力并可参与公开组件契约。
#[test]
fn ledger_8_8_skeleton_active_exposes_animation_capability() {
    // 构造启用 shimmer 的骨架屏。
    let skeleton = Skeleton::new().active(true);
    // active 配置必须保留动画能力位。
    assert!(WidgetComponent::capabilities(&skeleton).contains(WidgetCapabilities::ANIMATION));
}

// 验证 Watermark 的公开配置不会占用内容布局尺寸。
#[test]
fn ledger_8_9_watermark_keeps_zero_layout_footprint() {
    // 构造带完整绘制配置的水印。
    let watermark = Watermark::new("内部资料")
        .font_size(16.0)
        .opacity(0.12)
        .rotate(-20.0)
        .gap(220.0, 160.0);
    // 水印只绘制在父 frame 内，不改变布局占位。
    assert_eq!(
        WidgetLayout::measure(&watermark, Constraints::unconstrained()),
        Size::zero()
    );
}

// 验证 Transfer::render_item 可以接入自定义公开 View。
#[test]
fn ledger_8_10_transfer_render_item_builds_public_view() {
    // 构造带自定义条目渲染器的穿梭框。
    let transfer = Transfer::new()
        .source(vec![TransferItem::new("draft", "草稿")])
        .render_item(|item| label(item.title.clone()));
    // 自定义条目配置必须能够进入声明式 View 树。
    let _node = embed(transfer);
}

// 验证 Transfer::on_change 在真实迁移后收到回调。
#[test]
fn ledger_8_11_transfer_on_change_observes_move() {
    // 创建跨事件保存回调状态的单元格。
    let called = Rc::new(Cell::new(false));
    // 克隆回调所需的共享状态。
    let callback_called = Rc::clone(&called);
    // 构造一个已选中的源条目。
    let mut transfer = Transfer::new()
        .source(vec![TransferItem {
            key: "draft".into(),
            title: "草稿".into(),
            selected: true,
        }])
        .on_change(move |_source, _target, _direction| callback_called.set(true));
    // Enter 应沿默认活动 pane 迁移已选条目。
    let _result = EventHandler::on_event(
        &mut transfer,
        &SystemEvent::KeyDown {
            key: KeyCode::Enter,
            mods: KeyMod::NONE,
        },
    );
    // 迁移后必须触发用户回调。
    assert!(called.get());
}

// 验证 Transfer::searchable 接收公开文本输入并保留查询值。
#[test]
fn ledger_8_12_transfer_searchable_tracks_query() {
    // 构造启用搜索的穿梭框。
    let mut transfer = Transfer::new()
        .source(vec![TransferItem::new("draft", "草稿")])
        .searchable(true);
    // 输入事件必须由可搜索穿梭框处理。
    let _result =
        EventHandler::on_event(&mut transfer, &SystemEvent::TextInput { text: "草".into() });
    // 查询值必须保留给过滤路径消费。
    assert_eq!(transfer.search_query(), "草");
}

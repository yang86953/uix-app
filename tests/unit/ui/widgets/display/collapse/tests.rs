// 引入受测稳定面板与折叠组。
use super::{COLLAPSE_VISUAL_REF, Collapse, CollapsePanel};
// 引入公开 View 构建入口。
use crate::ui::view::View;
// 引入语义事件与受控状态测试类型。
use crate::ui::{EventHandler, SemanticKind, SemanticPayload, State, SystemEvent, WidgetId};

// 构造两个具有显式稳定 key 的面板。
fn panels() -> Vec<CollapsePanel> {
    // 返回顺序固定的测试数据。
    vec![
        // 首项使用 alpha 稳定身份。
        CollapsePanel::new("同名", "Alpha 内容").key("alpha"),
        // 次项使用 beta 稳定身份，即使 header 相同也不冲突。
        CollapsePanel::new("同名", "Beta 内容").key("beta"),
    ]
}

// 验证显式 key 与 header 兼容身份。
#[test]
fn panel_stable_key_prefers_explicit_value() {
    // 旧调用方不设置 key 时继续使用 header。
    let fallback = CollapsePanel::new("兼容标题", "内容");
    // header 必须成为兼容稳定身份。
    assert_eq!(fallback.stable_key(), "兼容标题");
    // 显式非空 key 必须覆盖 header 身份。
    assert_eq!(fallback.key("stable").stable_key(), "stable");
}

// 验证未绑定时继续使用面板 expanded 初值与手风琴兼容归一化。
#[test]
fn uncontrolled_panels_keep_expanded_initial_values() {
    // 构造两个默认展开的非受控面板。
    let collapse = Collapse::new().panels(vec![
        // 首项使用显式稳定 key。
        CollapsePanel::new("甲", "甲内容").key("alpha").expanded(),
        // 次项同样声明默认展开。
        CollapsePanel::new("乙", "乙内容").key("beta").expanded(),
    ]);
    // 非手风琴模式必须保留全部展开初值。
    assert_eq!(
        collapse.expanded_keys(),
        vec!["alpha".to_string(), "beta".to_string()]
    );

    // 既有无参数手风琴构建器继续归一化非受控初值。
    let accordion = collapse.accordion();
    // 声明顺序中的首项取得唯一展开所有权。
    assert_eq!(accordion.expanded_keys(), vec!["alpha".to_string()]);
}

// 验证外部失效 key 保持调用方所有且界面无展开项。
#[test]
fn controlled_active_keys_keep_unmatched_external_values() {
    // 外部状态包含集合中不存在的 key。
    let active = State::new(vec!["missing".to_string()]);
    // 构造受控折叠组。
    let collapse = Collapse::new().panels(panels()).active_keys(&active);

    // 失效 key 不得展开任意面板。
    assert!(collapse.expanded_keys().is_empty());
    // 运行时不得反向归一化调用方状态。
    assert_eq!(active.get(), vec!["missing".to_string()]);
}

// 验证用户切换先写回状态并只产生一次稳定 key Change。
#[test]
fn controlled_toggle_writes_state_before_single_change() {
    // 初始没有任何展开 key。
    let active = State::new(Vec::<String>::new());
    // 构造受控折叠组。
    let mut collapse = Collapse::new().panels(panels()).active_keys(&active);

    // 模拟用户展开第二个面板。
    collapse.toggle_panel(1);
    // 语义事件取走前状态必须已经写回稳定 key。
    assert_eq!(active.get(), vec!["beta".to_string()]);
    // 取走本次切换事件。
    let event = collapse
        .semantic_event(WidgetId::new(9), &SystemEvent::FocusIn)
        .expect("用户切换应产生 Change 事件");

    // 事件类型必须是统一 Change。
    assert_eq!(event.kind, SemanticKind::Change);
    // 载荷必须是稳定 key 而不是索引。
    assert!(matches!(event.payload, SemanticPayload::Text(value) if value == "beta"));
    // 同一次切换不得重复发布。
    assert!(
        collapse
            .semantic_event(WidgetId::new(9), &SystemEvent::FocusIn)
            .is_none()
    );
}

// 验证手风琴只归一化用户产出的集合而不改写外部输入。
#[test]
fn accordion_preserves_external_multi_key_and_normalizes_user_toggle() {
    // 外部状态同时请求两个有效 key。
    let active = State::new(vec!["alpha".to_string(), "beta".to_string()]);
    // 受控手风琴只显示声明顺序中的首个有效面板。
    let mut collapse = Collapse::new()
        .panels(panels())
        .active_keys(&active)
        .accordion();

    // 界面最多展开一个稳定 key。
    assert_eq!(collapse.expanded_keys(), vec!["alpha".to_string()]);
    // 外部多 key 输入仍由调用方拥有。
    assert_eq!(active.get(), vec!["alpha".to_string(), "beta".to_string()]);

    // 用户随后展开第二项。
    collapse.toggle_panel(1);
    // 用户产出的新状态必须原子归一化为单个 key。
    assert_eq!(active.get(), vec!["beta".to_string()]);
}

// 验证受控重建精确采用最新外部 key 集合。
#[test]
fn controlled_sync_from_uses_latest_external_keys() {
    // 初始展开首项。
    let active = State::new(vec!["alpha".to_string()]);
    // 构造将被原位同步的当前组件。
    let mut current = Collapse::new().panels(panels()).active_keys(&active);
    // 保存首项内容 Canvas 已捕获的透明度句柄。
    let alpha_opacity = current.content_opacities[0].clone();
    // 保存次项内容 Canvas 已捕获的透明度句柄。
    let beta_opacity = current.content_opacities[1].clone();
    // 调用方在下一声明前切换到次项。
    active.set(vec!["beta".to_string()]);
    // 使用同一状态句柄构造下一帧声明。
    let next = Collapse::new().panels(panels()).active_keys(&active);

    // 执行真实组件原位同步路径。
    current.sync_from(next);

    // 界面必须采用最新外部稳定 key。
    assert_eq!(current.expanded_keys(), vec!["beta".to_string()]);
    // 受控重建不得替换首项已物化 Canvas 持有的句柄。
    assert!(std::rc::Rc::ptr_eq(
        &alpha_opacity,
        &current.content_opacities[0]
    ));
    // 受控重建不得替换次项已物化 Canvas 持有的句柄。
    assert!(std::rc::Rc::ptr_eq(
        &beta_opacity,
        &current.content_opacities[1]
    ));
    // 折叠首项必须把原句柄更新到完全透明。
    assert_eq!(alpha_opacity.get(), 0.0);
    // 展开次项必须把原句柄更新到完全不透明。
    assert_eq!(beta_opacity.get(), 1.0);
    // 外部状态不得被旧内部快照覆盖。
    assert_eq!(active.get(), vec!["beta".to_string()]);
}

// 验证稳定动态内容不重建条目，而标题兼容 key 变化仍触发刷新。
#[test]
fn stable_content_refresh_detects_key_changes_without_rebuilding_first() {
    // 使用展开面板建立与真实动态内容相同的物化条目。
    let mut collapse = Collapse::new().panels(vec![
        CollapsePanel::new("甲", "甲内容").expanded(),
        CollapsePanel::new("乙", "乙内容").expanded(),
    ]);
    let entries = collapse.desired_content_entries();
    let child_count = entries.len();
    collapse.mark_content_materialized(entries);

    // 未变化时稳定检查必须直接复用已有内容子树。
    assert!(collapse.content_views_for_refresh(child_count).is_none());

    // 未声明显式 key 时，标题变化会改变兼容稳定身份并要求刷新。
    collapse.panels[0].header = "新甲".to_string();
    assert!(collapse.content_views_for_refresh(child_count).is_some());
}

// 验证 UIX 声明根保持面板内核并注入尺寸、排版与内容留白。
#[test]
fn uix_root_preserves_collapse_kernel_and_visual_contract() {
    let node = View::build(Collapse::new().panels(panels()));
    assert!(node.children.is_empty());
    let kernel = node
        .widget
        .as_any()
        .downcast_ref::<Collapse>()
        .expect("UIX 根必须保留 Collapse 内核");
    assert_eq!(kernel.panels.len(), 2);
    assert!(!kernel.borderless);
    assert_eq!(
        kernel.visual_contract_for_test(),
        (240.0, 320.0, 36.0, 14.0, 12.0, 16.0)
    );
}

// 验证 Collapse 实例共享 UIX 视觉表且显式无边框设置优先。
#[test]
fn collapse_instances_share_uix_visual_and_preserve_authored_borderless() {
    assert!(std::ptr::eq(Collapse::new().visual, COLLAPSE_VISUAL_REF));
    let first = View::build(Collapse::new().panels(panels()));
    let second = View::build(Collapse::new().panels(panels()).borderless(true));
    let first = first.widget.as_any().downcast_ref::<Collapse>().unwrap();
    let second = second.widget.as_any().downcast_ref::<Collapse>().unwrap();
    assert!(first.shares_visual_with_for_test(second));
    assert!(std::ptr::eq(first.visual, COLLAPSE_VISUAL_REF));
    assert!(second.borderless);
}

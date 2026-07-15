use super::*;

/// 模拟主循环：reconcile → layout（不手动重设 root frame），断言内容区 ScrollView 仍有高度。
#[test]
fn page_switch_keeps_content_scrollview_height() {
    let active = State::new(0usize);
    let timer_ticks = State::new(0u32);
    let active_for_build = active.clone();
    let timer_for_build = timer_ticks.clone();
    let root = ViewAdapter::capture_root(move || {
        app_shell(active_for_build.clone(), timer_for_build.clone())
    });
    let mut tree = ViewAdapter::build_nodes(root);
    if let Some(r) = tree.root_mut() {
        r.set_frame(Rect::new(0.0, 0.0, INIT_W as f32, INIT_H as f32));
    }
    tree.layout();

    let before: Vec<Rect> = tree
        .find_all_by_type::<ScrollView>()
        .into_iter()
        .filter_map(|(id, _)| tree.get(id).map(|n| n.frame()))
        .filter(|f| f.x >= SIDEBAR_W - 2.0)
        .collect();
    assert!(
        before.iter().any(|f| f.h > 200.0),
        "home content ScrollView should be tall, got {before:?}"
    );
    let header_before = tree
        .find_all_by_type::<uix::ui::widgets::Container>()
        .into_iter()
        .filter_map(|(id, c)| {
            let frame = tree.get(id)?.frame();
            // 顶栏：内容区左侧对齐、窄高
            if (frame.x - SIDEBAR_W).abs() < 2.0 && frame.y < 5.0 && frame.h < 80.0 {
                Some((c.flex_grow(), frame))
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    assert!(
        header_before.iter().any(|(g, _)| *g == 0.0),
        "header_bar must not flex-grow, got {header_before:?}"
    );

    active.set(PAGE_GENERAL);
    assert!(tree.take_reconcile_requested());
    let active_for_reconcile = active.clone();
    let timer_for_reconcile = timer_ticks.clone();
    let root = ViewAdapter::capture_root(move || {
        app_shell(active_for_reconcile.clone(), timer_for_reconcile.clone())
    });
    ViewAdapter::reconcile_nodes(&mut tree, root);
    // 主循环不会在 reconcile 后重设 root；只跑 layout
    tree.layout();

    let after: Vec<Rect> = tree
        .find_all_by_type::<ScrollView>()
        .into_iter()
        .filter_map(|(id, _)| tree.get(id).map(|n| n.frame()))
        .filter(|f| f.x >= SIDEBAR_W - 2.0)
        .collect();
    assert!(
        after.iter().any(|f| f.h > 400.0 && f.y < 200.0),
        "after page switch content ScrollView must stay in viewport, got {after:?}"
    );

    let body_labels: Vec<(String, Rect)> = tree
        .find_all_by_type::<Label>()
        .into_iter()
        .filter_map(|(id, l)| {
            let text = l.text().to_string();
            let frame = tree.get(id)?.frame();
            if frame.x < SIDEBAR_W || text.trim().is_empty() {
                return None;
            }
            Some((text, frame))
        })
        .collect();
    assert!(
        body_labels
            .iter()
            .any(|(t, f)| t.contains("Typography") && f.h > 0.0 && f.w > 0.0),
        "general body labels must have non-zero frames, got {body_labels:?}"
    );
}

#[test]
fn page_switch_reconcile_updates_content() {
    let active = State::new(0usize);
    let timer_ticks = State::new(0u32);
    let active_for_build = active.clone();
    let timer_for_build = timer_ticks.clone();
    let root = ViewAdapter::capture_root(move || {
        app_shell(active_for_build.clone(), timer_for_build.clone())
    });
    let mut tree = ViewAdapter::build_nodes(root);
    if let Some(r) = tree.root_mut() {
        r.set_frame(Rect::new(0.0, 0.0, INIT_W as f32, INIT_H as f32));
    }
    tree.layout();

    let home_labels: Vec<String> = tree
        .find_all_by_type::<Label>()
        .into_iter()
        .map(|(_, l)| l.text().to_string())
        .collect();
    assert!(
        home_labels.iter().any(|t| t.contains("快捷导航")),
        "home page content missing, got: {home_labels:?}"
    );
    assert!(
        !home_labels.iter().any(|t| t.contains("Typography")),
        "general content should not appear on home"
    );

    active.set(PAGE_GENERAL);
    assert!(tree.take_reconcile_requested());

    let active_for_reconcile = active.clone();
    let timer_for_reconcile = timer_ticks.clone();
    let root = ViewAdapter::capture_root(move || {
        app_shell(active_for_reconcile.clone(), timer_for_reconcile.clone())
    });
    ViewAdapter::reconcile_nodes(&mut tree, root);
    if let Some(r) = tree.root_mut() {
        r.set_frame(Rect::new(0.0, 0.0, INIT_W as f32, INIT_H as f32));
    }
    tree.layout();

    let general_labels: Vec<String> = tree
        .find_all_by_type::<Label>()
        .into_iter()
        .map(|(_, l)| l.text().to_string())
        .collect();
    assert!(
        general_labels.iter().any(|t| t.contains("Typography")),
        "general page content missing after reconcile, got: {general_labels:?}"
    );
    assert!(
        !general_labels.iter().any(|t| t.contains("快捷导航")),
        "home section should not remain after switch to general"
    );

    tree.reset_invalidation();
    assert!(!tree.take_reconcile_requested());

    active.set(crate::common::page::PAGE_LAYOUT);
    assert!(tree.take_reconcile_requested());

    let active_for_layout = active.clone();
    let timer_for_layout = timer_ticks.clone();
    let root = ViewAdapter::capture_root(move || {
        app_shell(active_for_layout.clone(), timer_for_layout.clone())
    });
    ViewAdapter::reconcile_nodes(&mut tree, root);
    if let Some(r) = tree.root_mut() {
        r.set_frame(Rect::new(0.0, 0.0, INIT_W as f32, INIT_H as f32));
    }
    tree.layout();

    let layout_labels: Vec<String> = tree
        .find_all_by_type::<Label>()
        .into_iter()
        .map(|(_, l)| l.text().to_string())
        .collect();
    assert!(
        layout_labels.iter().any(|t| t.contains("Splitter")),
        "layout page content missing after second switch, got: {layout_labels:?}"
    );
}

#[test]
fn page_switch_updates_heading_and_body_together() {
    let active = State::new(0usize);
    let timer_ticks = State::new(0u32);
    let active_for_build = active.clone();
    let timer_for_build = timer_ticks.clone();
    let root = ViewAdapter::capture_root(move || {
        app_shell(active_for_build.clone(), timer_for_build.clone())
    });
    let mut tree = ViewAdapter::build_nodes(root);
    if let Some(r) = tree.root_mut() {
        r.set_frame(Rect::new(0.0, 0.0, INIT_W as f32, INIT_H as f32));
    }
    tree.layout();

    fn page_title_present(labels: &[String], title: &str) -> bool {
        labels.iter().any(|t| t.trim() == title)
    }

    let initial_labels: Vec<String> = tree
        .find_all_by_type::<Label>()
        .into_iter()
        .map(|(_, l)| l.text().to_string())
        .collect();
    assert!(page_title_present(&initial_labels, "首页"));
    assert!(
        initial_labels.iter().any(|t| t.contains("快捷导航")),
        "home body missing, got: {initial_labels:?}"
    );

    active.set(PAGE_APP);
    assert!(tree.take_reconcile_requested());

    let active_for_reconcile = active.clone();
    let timer_for_reconcile = timer_ticks.clone();
    let root = ViewAdapter::capture_root(move || {
        app_shell(active_for_reconcile.clone(), timer_for_reconcile.clone())
    });
    ViewAdapter::reconcile_nodes(&mut tree, root);
    if let Some(r) = tree.root_mut() {
        r.set_frame(Rect::new(0.0, 0.0, INIT_W as f32, INIT_H as f32));
    }
    tree.layout();

    let labels: Vec<String> = tree
        .find_all_by_type::<Label>()
        .into_iter()
        .map(|(_, l)| l.text().to_string())
        .collect();
    assert!(page_title_present(&labels, "应用能力"));
    assert!(
        labels.iter().any(|t| t.contains("响应式 State")),
        "runtime body missing after switch to 应用能力, got: {labels:?}"
    );
    assert!(
        !labels.iter().any(|t| t.contains("快捷导航")),
        "home body leaked after switch to 应用能力"
    );

    active.set(PAGE_GENERAL);
    assert!(tree.take_reconcile_requested());

    let active_for_general = active.clone();
    let timer_for_general = timer_ticks.clone();
    let root = ViewAdapter::capture_root(move || {
        app_shell(active_for_general.clone(), timer_for_general.clone())
    });
    ViewAdapter::reconcile_nodes(&mut tree, root);
    if let Some(r) = tree.root_mut() {
        r.set_frame(Rect::new(0.0, 0.0, INIT_W as f32, INIT_H as f32));
    }
    tree.layout();

    let labels: Vec<String> = tree
        .find_all_by_type::<Label>()
        .into_iter()
        .map(|(_, l)| l.text().to_string())
        .collect();
    assert!(page_title_present(&labels, "通用"));
    assert!(
        labels.iter().any(|t| t.contains("Typography")),
        "general body missing, got: {labels:?}"
    );
    assert!(
        !labels.iter().any(|t| t.contains("响应式 State")),
        "runtime body leaked after switch to 通用"
    );
}

#[test]
fn timer_state_update_requests_paint() {
    let timer_ticks = State::new(0u32);
    let ticks_for_label = timer_ticks.clone();
    let mut tree = ViewAdapter::build(dynamic_label(move || {
        format!("timer: {}", ticks_for_label.get())
    }));
    if let Some(r) = tree.root_mut() {
        r.set_frame(Rect::new(0.0, 0.0, 120.0, 24.0));
    }
    tree.layout();
    tree.reset_invalidation();

    timer_ticks.set(3);
    assert!(tree.has_render_work());
}

#[test]
fn home_page_builds() {
    let tk = DesignTokens::antd_light();
    let timer_ticks = State::new(0u32);
    let active = State::new(0usize);
    let ctx = crate::demos::DemoCtx::new(&tk, &timer_ticks, Some(&active));
    let root = crate::demos::home::page_home(&ctx);
    let tree = ViewAdapter::build(root);
    assert!(tree.root_id().is_some());
}

#[test]
fn demo_local_counters_survive_root_reconcile() {
    use uix::core::Point;
    use uix::native::traits::input::{KeyMod, MouseButton};

    let active = State::new(0usize);
    let timer_ticks = State::new(0u32);
    let home_count = State::new(0i32);
    let runtime_count = State::new(0i32);
    let theme_control = ThemeControl::default();

    let root = ViewAdapter::capture_root(|| {
        app_shell_with_counters(
            active.clone(),
            timer_ticks.clone(),
            &home_count,
            &runtime_count,
            &theme_control,
        )
    });
    let mut tree = ViewAdapter::build_nodes(root);
    if let Some(root) = tree.root_mut() {
        root.set_frame(Rect::new(0.0, 0.0, INIT_W as f32, INIT_H as f32));
    }
    tree.layout();

    let plus = tree
        .find_all_by_type::<Button>()
        .into_iter()
        .find(|(_, button)| button.text() == "+1")
        .expect("home increment button");
    let frame = tree.get(plus.0).expect("increment button node").frame();
    let pos = Point::new(frame.x + frame.w * 0.5, frame.y + frame.h * 0.5);
    let _ = tree.dispatch_event(&SystemEvent::PointerDown {
        pos,
        button: MouseButton::Left,
        mods: KeyMod::NONE,
    });
    let _ = tree.dispatch_event(&SystemEvent::PointerUp {
        pos,
        button: MouseButton::Left,
        mods: KeyMod::NONE,
    });

    assert_eq!(home_count.get(), 1);
    assert!(tree.take_reconcile_requested());

    let next = ViewAdapter::capture_root(|| {
        app_shell_with_counters(
            active.clone(),
            timer_ticks.clone(),
            &home_count,
            &runtime_count,
            &theme_control,
        )
    });
    ViewAdapter::reconcile_nodes(&mut tree, next);
    if let Some(root) = tree.root_mut() {
        root.set_frame(Rect::new(0.0, 0.0, INIT_W as f32, INIT_H as f32));
    }
    tree.layout();

    active.set(PAGE_APP);
    let runtime_root = ViewAdapter::capture_root(|| {
        app_shell_with_counters(
            active.clone(),
            timer_ticks.clone(),
            &home_count,
            &runtime_count,
            &theme_control,
        )
    });
    ViewAdapter::reconcile_nodes(&mut tree, runtime_root);
    if let Some(root) = tree.root_mut() {
        root.set_frame(Rect::new(0.0, 0.0, INIT_W as f32, INIT_H as f32));
    }
    tree.layout();

    let runtime_plus = tree
        .find_all_by_type::<Button>()
        .into_iter()
        .find(|(_, button)| button.text() == "+1")
        .expect("runtime increment button");
    let runtime_frame = tree
        .get(runtime_plus.0)
        .expect("runtime increment button node")
        .frame();
    assert!(
        runtime_frame.h > 0.0 && runtime_frame.y < 320.0,
        "runtime counter controls must remain visible near the top of the scroll page, got {runtime_frame:?}"
    );
    let runtime_pos = Point::new(
        runtime_frame.x + runtime_frame.w * 0.5,
        runtime_frame.y + runtime_frame.h * 0.5,
    );
    let _ = tree.dispatch_event(&SystemEvent::PointerDown {
        pos: runtime_pos,
        button: MouseButton::Left,
        mods: KeyMod::NONE,
    });
    let _ = tree.dispatch_event(&SystemEvent::PointerUp {
        pos: runtime_pos,
        button: MouseButton::Left,
        mods: KeyMod::NONE,
    });

    assert_eq!(runtime_count.get(), 1);
}

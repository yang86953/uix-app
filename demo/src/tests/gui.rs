use super::*;
use crate::common::page::{
    INIT_H, INIT_W, PAGE_APP, PAGE_COUNT, PAGE_GENERAL, PAGE_TITLES, SIDEBAR_W,
};
use uix::prelude::{dynamic_label, Button, DesignTokens, Label, Rect, State, ViewAdapter};
use uix::ui::core::widget::WidgetCore;

#[test]
fn page_titles_match_modules() {
    assert_eq!(PAGE_TITLES.len(), PAGE_COUNT);
    assert_eq!(PAGE_COUNT, 11);
}

#[test]
fn gui_shell_layout() {
    let active = State::new(0usize);
    let timer_ticks = State::new(0u32);
    let anim_time = State::new(0.0f32);
    let root = app_shell(active, timer_ticks, anim_time);
    let mut tree = ViewAdapter::build(root);
    if let Some(r) = tree.root_mut() {
        r.set_frame(Rect::new(0.0, 0.0, INIT_W as f32, INIT_H as f32));
    }
    tree.layout();

    let root_id = tree.root_id().expect("root");
    let main_row = tree.get(root_id).unwrap().children()[0];
    let children = tree.get(main_row).unwrap().children().to_vec();
    assert_eq!(children.len(), 2, "shell should be sidebar + content");

    let nav_frame = tree.get(children[0]).unwrap().frame();
    let content_frame = tree.get(children[1]).unwrap().frame();
    assert!(
        (nav_frame.w - SIDEBAR_W).abs() < 2.0,
        "sidebar width {} expected ~{SIDEBAR_W}",
        nav_frame.w
    );
    assert!(
        content_frame.w > 900.0,
        "content too narrow: {}",
        content_frame.w
    );
}

#[test]
fn general_page_has_buttons() {
    let tk = DesignTokens::antd_light();
    let timer_ticks = State::new(0u32);
    let anim_time = State::new(0.0f32);
    let active = State::new(2usize);
    let ctx = crate::demos::DemoCtx::new(&tk, &timer_ticks, &anim_time, Some(&active));
    let root = crate::demos::general::page_general(&ctx);
    let mut tree = ViewAdapter::build(root);
    if let Some(r) = tree.root_mut() {
        r.set_frame(Rect::new(0.0, 0.0, INIT_W as f32, INIT_H as f32));
    }
    tree.layout();

    let buttons = tree.find_all_by_type::<Button>();
    assert!(
        buttons.len() >= 4,
        "general page should showcase multiple buttons"
    );
}

#[test]
fn gallery_page_lists_coverage() {
    let tk = DesignTokens::antd_light();
    let timer_ticks = State::new(0u32);
    let anim_time = State::new(0.0f32);
    let active = State::new(10usize);
    let ctx = crate::demos::DemoCtx::new(&tk, &timer_ticks, &anim_time, Some(&active));
    let root = crate::demos::gallery::page_gallery(&ctx);
    let tree = ViewAdapter::build(root);
    assert!(tree.root_id().is_some());
}

#[test]
fn page_switch_reconcile_updates_content() {
    let active = State::new(0usize);
    let timer_ticks = State::new(0u32);
    let anim_time = State::new(0.0f32);

    let active_for_build = active.clone();
    let timer_for_build = timer_ticks.clone();
    let anim_for_build = anim_time.clone();
    let root = ViewAdapter::capture_root(move || {
        app_shell(
            active_for_build.clone(),
            timer_for_build.clone(),
            anim_for_build.clone(),
        )
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
    let anim_for_reconcile = anim_time.clone();
    let root = ViewAdapter::capture_root(move || {
        app_shell(
            active_for_reconcile.clone(),
            timer_for_reconcile.clone(),
            anim_for_reconcile.clone(),
        )
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
    let anim_for_layout = anim_time.clone();
    let root = ViewAdapter::capture_root(move || {
        app_shell(
            active_for_layout.clone(),
            timer_for_layout.clone(),
            anim_for_layout.clone(),
        )
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
    let anim_time = State::new(0.0f32);

    let active_for_build = active.clone();
    let timer_for_build = timer_ticks.clone();
    let anim_for_build = anim_time.clone();
    let root = ViewAdapter::capture_root(move || {
        app_shell(
            active_for_build.clone(),
            timer_for_build.clone(),
            anim_for_build.clone(),
        )
    });
    let mut tree = ViewAdapter::build_nodes(root);
    if let Some(r) = tree.root_mut() {
        r.set_frame(Rect::new(0.0, 0.0, INIT_W as f32, INIT_H as f32));
    }
    tree.layout();

    fn heading_title(labels: &[String]) -> Option<String> {
        labels
            .iter()
            .find(|t| {
                PAGE_TITLES
                    .iter()
                    .any(|(_, title)| title.trim() == t.trim())
            })
            .cloned()
    }

    let initial_labels: Vec<String> = tree
        .find_all_by_type::<Label>()
        .into_iter()
        .map(|(_, l)| l.text().to_string())
        .collect();
    assert_eq!(heading_title(&initial_labels).as_deref(), Some("首页"));

    active.set(PAGE_APP);
    assert!(tree.take_reconcile_requested());

    let active_for_reconcile = active.clone();
    let timer_for_reconcile = timer_ticks.clone();
    let anim_for_reconcile = anim_time.clone();
    let root = ViewAdapter::capture_root(move || {
        app_shell(
            active_for_reconcile.clone(),
            timer_for_reconcile.clone(),
            anim_for_reconcile.clone(),
        )
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
    assert_eq!(heading_title(&labels).as_deref(), Some("应用能力"));
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
    let anim_for_general = anim_time.clone();
    let root = ViewAdapter::capture_root(move || {
        app_shell(
            active_for_general.clone(),
            timer_for_general.clone(),
            anim_for_general.clone(),
        )
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
    assert_eq!(heading_title(&labels).as_deref(), Some("通用"));
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
    let anim_time = State::new(0.0f32);
    let active = State::new(0usize);
    let ctx = crate::demos::DemoCtx::new(&tk, &timer_ticks, &anim_time, Some(&active));
    let root = crate::demos::home::page_home(&ctx);
    let tree = ViewAdapter::build(root);
    assert!(tree.root_id().is_some());
}

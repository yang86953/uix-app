use super::*;
use crate::common::page::{INIT_H, INIT_W, PAGE_TITLES, SIDEBAR_W};
use uix::prelude::{Button, DesignTokens, Rect, State, ViewAdapter};
use uix::ui::core::widget::WidgetCore;

#[test]
fn page_titles_match_modules() {
    assert_eq!(PAGE_TITLES.len(), 8);
}

#[test]
fn dashboard_shell_layout() {
    let active = State::new(0usize);
    let root = dashboard_root(active);
    let mut tree = ViewAdapter::build(root);
    if let Some(r) = tree.root_mut() {
        r.set_frame(Rect::new(0.0, 0.0, INIT_W as f32, INIT_H as f32));
    }
    tree.layout();

    let root_id = tree.root_id().expect("root");
    let children = tree.get(root_id).unwrap().children().to_vec();
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
    let root = crate::demos::general::page_general(&tk);
    let mut tree = ViewAdapter::build(root);
    if let Some(r) = tree.root_mut() {
        r.set_frame(Rect::new(0.0, 0.0, INIT_W as f32, INIT_H as f32));
    }
    tree.layout();

    let buttons = tree.find_all_by_type::<Button>();
    assert!(buttons.len() >= 4, "general page should showcase multiple buttons");
}

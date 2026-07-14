use super::*;

#[test]
fn shell_labels_are_vertically_spaced() {
    use uix::ui::widgets::NavItem;

    let active = State::new(0usize);
    let timer_ticks = State::new(0u32);
    let anim_time = State::new(0.0f32);
    let root = app_shell(active, timer_ticks, anim_time);
    let mut tree = ViewAdapter::build(root);
    if let Some(r) = tree.root_mut() {
        r.set_frame(Rect::new(0.0, 0.0, INIT_W as f32, INIT_H as f32));
    }
    tree.layout();

    let labels: Vec<(String, Rect)> = tree
        .find_all_by_type::<Label>()
        .into_iter()
        .filter_map(|(id, l)| {
            let text = l.text().to_string();
            if text.trim().is_empty() {
                return None;
            }
            let frame = tree.get(id)?.frame();
            Some((text, frame))
        })
        .collect();

    let brand = labels
        .iter()
        .find(|(t, _)| t == "UIX Demo")
        .expect("brand label");
    let home_nav = tree
        .find_all_by_type::<NavItem>()
        .into_iter()
        .find(|(id, item)| {
            item.label_text() == "首页"
                && tree
                    .get(*id)
                    .map(|n| n.frame().x < SIDEBAR_W)
                    .unwrap_or(false)
        })
        .expect("sidebar home NavItem");
    let home_nav_frame = tree.get(home_nav.0).expect("nav node").frame();
    let page_heading = labels
        .iter()
        .find(|(t, f)| t.trim() == "首页" && f.x >= SIDEBAR_W)
        .expect("page heading");

    assert!(
        home_nav_frame.y > brand.1.y + 20.0,
        "sidebar stacked: brand y={} nav y={}",
        brand.1.y,
        home_nav_frame.y
    );
    assert!(
        page_heading.1.y > 20.0,
        "page heading stuck near top: y={}",
        page_heading.1.y
    );
    assert!(
        page_heading.1.x >= SIDEBAR_W - 2.0,
        "page heading should be in content area, x={}",
        page_heading.1.x
    );
}

#[test]
fn sidebar_nav_uses_nav_item_component() {
    use uix::ui::widgets::NavItem;

    let active = State::new(0usize);
    let timer_ticks = State::new(0u32);
    let anim_time = State::new(0.0f32);
    let root = app_shell(active, timer_ticks, anim_time);
    let mut tree = ViewAdapter::build(root);
    if let Some(r) = tree.root_mut() {
        r.set_frame(Rect::new(0.0, 0.0, INIT_W as f32, INIT_H as f32));
    }
    tree.layout();

    let nav_items = tree.find_all_by_type::<NavItem>();
    assert!(
        nav_items.len() >= PAGE_COUNT,
        "sidebar should use NavItem for each page, got {}",
        nav_items.len()
    );

    let home = nav_items
        .iter()
        .find(|(id, item)| {
            item.label_text() == "首页"
                && item.nav_index() == 0
                && tree
                    .get(*id)
                    .map(|n| n.frame().x < SIDEBAR_W)
                    .unwrap_or(false)
        })
        .expect("sidebar home NavItem");
    let frame = tree.get(home.0).expect("nav node").frame();
    assert!(
        frame.h >= 30.0 && frame.w > 100.0,
        "NavItem frame should be laid out, got {frame:?}"
    );

    let group = tree
        .find_all_by_type::<Label>()
        .into_iter()
        .find(|(_, l)| l.text() == "入门")
        .expect("group label");
    let group_frame = tree.get(group.0).expect("group node").frame();
    assert!(
        group_frame.x < SIDEBAR_W && group_frame.y < frame.y,
        "group label should sit above home NavItem in sidebar"
    );
}

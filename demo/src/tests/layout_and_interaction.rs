use super::*;

#[test]
fn page_titles_match_modules() {
    assert_eq!(PAGE_TITLES.len(), PAGE_COUNT);
    assert_eq!(PAGE_COUNT, 13);
}

#[test]
fn gui_shell_layout() {
    let active = State::new(0usize);
    let timer_ticks = State::new(0u32);
    let root = app_shell(active, timer_ticks);
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
fn compact_shell_keeps_sidebar_scroll_above_status_bar() {
    let active = State::new(0usize);
    let timer_ticks = State::new(0u32);
    let root = app_shell(active, timer_ticks);
    let mut tree = ViewAdapter::build(root);
    if let Some(root) = tree.root_mut() {
        root.set_frame(Rect::new(0.0, 0.0, 900.0, 640.0));
    }
    tree.layout();

    let sidebar_scroll = tree
        .traverse()
        .iter()
        .copied()
        .find(|&id| {
            tree.get(id)
                .and_then(|node| node.automation_id())
                .is_some_and(|automation_id| automation_id == "sidebar-scroll")
        })
        .expect("sidebar scroll");
    let status_bar = tree
        .traverse()
        .iter()
        .copied()
        .find(|&id| {
            tree.get(id)
                .and_then(|node| node.automation_id())
                .is_some_and(|automation_id| automation_id == "app-status-bar")
        })
        .expect("app status bar");
    let scroll_frame = tree
        .get(sidebar_scroll)
        .expect("sidebar scroll node")
        .frame();
    let status_frame = tree.get(status_bar).expect("status bar node").frame();
    let scroll = tree
        .get(sidebar_scroll)
        .expect("sidebar scroll node")
        .component()
        .as_any()
        .downcast_ref::<ScrollView>()
        .expect("sidebar ScrollView");

    assert!((scroll_frame.w - SIDEBAR_W).abs() < 2.0);
    assert!(
        scroll_frame.y + scroll_frame.h <= status_frame.y + 0.5,
        "sidebar scroll must stay above status bar: scroll={scroll_frame:?} status={status_frame:?}"
    );
    assert!(
        scroll.max_scroll_y() > 0.0,
        "compact sidebar must expose vertical navigation scroll"
    );
}

#[test]
fn data_table_demo_is_bounded_before_following_sections() {
    let active = State::new(PAGE_DATA);
    let timer_ticks = State::new(0u32);
    let root = app_shell(active, timer_ticks);
    let mut tree = ViewAdapter::build(root);
    if let Some(root) = tree.root_mut() {
        root.set_frame(Rect::new(0.0, 0.0, INIT_W as f32, INIT_H as f32));
    }
    tree.layout();

    let table_frame = tree
        .find_all_by_type::<Table>()
        .into_iter()
        .map(|(id, _)| tree.get(id).expect("table node").frame())
        .next()
        .expect("data demo Table");
    let selectable_frame = tree
        .find_all_by_type::<SelectableList>()
        .into_iter()
        .map(|(id, _)| tree.get(id).expect("selectable list node").frame())
        .next()
        .expect("data demo SelectableList");

    assert!(
        table_frame.w > 0.0 && table_frame.w <= INNER_W,
        "{table_frame:?}"
    );
    assert!((table_frame.h - 200.0).abs() < 1.0, "{table_frame:?}");
    assert!(
        table_frame.y + table_frame.h <= selectable_frame.y,
        "Table must not paint through following sections: table={table_frame:?} selectable={selectable_frame:?}"
    );
}

#[test]
fn input_demo_form_owns_and_contains_its_items() {
    let active = State::new(PAGE_INPUT);
    let timer_ticks = State::new(0u32);
    let root = app_shell(active, timer_ticks);
    let mut tree = ViewAdapter::build(root);
    if let Some(root) = tree.root_mut() {
        root.set_frame(Rect::new(0.0, 0.0, INIT_W as f32, INIT_H as f32));
    }
    tree.layout();

    let (form_id, _) = tree
        .find_all_by_type::<Form>()
        .into_iter()
        .next()
        .expect("input demo Form");
    let form_node = tree.get(form_id).expect("form node");
    let form_frame = form_node.frame();
    assert_eq!(form_node.children().len(), 3);
    for child_id in form_node.children() {
        let child = tree.get(*child_id).expect("form item node");
        assert!(child.component().as_any().is::<FormItem>());
        assert_eq!(child.children().len(), 1);
        assert!(tree
            .get(child.children()[0])
            .expect("form input node")
            .component()
            .as_any()
            .is::<Input>());
        let item_frame = child.frame();
        assert!(
            item_frame.y >= form_frame.y
                && item_frame.y + item_frame.h <= form_frame.y + form_frame.h + 0.5,
            "FormItem must stay inside Form: form={form_frame:?} item={item_frame:?}"
        );
    }
}

/// 首页：放大后 ScrollView/内容列/提示条须跟窗口；section 色条不得吞满整行。
/// 调试 overlay：紫框=壳层，蓝框=内容列——二者宽度应接近（不再卡 INNER_W）。
#[test]
fn home_page_banners_fill_content_width_after_resize() {
    let active = State::new(0usize);
    let timer_ticks = State::new(0u32);
    let root = app_shell(active, timer_ticks);
    let mut tree = ViewAdapter::build(root);
    if let Some(r) = tree.root_mut() {
        r.set_frame(Rect::new(0.0, 0.0, INIT_W as f32, INIT_H as f32));
    }
    tree.layout();

    tree.dispatch_event(&SystemEvent::Resize {
        width: 1600.0,
        height: 1000.0,
    });
    tree.layout();

    let scroll = tree
        .find_all_by_type::<ScrollView>()
        .into_iter()
        .find(|(id, _)| {
            tree.get(*id)
                .is_some_and(|n| n.frame().x >= SIDEBAR_W - 2.0 && n.frame().w > 900.0)
        })
        .expect("content ScrollView");
    let scroll_w = tree.get(scroll.0).unwrap().frame().w;
    let page_col = tree.get(scroll.0).unwrap().children()[0];
    let page_col_w = tree.get(page_col).unwrap().frame().w;
    let gutter = 8.0; // ScrollBar::gutter() = SB_W(6) + EDGE_PAD(2)
                      // 有纵向滚动条时内容列扣除 gutter；无条时与视口同宽。
    let width_ok =
        (page_col_w - scroll_w).abs() < 2.0 || (scroll_w - page_col_w - gutter).abs() < 2.0;
    assert!(
        page_col_w > 1200.0 && width_ok,
        "page column should fill ScrollView (minus scrollbar gutter), col={page_col_w} scroll={scroll_w}"
    );

    let child_frames: Vec<_> = tree
        .get(page_col)
        .unwrap()
        .children()
        .iter()
        .filter_map(|&cid| tree.get(cid).map(|n| n.frame()))
        .collect();
    let max_banner_w = child_frames
        .iter()
        .map(|f| f.w)
        .filter(|&w| w > 200.0)
        .fold(0.0f32, f32::max);
    assert!(
        max_banner_w > INNER_W + 50.0 && max_banner_w > page_col_w * 0.85,
        "banners should fill page column, banner={max_banner_w} col={page_col_w}"
    );

    // section 色条：约 3×14，不得变成整行蓝带
    let accents: Vec<_> = tree
        .find_all_by_type::<uix::ui::widgets::Container>()
        .into_iter()
        .filter_map(|(id, _)| {
            let f = tree.get(id)?.frame();
            if f.x >= SIDEBAR_W && f.h > 10.0 && f.h < 20.0 && f.w > 0.0 && f.w < 12.0 {
                Some(f)
            } else {
                None
            }
        })
        .collect();
    assert!(
        !accents.is_empty(),
        "expected narrow section accent bars (~3px), got none — likely still flex_grow=1"
    );
}

/// 应用能力页：放大后 info_note 须接近内容列宽度（Stretch），不能卡在 INNER_W。
#[test]
fn app_page_info_notes_fill_content_width_after_resize() {
    let active = State::new(PAGE_APP);
    let timer_ticks = State::new(0u32);
    let root = app_shell(active, timer_ticks);
    let mut tree = ViewAdapter::build(root);
    if let Some(r) = tree.root_mut() {
        r.set_frame(Rect::new(0.0, 0.0, INIT_W as f32, INIT_H as f32));
    }
    tree.layout();

    tree.dispatch_event(&SystemEvent::Resize {
        width: 1600.0,
        height: 1000.0,
    });
    tree.layout();

    let scroll = tree
        .find_all_by_type::<ScrollView>()
        .into_iter()
        .find(|(id, _)| {
            tree.get(*id)
                .is_some_and(|n| n.frame().x >= SIDEBAR_W - 2.0 && n.frame().w > 900.0)
        })
        .expect("content ScrollView");
    let page_col = tree.get(scroll.0).unwrap().children()[0];
    let page_col_w = tree.get(page_col).unwrap().frame().w;
    assert!(
        page_col_w > 1200.0,
        "page column should follow wide viewport, got {page_col_w}"
    );

    // 直接子节点含 gap / section_title / panel；info_note 在 panel 内。
    // 断言抬升面板（或任意内容块）在宽视口下被 Stretch 拉满，而不是依赖
    // 脆弱的高度启发式（sample_block 修好后短面板不再落入 20..80）。
    let child_frames: Vec<Rect> = tree
        .get(page_col)
        .unwrap()
        .children()
        .iter()
        .filter_map(|&cid| tree.get(cid).map(|n| n.frame()))
        .collect();
    let max_block_w = child_frames
        .iter()
        .filter(|f| f.h > 40.0)
        .map(|f| f.w)
        .fold(0.0f32, f32::max);
    assert!(
        max_block_w > INNER_W + 50.0 && max_block_w > page_col_w * 0.85,
        "content blocks should fill page column, block={max_block_w} col={page_col_w}, children={child_frames:?}"
    );
}

/// 改变窗口大小后，侧栏固定宽、内容区与 ScrollView 须跟随新客户区。
#[test]
fn window_resize_updates_shell_content() {
    let active = State::new(0usize);
    let timer_ticks = State::new(0u32);
    let root = app_shell(active, timer_ticks);
    let mut tree = ViewAdapter::build(root);
    let root_id = tree.root_id().expect("root");
    if let Some(r) = tree.root_mut() {
        r.set_frame(Rect::new(0.0, 0.0, INIT_W as f32, INIT_H as f32));
    }
    tree.layout();

    let scroll_h0 = tree
        .find_all_by_type::<ScrollView>()
        .into_iter()
        .filter_map(|(id, _)| tree.get(id).map(|n| n.frame()))
        .filter(|f| f.x >= SIDEBAR_W - 2.0)
        .map(|f| f.h)
        .fold(0.0f32, f32::max);

    tree.dispatch_event(&SystemEvent::Resize {
        width: 1600.0,
        height: 1000.0,
    });
    tree.layout();

    let root_frame = tree.get(root_id).expect("root").frame();
    assert!(
        (root_frame.w - 1600.0).abs() < 0.5 && (root_frame.h - 1000.0).abs() < 0.5,
        "root should follow resize, got {}x{}",
        root_frame.w,
        root_frame.h
    );

    let main_row = tree.get(root_id).unwrap().children()[0];
    let children = tree.get(main_row).unwrap().children().to_vec();
    let nav_frame = tree.get(children[0]).unwrap().frame();
    let content_frame = tree.get(children[1]).unwrap().frame();
    assert!(
        (nav_frame.w - SIDEBAR_W).abs() < 2.0,
        "sidebar width after resize: {}",
        nav_frame.w
    );
    assert!(
        content_frame.w > 1300.0,
        "content should widen with window, got {}",
        content_frame.w
    );

    // 页面内容列（ScrollView 子项）须随 viewport 变宽，不能卡在 INNER_W。
    let page_col_w = tree
        .find_all_by_type::<ScrollView>()
        .into_iter()
        .filter_map(|(id, _)| {
            let frame = tree.get(id)?.frame();
            if frame.x < SIDEBAR_W - 2.0 {
                return None;
            }
            let child = tree.get(id)?.children().first().copied()?;
            Some(tree.get(child)?.frame().w)
        })
        .fold(0.0f32, f32::max);
    assert!(
        page_col_w > 1300.0,
        "scroll content column should fill viewport width, got {page_col_w}"
    );

    let scroll_h1 = tree
        .find_all_by_type::<ScrollView>()
        .into_iter()
        .filter_map(|(id, _)| tree.get(id).map(|n| n.frame()))
        .filter(|f| f.x >= SIDEBAR_W - 2.0)
        .map(|f| f.h)
        .fold(0.0f32, f32::max);
    assert!(
        scroll_h1 > scroll_h0 + 50.0,
        "content ScrollView should grow with window, {scroll_h1} vs {scroll_h0}"
    );

    tree.dispatch_event(&SystemEvent::Resize {
        width: 900.0,
        height: 600.0,
    });
    tree.layout();
    let root_frame2 = tree.get(root_id).expect("root").frame();
    assert!(
        (root_frame2.w - 900.0).abs() < 0.5 && (root_frame2.h - 600.0).abs() < 0.5,
        "root should shrink with window, got {}x{}",
        root_frame2.w,
        root_frame2.h
    );
    let scroll_h2 = tree
        .find_all_by_type::<ScrollView>()
        .into_iter()
        .filter_map(|(id, _)| tree.get(id).map(|n| n.frame()))
        .filter(|f| f.x >= SIDEBAR_W - 2.0)
        .map(|f| f.h)
        .fold(0.0f32, f32::max);
    assert!(
        scroll_h2 < scroll_h1 - 50.0,
        "content ScrollView should shrink with window, {scroll_h2} vs {scroll_h1}"
    );
}

#[test]
fn general_page_has_buttons() {
    let tk = DesignTokens::antd_light();
    let timer_ticks = State::new(0u32);
    let active = State::new(2usize);
    let ctx = crate::demos::DemoCtx::new(&tk, &timer_ticks, Some(&active));
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
fn feedback_modal_traps_keyboard_focus_between_automation_targets() {
    use uix::core::Point;
    use uix::native::traits::input::{KeyCode, KeyMod, MouseButton};
    use uix::ui::EventResult;

    let active = State::new(PAGE_FEEDBACK);
    let timer_ticks = State::new(0u32);
    let root = app_shell(active, timer_ticks);
    let mut tree = ViewAdapter::build(root);
    if let Some(root) = tree.root_mut() {
        root.set_frame(Rect::new(0.0, 0.0, INIT_W as f32, INIT_H as f32));
    }
    tree.layout();

    let modal = tree
        .traverse()
        .iter()
        .copied()
        .find(|&id| {
            tree.get(id)
                .and_then(|node| node.automation_id())
                .is_some_and(|automation_id| automation_id == "feedback-focus-modal")
        })
        .expect("feedback focus modal automation target");
    let modal_frame = tree.get(modal).expect("feedback modal node").frame();
    assert!(
        modal_frame.w > 0.0 && modal_frame.h > 0.0,
        "closed modal trigger must be visible on the initial feedback viewport"
    );
    let modal_click = Point::new(
        modal_frame.x + modal_frame.w * 0.5,
        modal_frame.y + modal_frame.h * 0.5,
    );
    assert_eq!(
        tree.dispatch_event(&SystemEvent::PointerDown {
            pos: modal_click,
            button: MouseButton::Left,
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );
    assert!(
        tree.overlay_stack()
            .top()
            .is_none_or(|entry| entry.owner() != modal),
        "PointerDown alone must not activate the Modal focus trap"
    );
    assert_eq!(
        tree.dispatch_event(&SystemEvent::PointerUp {
            pos: modal_click,
            button: MouseButton::Left,
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );
    tree.layout();
    assert!(
        tree.overlay_stack()
            .top()
            .is_some_and(|entry| entry.owner() == modal && entry.traps_focus()),
        "opened feedback Modal must own the top focus trap"
    );

    assert_eq!(
        tree.dispatch_event(&SystemEvent::KeyDown {
            key: KeyCode::Tab,
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );
    let first = tree
        .managers()
        .focus
        .focused_component()
        .and_then(|id| tree.get(id))
        .and_then(|node| node.automation_id());
    assert_eq!(first, Some("feedback-modal-cancel"));

    assert_eq!(
        tree.dispatch_event(&SystemEvent::KeyDown {
            key: KeyCode::Tab,
            mods: KeyMod::SHIFT,
        }),
        EventResult::Handled
    );
    let reverse = tree
        .managers()
        .focus
        .focused_component()
        .and_then(|id| tree.get(id))
        .and_then(|node| node.automation_id());
    assert_eq!(reverse, Some("feedback-modal-confirm"));
}

/// sample_block（嵌套无固定尺寸 Space）不得把 Label 压成 0×0。
#[test]
fn general_page_label_color_samples_have_frames() {
    let tk = DesignTokens::antd_light();
    let timer_ticks = State::new(0u32);
    let active = State::new(PAGE_GENERAL);
    let ctx = crate::demos::DemoCtx::new(&tk, &timer_ticks, Some(&active));
    let root = crate::demos::general::page_general(&ctx);
    let mut tree = ViewAdapter::build(root);
    if let Some(r) = tree.root_mut() {
        r.set_frame(Rect::new(0.0, 0.0, INIT_W as f32, INIT_H as f32));
    }
    tree.layout();

    for name in ["Primary", "Secondary", "Tertiary"] {
        let frames: Vec<Rect> = tree
            .find_all_by_type::<Label>()
            .into_iter()
            .filter_map(|(id, l)| {
                if l.text() != name {
                    return None;
                }
                tree.get(id).map(|n| n.frame())
            })
            .collect();
        assert!(
            frames.iter().any(|f| f.w > 0.0 && f.h > 0.0),
            "Label sample `{name}` must be visible, got frames {frames:?}"
        );
    }
}

#[test]
fn gallery_page_lists_coverage() {
    let tk = DesignTokens::antd_light();
    let timer_ticks = State::new(0u32);
    let active = State::new(10usize);
    let ctx = crate::demos::DemoCtx::new(&tk, &timer_ticks, Some(&active));
    let root = crate::demos::gallery::page_gallery(&ctx);
    let tree = ViewAdapter::build(root);
    assert!(tree.root_id().is_some());
}

/// 窄窗通用页：`flow_row`（wrap）应变为多行，且内容列不得撑破 ScrollView 视口宽。
#[test]
fn general_page_flow_row_wraps_instead_of_horizontal_scroll() {
    let active = State::new(PAGE_GENERAL);
    let timer_ticks = State::new(0u32);
    let root = app_shell(active, timer_ticks);
    let mut tree = ViewAdapter::build(root);
    if let Some(r) = tree.root_mut() {
        // 侧栏 220 + 窄内容区：Icon 行（8 个 sample_block）在 wrap 下应折行。
        r.set_frame(Rect::new(0.0, 0.0, 640.0, 800.0));
    }
    tree.layout();

    let scroll = tree
        .find_all_by_type::<ScrollView>()
        .into_iter()
        .find(|(id, _)| {
            tree.get(*id)
                .is_some_and(|n| n.frame().x >= SIDEBAR_W - 2.0 && n.frame().h > 100.0)
        })
        .expect("content ScrollView");
    let scroll_frame = tree.get(scroll.0).unwrap().frame();
    let page_col = tree.get(scroll.0).unwrap().children()[0];
    let page_col_frame = tree.get(page_col).unwrap().frame();

    let icon_spaces: Vec<_> = tree
        .find_all_by_type::<uix::ui::widgets::Space>()
        .into_iter()
        .filter_map(|(id, _space)| {
            let kids = tree.get(id)?.children().to_vec();
            if kids.len() < 6 {
                return None;
            }
            let labels: Vec<String> = kids
                .iter()
                .filter_map(|&cid| {
                    let sample = tree.get(cid)?;
                    let label_id = *sample.children().first()?;
                    tree.get(label_id)?
                        .component()
                        .as_any()
                        .downcast_ref::<Label>()
                        .map(|l| l.text().to_string())
                })
                .collect();
            if labels.iter().any(|t| t == "search") && labels.iter().any(|t| t == "home") {
                Some((id, kids))
            } else {
                None
            }
        })
        .collect();
    assert!(
        !icon_spaces.is_empty(),
        "expected Icon Lucide flow_row on general page"
    );
    let (space_id, kids) = &icon_spaces[0];
    let space_frame = tree.get(*space_id).unwrap().frame();
    let child_frames: Vec<Rect> = kids
        .iter()
        .filter_map(|&id| tree.get(id).map(|n| n.frame()))
        .collect();
    let unique_ys = {
        let mut ys: Vec<i32> = child_frames.iter().map(|f| f.y.round() as i32).collect();
        ys.sort_unstable();
        ys.dedup();
        ys
    };
    assert!(
        unique_ys.len() > 1,
        "Icon flow_row should wrap under narrow content ({:.0}px), ys={unique_ys:?}, space={space_frame:?}, kids={child_frames:?}",
        scroll_frame.w
    );
    assert!(
        (page_col_frame.w - scroll_frame.w).abs() < 10.0
            || page_col_frame.w <= scroll_frame.w + 1.0,
        "page column should stay within scroll viewport, page_col={page_col_frame:?}, scroll={scroll_frame:?}"
    );
}

/// 内容超出视口时 ScrollView 须产生 max_scroll（否则窗口裁切且无滚动条）。
#[test]
fn content_scrollview_reports_overflow_scroll_range() {
    let active = State::new(PAGE_GENERAL);
    let timer_ticks = State::new(0u32);
    let root = app_shell(active, timer_ticks);
    let mut tree = ViewAdapter::build(root);
    if let Some(r) = tree.root_mut() {
        // 矮窗：通用页内容必然超出视口。
        r.set_frame(Rect::new(0.0, 0.0, INIT_W as f32, 480.0));
    }
    tree.layout();

    let scroll = tree
        .find_all_by_type::<ScrollView>()
        .into_iter()
        .find(|(id, _)| {
            tree.get(*id)
                .is_some_and(|n| n.frame().x >= SIDEBAR_W - 2.0 && n.frame().h > 100.0)
        })
        .expect("content ScrollView");
    let scroll_frame = tree.get(scroll.0).unwrap().frame();
    let sv: &ScrollView = tree
        .get(scroll.0)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref()
        .unwrap();
    assert!(
        scroll_frame.h < 400.0,
        "ScrollView viewport must stay within client area, got h={}",
        scroll_frame.h
    );
    assert!(
        sv.max_scroll_y() > 50.0,
        "tall page content must enable vertical scroll, max_y={}",
        sv.max_scroll_y()
    );
}

/// 窄窗首页固定宽快捷导航须产生横向滚动范围。
#[test]
fn home_narrow_window_enables_horizontal_scroll() {
    let active = State::new(0usize);
    let timer_ticks = State::new(0u32);
    let root = app_shell(active, timer_ticks);
    let mut tree = ViewAdapter::build(root);
    if let Some(r) = tree.root_mut() {
        // 侧栏 220 + 窄内容区，三块 200 宽导航应溢出。
        r.set_frame(Rect::new(0.0, 0.0, 640.0, 800.0));
    }
    tree.layout();

    let scroll = tree
        .find_all_by_type::<ScrollView>()
        .into_iter()
        .find(|(id, _)| {
            tree.get(*id)
                .is_some_and(|n| n.frame().x >= SIDEBAR_W - 2.0)
        })
        .expect("content ScrollView");
    let sv: &ScrollView = tree
        .get(scroll.0)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref()
        .unwrap();
    assert!(
        sv.max_scroll_x() > 0.0,
        "narrow home nav tiles must enable horizontal scroll, max_x={}",
        sv.max_scroll_x()
    );
}

/// 640×480 的「其他」页必须保留 Transfer/Upload，并能用纵向滚动条抵达溢出内容。
#[test]
fn other_page_narrow_window_keeps_content_and_vertical_scrollbar_interactive() {
    use uix::core::Point;
    use uix::draw::engine::cpu::noop_canvas_2d::NoopCanvas2D;
    use uix::draw::font::font_service::FontService;
    use uix::draw::image::ImageService;
    use uix::draw::painting::{PaintContext, PaintPass};
    use uix::draw::spatial::Orientation;
    use uix::draw::FontHandle;
    use uix::native::traits::input::{KeyMod, MouseButton};
    use uix::ui::theme::DesignTokens;
    use uix::ui::traits::WidgetRender;
    use uix::ui::{EventResult, Transfer, Upload};

    let active = State::new(PAGE_OTHER);
    let timer_ticks = State::new(0u32);
    let root = app_shell(active, timer_ticks);
    let mut tree = ViewAdapter::build(root);
    if let Some(root) = tree.root_mut() {
        root.set_frame(Rect::new(0.0, 0.0, 640.0, 480.0));
    }
    tree.layout();

    let scroll = tree
        .find_all_by_type::<ScrollView>()
        .into_iter()
        .find(|(id, _)| {
            tree.get(*id)
                .is_some_and(|node| node.frame().x >= SIDEBAR_W - 2.0)
        })
        .expect("other-page content ScrollView");
    let scroll_id = scroll.0;
    let scroll_frame = tree.get(scroll_id).expect("ScrollView node").frame();

    for (name, id) in [
        (
            "Transfer",
            tree.find_all_by_type::<Transfer>()
                .into_iter()
                .map(|(id, _)| id)
                .next()
                .expect("Transfer subtree"),
        ),
        (
            "Upload",
            tree.find_all_by_type::<Upload>()
                .into_iter()
                .map(|(id, _)| id)
                .next()
                .expect("Upload subtree"),
        ),
    ] {
        let frame = tree.get(id).expect("component node").frame();
        assert!(
            frame.w > 0.0 && frame.h > 0.0,
            "{name} must be laid out and locatable, got {frame:?}"
        );
    }

    // 真实帧会先走此渲染路径；它记录 ScrollView 的当前 viewport，供拖拽命中使用。
    let mut canvas = NoopCanvas2D;
    let fonts = FontService::new();
    let images = ImageService::new();
    let tokens = DesignTokens::antd_light();
    let mut ctx = PaintContext::new_for_test(
        &mut canvas,
        FontHandle::default(),
        &fonts,
        &images,
        &tokens,
        96.0,
        1.0,
        Orientation::YDown,
        640,
        480,
    );
    let scroll_view = tree
        .get(scroll_id)
        .expect("ScrollView node")
        .component()
        .as_any()
        .downcast_ref::<ScrollView>()
        .expect("ScrollView component");
    WidgetRender::render(scroll_view, scroll_frame, &mut ctx, &tree);
    ctx.set_paint_pass(PaintPass::AfterChildren);
    WidgetRender::render(scroll_view, scroll_frame, &mut ctx, &tree);

    let (max_x, max_y) = tree
        .get(scroll_id)
        .expect("ScrollView node")
        .component()
        .as_any()
        .downcast_ref::<ScrollView>()
        .map(|view| (view.max_scroll_x(), view.max_scroll_y()))
        .expect("ScrollView component");
    assert!(
        max_y > 0.0,
        "640×480 other page must retain vertical access to overflow content, got y={max_y}"
    );
    assert!(
        max_x <= 0.5,
        "Transfer must fit the narrow content viewport instead of leaving a false horizontal overflow, got x={max_x}"
    );

    let vertical_thumb = Point::new(scroll_frame.x + scroll_frame.w - 4.0, scroll_frame.y + 12.0);
    assert_eq!(
        tree.hit_test(vertical_thumb),
        Some(scroll_id),
        "vertical scrollbar gutter must target the content ScrollView"
    );
    assert_eq!(
        tree.dispatch_event(&SystemEvent::PointerDown {
            pos: vertical_thumb,
            button: MouseButton::Left,
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );
    assert_eq!(
        tree.dispatch_event(&SystemEvent::PointerMove {
            pos: Point::new(vertical_thumb.x, vertical_thumb.y + 48.0),
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );
    let scroll_y = tree
        .get(scroll_id)
        .expect("ScrollView node")
        .component()
        .as_any()
        .downcast_ref::<ScrollView>()
        .expect("ScrollView component")
        .scroll_y();
    assert!(
        scroll_y > 0.0,
        "vertical scrollbar drag must change scroll_y"
    );
    assert_eq!(
        tree.dispatch_event(&SystemEvent::PointerUp {
            pos: Point::new(vertical_thumb.x, vertical_thumb.y + 48.0),
            button: MouseButton::Left,
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );
}

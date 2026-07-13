use super::*;
use crate::common::page::{
    INIT_H, INIT_W, INNER_W, PAGE_APP, PAGE_COUNT, PAGE_GENERAL, PAGE_OTHER, PAGE_TITLES, SIDEBAR_W,
};
use uix::prelude::{dynamic_label, Button, DesignTokens, Label, Rect, State, SystemEvent};
use uix::ui::test_harness::{ViewAdapter, WidgetCore};
use uix::ui::traits::WidgetLayout;
use uix::ui::widgets::ScrollView;

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

/// 首页：放大后 ScrollView/内容列/提示条须跟窗口；section 色条不得吞满整行。
/// 调试 overlay：紫框=壳层，蓝框=内容列——二者宽度应接近（不再卡 INNER_W）。
#[test]
fn home_page_banners_fill_content_width_after_resize() {
    let active = State::new(0usize);
    let timer_ticks = State::new(0u32);
    let anim_time = State::new(0.0f32);
    let root = app_shell(active, timer_ticks, anim_time);
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
    let anim_time = State::new(0.0f32);
    let root = app_shell(active, timer_ticks, anim_time);
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
    let anim_time = State::new(0.0f32);
    let root = app_shell(active, timer_ticks, anim_time);
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

/// sample_block（嵌套无固定尺寸 Space）不得把 Label 压成 0×0。
#[test]
fn general_page_label_color_samples_have_frames() {
    let tk = DesignTokens::antd_light();
    let timer_ticks = State::new(0u32);
    let anim_time = State::new(0.0f32);
    let active = State::new(PAGE_GENERAL);
    let ctx = crate::demos::DemoCtx::new(&tk, &timer_ticks, &anim_time, Some(&active));
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
    let anim_time = State::new(0.0f32);
    let active = State::new(10usize);
    let ctx = crate::demos::DemoCtx::new(&tk, &timer_ticks, &anim_time, Some(&active));
    let root = crate::demos::gallery::page_gallery(&ctx);
    let tree = ViewAdapter::build(root);
    assert!(tree.root_id().is_some());
}

/// 窄窗通用页：`flow_row`（wrap）应变为多行，且内容列不得撑破 ScrollView 视口宽。
#[test]
fn general_page_flow_row_wraps_instead_of_horizontal_scroll() {
    let active = State::new(PAGE_GENERAL);
    let timer_ticks = State::new(0u32);
    let anim_time = State::new(0.0f32);
    let root = app_shell(active, timer_ticks, anim_time);
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
    let anim_time = State::new(0.0f32);
    let root = app_shell(active, timer_ticks, anim_time);
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
    let anim_time = State::new(0.0f32);
    let root = app_shell(active, timer_ticks, anim_time);
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
    let anim_time = State::new(0.0f32);
    let root = app_shell(active, timer_ticks, anim_time);
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
    let mut ctx = PaintContext::new(
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

/// 模拟主循环：reconcile → layout（不手动重设 root frame），断言内容区 ScrollView 仍有高度。
#[test]
fn page_switch_keeps_content_scrollview_height() {
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
    let anim_for_reconcile = anim_time.clone();
    let root = ViewAdapter::capture_root(move || {
        app_shell(
            active_for_reconcile.clone(),
            timer_for_reconcile.clone(),
            anim_for_reconcile.clone(),
        )
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
    let anim_time = State::new(0.0f32);
    let active = State::new(0usize);
    let ctx = crate::demos::DemoCtx::new(&tk, &timer_ticks, &anim_time, Some(&active));
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
    let anim_time = State::new(0.0f32);
    let home_count = State::new(0i32);
    let runtime_count = State::new(0i32);
    let theme_control = ThemeControl::default();

    let root = ViewAdapter::capture_root(|| {
        app_shell_with_counters(
            active.clone(),
            timer_ticks.clone(),
            anim_time.clone(),
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
            anim_time.clone(),
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
            anim_time.clone(),
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

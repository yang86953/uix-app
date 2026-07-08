use super::*;

#[test]
fn page_titles_and_build() {
    assert_eq!(PAGE_TITLES.len(), 8);
    let tk = DesignTokens::antd_light();
    for i in 0..PAGE_TITLES.len() {
        let _node = build_page(i, &tk);
    }
}

/// 验证 demo 树布局后导航项与按钮行不重叠。
#[test]
fn demo_tree_layout_no_overlap() {
    use uix::ui::widgets::navigation::NavItem;

    let tk = DesignTokens::antd_light();
    let (root_node, _, _) = build_demo_tree(&tk, 0);
    let mut tree = WidgetTree::new();
    tree.build(root_node);
    if let Some(root) = tree.root_mut() {
        root.set_frame(Rect::new(0.0, 0.0, INIT_W as f32, INIT_H as f32));
    }

    let page_ids = collect_page_ids(&tree);

    for (i, &id) in page_ids.iter().enumerate() {
        if i != 0 {
            tree.set_visible(id, false);
        }
    }
    tree.layout();

    // 导航项应纵向错开
    let nav_item_ys: Vec<f32> = tree
        .find_all_by_type::<NavItem>()
        .into_iter()
        .filter_map(|(id, _)| tree.get(id).map(|n| n.frame().y))
        .collect();
    assert!(nav_item_ys.len() >= 2);
    let unique_ys: std::collections::HashSet<i32> =
        nav_item_ys.iter().map(|y| y.round() as i32).collect();
    assert_eq!(
        unique_ys.len(),
        nav_item_ys.len(),
        "nav overlap: {nav_item_ys:?}"
    );

    // 按钮行内 5 个按钮应横向错开
    let page0 = page_ids[0];
    let inner_container = tree
        .get(page0)
        .and_then(|p| p.children().get(1).copied())
        .and_then(|scroll| tree.get(scroll))
        .and_then(|s| s.children().first().copied())
        .expect("scroll inner");
    let button_row_id = tree
        .get(inner_container)
        .and_then(|n| n.children().get(6).copied())
        .expect("button row");
    let button_xs: Vec<f32> = tree
        .get(button_row_id)
        .map(|n| {
            n.children()
                .iter()
                .filter_map(|&cid| tree.get(cid).map(|c| c.frame().x))
                .collect()
        })
        .unwrap_or_default();
    let unique_xs: std::collections::HashSet<i32> =
        button_xs.iter().map(|x| x.round() as i32).collect();
    assert_eq!(
        unique_xs.len(),
        button_xs.len(),
        "button overlap: {button_xs:?}"
    );

    // 模拟 event_loop 启动：bind_invalidation 后 layout 仍应正确
    tree.bind_invalidation();
    tree.layout();

    let root_id = tree.root_id().expect("root");
    let root_children = tree.get(root_id).unwrap().children().to_vec();
    let nav_frame = tree.get(root_children[0]).unwrap().frame();
    let content_frame = tree.get(root_children[1]).unwrap().frame();
    assert!(
        (nav_frame.w - SIDEBAR_W).abs() < 2.0,
        "nav width {} should be ~{SIDEBAR_W}",
        nav_frame.w
    );
    assert!(
        content_frame.w > 900.0,
        "content width {} too narrow",
        content_frame.w
    );

    let nav_item_ys_after: Vec<f32> = tree
        .find_all_by_type::<NavItem>()
        .into_iter()
        .filter_map(|(id, _)| tree.get(id).map(|n| n.frame().y))
        .collect();
    let unique_after: std::collections::HashSet<i32> =
        nav_item_ys_after.iter().map(|y| y.round() as i32).collect();
    assert_eq!(
        unique_after.len(),
        nav_item_ys_after.len(),
        "nav overlap after bind: {nav_item_ys_after:?}"
    );
}

/// 首帧渲染后侧栏导航区应有非背景像素（验证 CPU 路径完整绘制）。
#[test]
fn demo_first_frame_renders_nav_sidebar() {
    use uix::draw::painting::ThemeSnapshot;
    use uix::draw::pipeline::{FrameRenderInput, FrameRenderer};
    use uix::ui::widgets::navigation::NavItem;

    let tk = DesignTokens::antd_light();
    let (root_node, _, _) = build_demo_tree(&tk, 0);
    let mut tree = WidgetTree::new();
    tree.build(root_node);
    if let Some(root) = tree.root_mut() {
        root.set_frame(Rect::new(0.0, 0.0, INIT_W as f32, INIT_H as f32));
    }

    for (i, &id) in collect_page_ids(&tree).iter().enumerate() {
        if i != 0 {
            tree.set_visible(id, false);
        }
    }

    // 模拟 event_loop 首帧
    tree.bind_invalidation();
    tree.mark_full_frame_dirty();
    tree.layout();

    let root_id = tree.root_id().expect("root");
    let nav_container = tree.get(root_id).unwrap().children()[0];
    let nav_frame = tree.get(nav_container).unwrap().frame();
    let content_frame = tree.get(root_id).unwrap().children()[1];
    let content_frame = tree.get(content_frame).unwrap().frame();
    assert!(
        (nav_frame.w - SIDEBAR_W).abs() < 2.0,
        "nav width wrong: {nav_frame:?}"
    );
    assert!(
        content_frame.x >= SIDEBAR_W - 1.0,
        "content overlaps nav: content={content_frame:?} nav={nav_frame:?}"
    );
    assert!(
        nav_frame.h > 100.0,
        "nav container height too small: {nav_frame:?}"
    );

    let nav_items: Vec<(ComponentId, Rect)> = tree
        .find_all_by_type::<NavItem>()
        .into_iter()
        .map(|(id, _)| (id, tree.get(id).unwrap().frame()))
        .collect();
    assert!(nav_items.len() >= 2, "expected nav items");
    for (id, f) in &nav_items {
        assert!(f.h > 1.0, "nav item {id} height zero: {f:?}");
        assert!(f.w > 1.0, "nav item {id} width zero: {f:?}");
    }

    let mut engine = SoftwareEngine::new();
    engine.initialize(INIT_W, INIT_H).unwrap();
    let mut renderer = FrameRenderer::new();
    let theme = ThemeSnapshot::new(&tk);
    let platform = uix::native::create_platform().expect("platform");
    let fs = create_font_service(platform.as_ref());
    let img = ImageService::new();
    let region = tree.dirty_region();

    renderer.render_frame(
        &mut engine,
        &tree,
        FrameRenderInput {
            rendered_first: false,
            dirty_region: &region,
            tree_version: tree.tree_version(),
            scroll_move: None,
            theme,
            font: fs.loaded_font_handle,
            font_service: &fs,
            image_service: &img,
            debug_mode: false,
            hover_pos: None,
            metrics: None,
        },
    );

    let canvas = engine.canvas_2d();
    let stride = canvas.width() as usize;
    let pixels = canvas.pixels_mut();

    // 在第二个导航项的文字/图标区域采样（背景与侧栏同色，跳过左侧背景区）
    let (_, second) = &nav_items[1];
    let sx = (second.x + 40.0) as i32;
    let sy = second.y as i32;
    let sw = (second.w - 40.0) as i32;
    let sh = second.h as i32;
    let bg = tk.color_bg_container.to_rgba();
    let mut non_bg = 0usize;
    for y in sy..(sy + sh).min(INIT_H) {
        for x in sx..(sx + sw).min(INIT_W) {
            let idx = y as usize * stride + x as usize;
            if idx < pixels.len() && pixels[idx] != bg {
                non_bg += 1;
            }
        }
    }
    assert!(
            non_bg > 10,
            "nav item 1 should have painted pixels, got {non_bg} non-bg (frame={second:?}, nav_container={nav_frame:?})"
        );

    // 默认激活项（index 0）也应有可见文字像素
    let (_, active) = &nav_items[0];
    let mut active_non_bg = 0usize;
    let ax = active.x as i32;
    let ay = active.y as i32;
    let aw = active.w as i32;
    let ah = active.h as i32;
    for y in ay..(ay + ah).min(INIT_H) {
        for x in ax..(ax + aw).min(INIT_W) {
            let idx = y as usize * stride + x as usize;
            if idx < pixels.len()
                && pixels[idx] != bg
                && pixels[idx] != tk.color_primary_bg.to_rgba()
            {
                active_non_bg += 1;
            }
        }
    }
    assert!(
        active_non_bg > 5,
        "active nav item should have text/icon pixels, got {active_non_bg}"
    );

    // 第二帧：节点已干净，依赖 DisplayList 重放，激活项文字/图标仍应可见
    tree.reset_invalidation();
    tree.mark_full_frame_dirty();
    let cached_region = tree.dirty_region();
    renderer.render_frame(
        &mut engine,
        &tree,
        FrameRenderInput {
            rendered_first: true,
            dirty_region: &cached_region,
            tree_version: tree.tree_version(),
            scroll_move: None,
            theme: ThemeSnapshot::new(&tk),
            font: fs.loaded_font_handle,
            font_service: &fs,
            image_service: &img,
            debug_mode: false,
            hover_pos: None,
            metrics: None,
        },
    );

    let canvas = engine.canvas_2d();
    let stride = canvas.width() as usize;
    let pixels = canvas.pixels_mut();
    let mut active_replay_non_bg = 0usize;
    for y in ay..(ay + ah).min(INIT_H) {
        for x in ax..(ax + aw).min(INIT_W) {
            let idx = y as usize * stride + x as usize;
            if idx < pixels.len()
                && pixels[idx] != bg
                && pixels[idx] != tk.color_primary_bg.to_rgba()
            {
                active_replay_non_bg += 1;
            }
        }
    }
    assert!(
            active_replay_non_bg > 5,
            "active nav item should keep text/icon after DisplayList replay, got {active_replay_non_bg}"
        );

    // 模拟第二帧：仅内容区 partial dirty（滚动/局部更新常见路径）
    tree.reset_invalidation();
    tree.invalidate_paint_rect(
        tree.root_id().unwrap(),
        Rect::new(200.0, 0.0, 1000.0, 800.0),
    );
    let partial = tree.dirty_region();
    renderer.render_frame(
        &mut engine,
        &tree,
        FrameRenderInput {
            rendered_first: true,
            dirty_region: &partial,
            tree_version: tree.tree_version(),
            scroll_move: None,
            theme: ThemeSnapshot::new(&tk),
            font: fs.loaded_font_handle,
            font_service: &fs,
            image_service: &img,
            debug_mode: false,
            hover_pos: None,
            metrics: None,
        },
    );

    let canvas = engine.canvas_2d();
    let stride = canvas.width() as usize;
    let pixels = canvas.pixels_mut();
    let mut non_bg_after_partial = 0usize;
    for y in sy..(sy + sh).min(INIT_H) {
        for x in sx..(sx + sw).min(INIT_W) {
            let idx = y as usize * stride + x as usize;
            if idx < pixels.len() && pixels[idx] != bg {
                non_bg_after_partial += 1;
            }
        }
    }
    assert!(
        non_bg_after_partial > 10,
        "nav should survive partial content repaint, got {non_bg_after_partial} non-bg"
    );
}

/// 模拟 hover 触发的局部重绘，顶部不应出现透底/layout 底色接缝。
#[test]
fn demo_nav_hover_partial_repaint_no_top_seam() {
    use uix::core::Point;
    use uix::draw::painting::ThemeSnapshot;
    use uix::draw::pipeline::{FrameRenderInput, FrameRenderer};
    use uix::native::traits::input::KeyMod;
    use uix::ui::widgets::navigation::NavItem;
    use uix::ui::SystemEvent;

    let tk = DesignTokens::antd_light();
    let (root_node, _, _) = build_demo_tree(&tk, 0);
    let mut tree = WidgetTree::new();
    tree.build(root_node);
    if let Some(root) = tree.root_mut() {
        root.set_frame(Rect::new(0.0, 0.0, INIT_W as f32, INIT_H as f32));
    }
    for (i, &id) in collect_page_ids(&tree).iter().enumerate() {
        if i != 0 {
            tree.set_visible(id, false);
        }
    }
    tree.bind_invalidation();
    tree.layout();

    let mut engine = SoftwareEngine::new();
    engine.initialize(INIT_W, INIT_H).unwrap();
    let mut renderer = FrameRenderer::new();
    let platform = uix::native::create_platform().expect("platform");
    let fs = create_font_service(platform.as_ref());
    let img = ImageService::new();

    // 首帧建立 DisplayList 缓存
    tree.mark_full_frame_dirty();
    let first_region = tree.dirty_region();
    renderer.render_frame(
        &mut engine,
        &tree,
        FrameRenderInput {
            rendered_first: false,
            dirty_region: &first_region,
            tree_version: tree.tree_version(),
            scroll_move: None,
            theme: ThemeSnapshot::new(&tk),
            font: fs.loaded_font_handle,
            font_service: &fs,
            image_service: &img,
            debug_mode: false,
            hover_pos: None,
            metrics: None,
        },
    );
    tree.reset_invalidation();

    let nav_items: Vec<(ComponentId, Rect)> = tree
        .find_all_by_type::<NavItem>()
        .into_iter()
        .map(|(id, _)| (id, tree.get(id).unwrap().frame()))
        .collect();
    let (hover_id, hover_frame) = nav_items[2];
    let hover_center = Point::new(
        hover_frame.x + hover_frame.w * 0.5,
        hover_frame.y + hover_frame.h * 0.5,
    );
    let _ = tree.dispatch_event(&SystemEvent::PointerMove {
        pos: hover_center,
        mods: KeyMod::NONE,
    });
    let _ = hover_id;

    let hover_region = tree.dirty_region();
    assert!(
        !hover_region.is_empty(),
        "hover should produce paint invalidation"
    );
    renderer.render_frame(
        &mut engine,
        &tree,
        FrameRenderInput {
            rendered_first: true,
            dirty_region: &hover_region,
            tree_version: tree.tree_version(),
            scroll_move: None,
            theme: ThemeSnapshot::new(&tk),
            font: fs.loaded_font_handle,
            font_service: &fs,
            image_service: &img,
            debug_mode: false,
            hover_pos: Some(hover_center),
            metrics: None,
        },
    );

    let canvas = engine.canvas_2d();
    let stride = canvas.width() as usize;
    let pixels = canvas.pixels_mut();
    let top_y = hover_frame.y as i32;
    let mid_x = hover_center.x as i32;
    let idx = top_y as usize * stride + mid_x as usize;
    let px = pixels[idx];
    let layout_bg = tk.color_bg_layout.to_rgba();
    let transparent = 0u32;
    assert!(
        px != layout_bg && px != transparent,
        "hover item top edge should not expose layout/transparent seam, got {px:#010x}"
    );
    assert!(
        (px >> 24) >= 240,
        "hover item top pixel should be opaque, got {px:#010x}"
    );
}

use super::*;

#[test]
fn reconcile_list_patches_instance_and_syncs_config() {
    use crate::ui::widgets::List;

    let mut tree = ViewAdapter::build_nodes(ViewNode::leaf(List::new().items(vec!["old"])));
    let root_id = tree.root_id().expect("list root should exist");
    let before_ptr = tree
        .get(root_id)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<List>()
        .unwrap() as *const List;

    ViewAdapter::reconcile_nodes(
        &mut tree,
        ViewNode::leaf(
            List::new()
                .header("Header")
                .footer("Footer")
                .bordered(false)
                .size(ControlSize::Large)
                .items(vec!["Ada", "Grace"])
                .load_more("More"),
        ),
    );

    let list = tree
        .get(root_id)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<List>()
        .unwrap();
    assert_eq!(list as *const List, before_ptr);
    assert_eq!(
        list.snapshot_fields(),
        SnapshotFields::List {
            header: "Header".to_string(),
            footer: "Footer".to_string(),
            bordered: false,
            list_size: ControlSize::Large,
            items: vec!["Ada".to_string(), "Grace".to_string()],
            load_more_text: "More".to_string(),
        }
    );
}

#[test]
fn reconcile_layout_shell_patches_instance_and_syncs_config() {
    use crate::ui::widgets::{Content, Footer, Header, Layout, Sider};

    let mut tree = ViewAdapter::build_nodes(ViewNode::leaf(Layout::new()));
    let root_id = tree.root_id().expect("layout root should exist");
    let before_ptr = tree
        .get(root_id)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<Layout>()
        .unwrap() as *const Layout;

    ViewAdapter::reconcile_nodes(&mut tree, ViewNode::leaf(Layout::new().bg(Color::red())));

    let layout = tree
        .get(root_id)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<Layout>()
        .unwrap();
    assert_eq!(layout as *const Layout, before_ptr);
    assert_eq!(
        layout.snapshot_fields(),
        SnapshotFields::Layout {
            bg_color: Some(Color::red()),
        }
    );

    let mut tree = ViewAdapter::build_nodes(ViewNode::leaf(Header::new(32.0)));
    let root_id = tree.root_id().expect("header root should exist");
    let before_ptr = tree
        .get(root_id)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<Header>()
        .unwrap() as *const Header;

    ViewAdapter::reconcile_nodes(
        &mut tree,
        ViewNode::leaf(Header::new(48.0).bg(Color::blue())),
    );

    let header = tree
        .get(root_id)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<Header>()
        .unwrap();
    assert_eq!(header as *const Header, before_ptr);
    assert_eq!(
        header.snapshot_fields(),
        SnapshotFields::Header {
            height: 48.0,
            bg_color: Some(Color::blue()),
        }
    );

    let mut tree = ViewAdapter::build_nodes(ViewNode::leaf(Sider::new(160.0)));
    let root_id = tree.root_id().expect("sider root should exist");
    let before_ptr = tree
        .get(root_id)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<Sider>()
        .unwrap() as *const Sider;

    ViewAdapter::reconcile_nodes(
        &mut tree,
        ViewNode::leaf(
            Sider::new(240.0)
                .bg(Color::green())
                .collapsible(true)
                .collapsed(true)
                .collapsed_width(72.0),
        ),
    );

    let sider = tree
        .get(root_id)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<Sider>()
        .unwrap();
    assert_eq!(sider as *const Sider, before_ptr);
    assert_eq!(
        sider.snapshot_fields(),
        SnapshotFields::Sider {
            width: 240.0,
            bg_color: Some(Color::green()),
            collapsible: true,
            collapsed: true,
            collapsed_width: 72.0,
        }
    );

    let mut tree = ViewAdapter::build_nodes(ViewNode::leaf(Content::new()));
    let root_id = tree.root_id().expect("content root should exist");
    let before_ptr = tree
        .get(root_id)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<Content>()
        .unwrap() as *const Content;

    ViewAdapter::reconcile_nodes(&mut tree, ViewNode::leaf(Content::new().bg(Color::red())));

    let content = tree
        .get(root_id)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<Content>()
        .unwrap();
    assert_eq!(content as *const Content, before_ptr);
    assert_eq!(
        content.snapshot_fields(),
        SnapshotFields::Content {
            bg_color: Some(Color::red()),
        }
    );

    let mut tree = ViewAdapter::build_nodes(ViewNode::leaf(Footer::new(24.0)));
    let root_id = tree.root_id().expect("footer root should exist");
    let before_ptr = tree
        .get(root_id)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<Footer>()
        .unwrap() as *const Footer;

    ViewAdapter::reconcile_nodes(
        &mut tree,
        ViewNode::leaf(Footer::new(56.0).bg(Color::blue())),
    );

    let footer = tree
        .get(root_id)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<Footer>()
        .unwrap();
    assert_eq!(footer as *const Footer, before_ptr);
    assert_eq!(
        footer.snapshot_fields(),
        SnapshotFields::Footer {
            height: 56.0,
            bg_color: Some(Color::blue()),
        }
    );
}

#[test]
fn reconcile_bar_chart_patches_instance_and_syncs_config() {
    use crate::ui::widgets::{BarChart, BarData};

    let mut tree = ViewAdapter::build_nodes(ViewNode::leaf(BarChart::new()));
    let root_id = tree.root_id().expect("bar chart root should exist");
    let before_ptr = tree
        .get(root_id)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<BarChart>()
        .unwrap() as *const BarChart;
    let data = vec![
        BarData::new("A", 3.0, Color::red()),
        BarData::new("B", 8.0, Color::blue()),
    ];

    ViewAdapter::reconcile_nodes(
        &mut tree,
        ViewNode::leaf(
            BarChart::new()
                .data(data.clone())
                .width(260.0)
                .height(180.0)
                .max_value(10.0)
                .show_value(false)
                .bar_radius(6.0),
        ),
    );

    let chart = tree
        .get(root_id)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<BarChart>()
        .unwrap();
    assert_eq!(chart as *const BarChart, before_ptr);
    assert_eq!(
        chart.snapshot_fields(),
        SnapshotFields::BarChart {
            data,
            fixed_width: 260.0,
            fixed_height: 180.0,
            max_value: 10.0,
            show_value: false,
            bar_radius: 6.0,
        }
    );
}

#[test]
fn reconcile_line_chart_patches_instance_and_syncs_config() {
    use crate::ui::widgets::{LineChart, LineData};

    let mut tree = ViewAdapter::build_nodes(ViewNode::leaf(LineChart::new()));
    let root_id = tree.root_id().expect("line chart root should exist");
    let before_ptr = tree
        .get(root_id)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<LineChart>()
        .unwrap() as *const LineChart;
    let data = vec![LineData::new("Mon", 2.0), LineData::new("Tue", 5.0)];

    ViewAdapter::reconcile_nodes(
        &mut tree,
        ViewNode::leaf(
            LineChart::new()
                .data(data.clone())
                .width(320.0)
                .height(140.0)
                .line_color(Color::green())
                .max_value(8.0)
                .auto_min(true)
                .show_grid(false)
                .show_dots(false)
                .line_width(4.0),
        ),
    );

    let chart = tree
        .get(root_id)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<LineChart>()
        .unwrap();
    assert_eq!(chart as *const LineChart, before_ptr);
    assert_eq!(
        chart.snapshot_fields(),
        SnapshotFields::LineChart {
            data,
            fixed_width: 320.0,
            fixed_height: 140.0,
            line_color: Some(Color::green()),
            max_value: 8.0,
            auto_min: true,
            show_grid: false,
            show_dots: false,
            line_width: 4.0,
            dot_radius: 3.0,
        }
    );
}

#[test]
fn reconcile_pie_chart_patches_instance_and_syncs_config() {
    use crate::ui::widgets::{PieChart, PieData};

    let mut tree = ViewAdapter::build_nodes(ViewNode::leaf(PieChart::new()));
    let root_id = tree.root_id().expect("pie chart root should exist");
    let before_ptr = tree
        .get(root_id)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<PieChart>()
        .unwrap() as *const PieChart;
    let data = vec![
        PieData::new("Used", 70.0, Color::red()),
        PieData::new("Free", 30.0, Color::green()),
    ];

    ViewAdapter::reconcile_nodes(
        &mut tree,
        ViewNode::leaf(PieChart::new().data(data.clone()).size(220.0).donut(0.45)),
    );

    let chart = tree
        .get(root_id)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<PieChart>()
        .unwrap();
    assert_eq!(chart as *const PieChart, before_ptr);
    assert_eq!(
        chart.snapshot_fields(),
        SnapshotFields::PieChart {
            data,
            fixed_size: 220.0,
            hole_radius: 0.45,
        }
    );
}

#[test]
fn reconcile_qrcode_patches_instance_and_syncs_config() {
    use crate::ui::widgets::QRCode;

    let mut tree = ViewAdapter::build_nodes(ViewNode::leaf(QRCode::new("old")));
    let root_id = tree.root_id().expect("qrcode root should exist");
    let before_ptr = tree
        .get(root_id)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<QRCode>()
        .unwrap() as *const QRCode;

    ViewAdapter::reconcile_nodes(
        &mut tree,
        ViewNode::leaf(QRCode::new("new").size(96.0).error_level(0)),
    );

    let qrcode = tree
        .get(root_id)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<QRCode>()
        .unwrap();
    assert_eq!(qrcode as *const QRCode, before_ptr);
    assert_eq!(
        qrcode.snapshot_fields(),
        SnapshotFields::QRCode {
            value: "new".to_string(),
            size: 96.0,
            error_level: 0,
            module_count: 21,
            encoding_error: None,
        }
    );
}

#[test]
fn reconcile_watermark_patches_instance_and_syncs_config() {
    use crate::ui::widgets::Watermark;

    let mut tree = ViewAdapter::build_nodes(ViewNode::leaf(Watermark::new("old")));
    let root_id = tree.root_id().expect("watermark root should exist");
    let before_ptr = tree
        .get(root_id)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<Watermark>()
        .unwrap() as *const Watermark;

    ViewAdapter::reconcile_nodes(
        &mut tree,
        ViewNode::leaf(
            Watermark::new("new")
                .color(Color::blue())
                .font_size(18.0)
                .opacity(0.3)
                .rotate(-12.0)
                .gap(120.0, 90.0)
                .offset(8.0, 16.0),
        ),
    );

    let watermark = tree
        .get(root_id)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<Watermark>()
        .unwrap();
    assert_eq!(watermark as *const Watermark, before_ptr);
    assert_eq!(
        watermark.snapshot_fields(),
        SnapshotFields::Watermark {
            text: "new".to_string(),
            color: Color::blue(),
            font_size: 18.0,
            opacity: 0.3,
            rotate: -12.0,
            gap_x: 120.0,
            gap_y: 90.0,
            x_offset: 8.0,
            y_offset: 16.0,
        }
    );
}

#[test]
fn reconcile_affix_preserves_position_state_and_syncs_offset() {
    use crate::ui::widgets::Affix;

    let mut affix = Affix::new(10.0);
    affix.set_child_bounds(100.0, 36.0);
    affix.update_scroll(24.0);
    let mut tree = ViewAdapter::build_nodes(ViewNode::leaf(affix));
    let root_id = tree.root_id().expect("affix root should exist");
    let before_ptr = tree
        .get(root_id)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<Affix>()
        .unwrap() as *const Affix;

    ViewAdapter::reconcile_nodes(&mut tree, ViewNode::leaf(Affix::new(40.0)));

    let affix = tree
        .get(root_id)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<Affix>()
        .unwrap();
    assert_eq!(affix as *const Affix, before_ptr);
    assert!(!affix.is_affixed());
    assert_eq!(affix.child_y(), 76.0);
    assert_eq!(
        affix.measure(crate::core::Constraints::loose(Size::new(120.0, 80.0))),
        Size::new(0.0, 36.0)
    );
    assert_eq!(
        affix.snapshot_fields(),
        SnapshotFields::Affix {
            offset_top: 40.0,
            scroll_y: 24.0,
            affixed: false,
        }
    );
}

#[test]
fn reconcile_breadcrumb_patches_instance_and_syncs_config() {
    use crate::ui::widgets::{Breadcrumb, BreadcrumbItem};

    let mut tree = ViewAdapter::build_nodes(ViewNode::leaf(Breadcrumb::new()));
    let root_id = tree.root_id().expect("breadcrumb root should exist");
    let before_ptr = tree
        .get(root_id)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<Breadcrumb>()
        .unwrap() as *const Breadcrumb;
    let items = vec![
        BreadcrumbItem::new("Home"),
        BreadcrumbItem::new("Docs").active(),
    ];

    ViewAdapter::reconcile_nodes(
        &mut tree,
        ViewNode::leaf(Breadcrumb::new().items(items.clone()).separator(">")),
    );

    let breadcrumb = tree
        .get(root_id)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<Breadcrumb>()
        .unwrap();
    assert_eq!(breadcrumb as *const Breadcrumb, before_ptr);
    assert_eq!(
        breadcrumb.snapshot_fields(),
        SnapshotFields::Breadcrumb {
            items,
            separator: ">".to_string(),
        }
    );
}

#[test]
fn reconcile_timeline_patches_instance_and_syncs_config() {
    use crate::ui::widgets::{Timeline, TimelineItem};

    let mut tree = ViewAdapter::build_nodes(ViewNode::leaf(Timeline::new()));
    let root_id = tree.root_id().expect("timeline root should exist");
    let before_ptr = tree
        .get(root_id)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<Timeline>()
        .unwrap() as *const Timeline;
    let items = vec![
        TimelineItem::new("Started").description("alpha"),
        TimelineItem::new("Done").color(Color::green()),
    ];

    ViewAdapter::reconcile_nodes(
        &mut tree,
        ViewNode::leaf(
            Timeline::new()
                .items(items.clone())
                .pending(true)
                .reverse(true),
        ),
    );

    let timeline = tree
        .get(root_id)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<Timeline>()
        .unwrap();
    assert_eq!(timeline as *const Timeline, before_ptr);
    assert_eq!(
        timeline.snapshot_fields(),
        SnapshotFields::Timeline {
            items,
            pending: true,
            reverse: true,
        }
    );
}

#[test]
fn reconcile_message_preserves_queue_and_syncs_placement() {
    use crate::ui::widgets::Message;
    use crate::ui::Placement;

    let message = Message::new();
    message.success("kept");
    let handle = message.handle();
    let mut tree = ViewAdapter::build_nodes(ViewNode::leaf(message));
    let root_id = tree.root_id().expect("message root should exist");
    let before_ptr = tree
        .get(root_id)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<Message>()
        .unwrap() as *const Message;

    ViewAdapter::reconcile_nodes(
        &mut tree,
        ViewNode::leaf(Message::new().placement(Placement::TopRight)),
    );

    let message = tree
        .get(root_id)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<Message>()
        .unwrap();
    assert_eq!(message as *const Message, before_ptr);
    let items = handle.items();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].content, "kept");
    assert_eq!(
        message.snapshot_fields(),
        SnapshotFields::Message {
            placement: Placement::TopRight,
        }
    );
}

#[test]
fn reconcile_notification_preserves_queue_and_syncs_placement() {
    use crate::ui::widgets::Notification;
    use crate::ui::Placement;

    let notification = Notification::new();
    notification.open("kept", "body", StatusLevel::Info);
    let handle = notification.handle();
    let mut tree = ViewAdapter::build_nodes(ViewNode::leaf(notification));
    let root_id = tree.root_id().expect("notification root should exist");
    let before_ptr = tree
        .get(root_id)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<Notification>()
        .unwrap() as *const Notification;

    ViewAdapter::reconcile_nodes(
        &mut tree,
        ViewNode::leaf(Notification::new().placement(Placement::BottomLeft)),
    );

    let notification = tree
        .get(root_id)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<Notification>()
        .unwrap();
    assert_eq!(notification as *const Notification, before_ptr);
    let items = handle.items();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].title, "kept");
    assert_eq!(
        notification.snapshot_fields(),
        SnapshotFields::Notification {
            placement: Placement::BottomLeft,
        }
    );
}

#[test]
fn reconcile_collapse_preserves_expanded_state_and_syncs_config() {
    use crate::ui::widgets::{Collapse, CollapsePanel};

    let mut tree = ViewAdapter::build_nodes(ViewNode::leaf(
        Collapse::new().panels(vec![CollapsePanel::new("Old", "old")]),
    ));
    let root_id = tree.root_id().expect("collapse root should exist");
    let before_ptr = tree
        .get(root_id)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<Collapse>()
        .unwrap() as *const Collapse;

    assert_eq!(
        crate::ui::EventHandler::on_event(
            tree.get_mut(root_id)
                .unwrap()
                .component_mut()
                .as_any_mut()
                .downcast_mut::<Collapse>()
                .unwrap(),
            &SystemEvent::PointerDown {
                pos: Point::new(1.0, 1.0),
                button: MouseButton::Left,
                mods: KeyMod::NONE,
            },
        ),
        EventResult::Handled
    );

    ViewAdapter::reconcile_nodes(
        &mut tree,
        ViewNode::leaf(
            Collapse::new()
                .panels(vec![CollapsePanel::new("New", "new body")])
                .accordion(),
        ),
    );

    let collapse = tree
        .get(root_id)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<Collapse>()
        .unwrap();
    assert_eq!(collapse as *const Collapse, before_ptr);
    assert_eq!(
        collapse.measure(crate::core::Constraints::loose(Size::new(200.0, 200.0))),
        Size::new(0.0, 70.0)
    );
    assert!(matches!(
        collapse.snapshot_fields(),
        SnapshotFields::Collapse {
            panels,
            accordion: true,
            focused_header: 0,
        }
            if panels.len() == 1
                && panels[0].header == "New"
                && panels[0].content == "new body"
                && panels[0].expanded
    ));
}

#[test]
fn reconcile_carousel_patches_instance_and_syncs_config() {
    use crate::ui::widgets::Carousel;

    let mut tree = ViewAdapter::build_nodes(ViewNode::leaf(Carousel::new()));
    let root_id = tree.root_id().expect("carousel root should exist");
    let before_ptr = tree
        .get(root_id)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<Carousel>()
        .unwrap() as *const Carousel;

    ViewAdapter::reconcile_nodes(
        &mut tree,
        ViewNode::leaf(Carousel::new().show_dots(false).show_arrows(false)),
    );

    let carousel = tree
        .get(root_id)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<Carousel>()
        .unwrap();
    assert_eq!(carousel as *const Carousel, before_ptr);
    assert_eq!(carousel.current_index(), 0);
    assert_eq!(
        carousel.snapshot_fields(),
        SnapshotFields::Carousel {
            show_dots: false,
            show_arrows: false,
            fixed_width: None,
            fixed_height: None,
            current: 0,
            slide_count: 0,
        }
    );
}

#[test]
fn reconcile_nonempty_carousel_ignores_runtime_snapshot_fields() {
    use crate::ui::widgets::{Carousel, Label};

    fn view() -> ViewNode {
        ViewNode::new(
            Carousel::new(),
            vec![
                ViewNode::leaf(Label::new("First")),
                ViewNode::leaf(Label::new("Second")),
            ],
        )
    }

    let mut tree = ViewAdapter::build_nodes(view());
    let root_id = tree.root_id().expect("carousel root should exist");
    tree.get_mut(root_id)
        .unwrap()
        .set_frame(Rect::new(0.0, 0.0, 300.0, 200.0));
    tree.layout();
    tree.invalidation().lock().unwrap().clear();

    ViewAdapter::reconcile_nodes(&mut tree, view());

    assert!(
        tree.invalidation().lock().unwrap().is_empty(),
        "equal authored config must not invalidate because slide_count is runtime state"
    );
}

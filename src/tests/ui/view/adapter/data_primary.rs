use super::*;

#[test]
fn reconcile_table_preserves_runtime_selection_and_syncs_config() {
    use crate::ui::widgets::{Table, TableColumn};
    use crate::ui::SnapshotTableColumn;

    let mut tree = ViewAdapter::build_nodes(ViewNode::leaf(
        Table::new()
            .columns(vec![TableColumn::new("Name", 120.0).sortable(true)])
            .rows(vec![
                vec!["Ada".to_string()],
                vec!["Grace".to_string()],
                vec!["Lin".to_string()],
            ])
            .selection(true)
            .page_size(2),
    ));
    let root_id = tree.root_id().expect("table root should exist");
    {
        let table = tree
            .get_mut(root_id)
            .unwrap()
            .component_mut()
            .as_any_mut()
            .downcast_mut::<Table>()
            .unwrap();
        table.set_selected_row(Some(1));
        assert_eq!(
            crate::ui::EventHandler::on_event(
                table,
                &SystemEvent::PointerDown {
                    pos: Point::new(8.0, 34.0),
                    button: MouseButton::Left,
                    mods: KeyMod::NONE,
                },
            ),
            EventResult::Handled
        );
    }
    let before_ptr = tree
        .get(root_id)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<Table>()
        .unwrap() as *const Table;

    ViewAdapter::reconcile_nodes(
        &mut tree,
        crate::ui::view::View::build(
            Table::new()
                .columns(vec![TableColumn::new("City", 96.0).filterable(true)])
                .rows(vec![vec!["Paris".to_string()], vec!["London".to_string()]])
                .row_height(36.0)
                .empty_text("No rows")
                .expandable(72.0, |_row| crate::ui::view::label("Details"))
                .sortable(true)
                .selection(true)
                .bordered(true)
                .virtual_scroll(true)
                .page_size(8),
        ),
    );

    let table = tree
        .get(root_id)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<Table>()
        .unwrap();
    assert_eq!(table as *const Table, before_ptr);
    assert_eq!(table.selected_row(), Some(1));
    assert_eq!(table.checked_rows(), &[0]);
    assert_eq!(
        table.snapshot_fields(),
        SnapshotFields::Table {
            columns: vec![SnapshotTableColumn {
                title: "City".to_string(),
                width: 96.0,
                sortable: false,
                sort_direction: crate::ui::widgets::SortDirection::None,
                filterable: true,
                filters: Vec::new(),
                fixed: None,
            }],
            column_groups: Vec::new(),
            rows: vec![vec!["Paris".to_string()], vec!["London".to_string()]],
            row_h: 36.0,
            header_h: 32.0,
            expandable: true,
            expand_height: 72.0,
            sortable: true,
            selection: true,
            bordered: true,
            selected_row: Some(1),
            checked_rows: vec![0],
            empty_text: "No rows".to_string(),
            page_size: 8,
            virtual_scroll: true,
        }
    );
}

#[test]
fn table_expand_renderer_is_replaced_and_removed_with_component_sidecar() {
    use crate::ui::widgets::Table;

    let first_capture = Rc::new(Cell::new(0));
    let first_renderer = Rc::clone(&first_capture);
    let mut tree = ViewAdapter::build(Table::new().rows(vec![vec!["Ada".to_string()]]).expandable(
        48.0,
        move |_row| {
            let _ = first_renderer.get();
            crate::ui::view::label("First details")
        },
    ));
    let root = tree.root_id().expect("table root");

    assert!(tree.has_table_expand_renderer(root));
    assert_eq!(Rc::strong_count(&first_capture), 2);

    let second_capture = Rc::new(Cell::new(0));
    let second_renderer = Rc::clone(&second_capture);
    ViewAdapter::reconcile(
        &mut tree,
        Table::new()
            .rows(vec![vec!["Grace".to_string()]])
            .expandable(56.0, move |_row| {
                let _ = second_renderer.get();
                crate::ui::view::label("Second details")
            }),
    );

    assert_eq!(tree.root_id(), Some(root), "ComponentId must stay stable");
    assert!(tree.has_table_expand_renderer(root));
    assert_eq!(Rc::strong_count(&first_capture), 1);
    assert_eq!(Rc::strong_count(&second_capture), 2);

    ViewAdapter::reconcile_nodes(&mut tree, ViewNode::leaf(Table::new()));

    assert_eq!(tree.root_id(), Some(root));
    assert!(!tree.has_table_expand_renderer(root));
    assert_eq!(Rc::strong_count(&second_capture), 1);
}

#[test]
fn table_expand_affordance_materializes_and_reconciles_view_child() {
    use crate::ui::view::button;
    use crate::ui::widgets::{Button, Table, TableColumn};

    let mut tree = ViewAdapter::build(
        Table::new()
            .columns(vec![TableColumn::new("Name", 120.0)])
            .rows(vec![vec!["Ada".to_string()]])
            .expandable(48.0, |row| button(row.first().cloned().unwrap_or_default())),
    );
    let root = tree.root_id().expect("table root");
    tree.get_mut(root)
        .expect("table node")
        .set_frame(Rect::new(0.0, 0.0, 120.0, 120.0));
    tree.layout();

    let toggle = SystemEvent::PointerDown {
        pos: Point::new(112.0, 40.0),
        button: MouseButton::Left,
        mods: KeyMod::NONE,
    };
    assert_eq!(tree.dispatch_event(&toggle), EventResult::Handled);
    tree.layout();
    let child = tree
        .get(root)
        .expect("table node")
        .children()
        .first()
        .copied()
        .expect("expanded child");
    assert_eq!(
        tree.get(child)
            .expect("expanded child node")
            .component()
            .as_any()
            .downcast_ref::<Button>()
            .expect("expanded Button")
            .text(),
        "Ada"
    );

    ViewAdapter::reconcile(
        &mut tree,
        Table::new()
            .columns(vec![TableColumn::new("Name", 120.0)])
            .rows(vec![vec!["Grace".to_string()]])
            .expandable(48.0, |row| button(row.first().cloned().unwrap_or_default())),
    );

    let reconciled_child = tree
        .get(root)
        .expect("table node")
        .children()
        .first()
        .copied()
        .expect("reconciled expanded child");
    assert_eq!(reconciled_child, child);
    assert_eq!(
        tree.get(reconciled_child)
            .expect("expanded child node")
            .component()
            .as_any()
            .downcast_ref::<Button>()
            .expect("expanded Button")
            .text(),
        "Grace"
    );
    assert_eq!(tree.dispatch_event(&toggle), EventResult::Handled);
    assert!(tree
        .get_mut(root)
        .expect("table node")
        .children()
        .is_empty());
}

#[test]
fn replacing_tree_root_drops_table_expand_renderer() {
    use crate::ui::widgets::{Label, Table};

    let capture = Rc::new(Cell::new(0));
    let renderer_capture = Rc::clone(&capture);
    let mut tree = ViewAdapter::build(Table::new().expandable(48.0, move |_row| {
        let _ = renderer_capture.get();
        crate::ui::view::label("Details")
    }));
    assert_eq!(Rc::strong_count(&capture), 2);

    ViewAdapter::reconcile_nodes(&mut tree, ViewNode::leaf(Label::new("replacement")));

    assert_eq!(Rc::strong_count(&capture), 1);
}

#[test]
fn reconcile_progress_bar_preserves_animation_phase_and_syncs_config() {
    use crate::ui::widgets::{ProgressBar, ProgressMode, ProgressType};

    let mut tree = ViewAdapter::build_nodes(ViewNode::leaf(ProgressBar::new().indeterminate()));
    let root_id = tree.root_id().expect("progress root should exist");
    {
        let progress = tree
            .get_mut(root_id)
            .unwrap()
            .component_mut()
            .as_any_mut()
            .downcast_mut::<ProgressBar>()
            .unwrap();
        assert!(WidgetAnimation::update_animation(progress, 0.25));
    }
    let before = tree
        .get(root_id)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<ProgressBar>()
        .unwrap()
        .animation_phase();
    assert!(before > 0.0);

    ViewAdapter::reconcile_nodes(
        &mut tree,
        ViewNode::leaf(
            ProgressBar::new()
                .progress(0.6)
                .stroke_color(Color::red())
                .track_color(Color::blue())
                .size(96.0, 12.0)
                .circle(),
        ),
    );

    let progress = tree
        .get(root_id)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<ProgressBar>()
        .unwrap();
    assert_eq!(progress.animation_phase(), before);
    assert_eq!(
        progress.snapshot_fields(),
        SnapshotFields::ProgressBar {
            progress: 0.6,
            mode: ProgressMode::Determinate(0.6),
            stroke_color: Some(Color::red()),
            track_color: Some(Color::blue()),
            height: 12.0,
            width: 96.0,
            round: true,
            progress_type: ProgressType::Circle,
        }
    );
}

#[test]
fn reconcile_spin_preserves_phase_and_syncs_config() {
    use crate::ui::widgets::{Spin, SpinSize};

    let mut tree = ViewAdapter::build_nodes(ViewNode::leaf(Spin::new()));
    let root_id = tree.root_id().expect("spin root should exist");
    {
        let spin = tree
            .get_mut(root_id)
            .unwrap()
            .component_mut()
            .as_any_mut()
            .downcast_mut::<Spin>()
            .unwrap();
        assert!(WidgetAnimation::update_animation(spin, 0.25));
    }
    let before = tree
        .get(root_id)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<Spin>()
        .unwrap()
        .phase();
    assert!(before > 0.0);

    ViewAdapter::reconcile_nodes(
        &mut tree,
        ViewNode::leaf(
            Spin::new()
                .large()
                .color(Color::green())
                .spinning(false)
                .tip("Loading")
                .wrapper_mode(),
        ),
    );

    let spin = tree
        .get(root_id)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<Spin>()
        .unwrap();
    assert_eq!(spin.phase(), before);
    assert_eq!(
        spin.snapshot_fields(),
        SnapshotFields::Spin {
            size: SpinSize::Large,
            color: Some(Color::green()),
            spinning: false,
            tip: "Loading".to_string(),
            wrapper_mode: true,
        }
    );
}

#[test]
fn reconcile_float_button_patches_instance_and_syncs_config() {
    use crate::ui::widgets::FloatButton;

    let mut tree = ViewAdapter::build_nodes(ViewNode::leaf(FloatButton::new("+")));
    let root_id = tree.root_id().expect("float button root should exist");
    let before_ptr = tree
        .get(root_id)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<FloatButton>()
        .unwrap() as *const FloatButton;

    ViewAdapter::reconcile_nodes(
        &mut tree,
        ViewNode::leaf(
            FloatButton::new("up")
                .tooltip("Top")
                .badge(9)
                .size(48.0)
                .position(12.0, 24.0),
        ),
    );

    let float_button = tree
        .get(root_id)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<FloatButton>()
        .unwrap();
    assert_eq!(float_button as *const FloatButton, before_ptr);
    assert_eq!(
        float_button.snapshot_fields(),
        SnapshotFields::FloatButton {
            icon: "up".to_string(),
            tooltip: "Top".to_string(),
            badge_count: 9,
            size: 48.0,
            x: 12.0,
            y: 24.0,
        }
    );
}

#[test]
fn reconcile_back_top_preserves_visibility_and_syncs_threshold() {
    use crate::ui::widgets::BackTop;

    let mut tree =
        ViewAdapter::build_nodes(ViewNode::leaf(BackTop::new().visibility_height(100.0)));
    let root_id = tree.root_id().expect("back top root should exist");
    tree.get_mut(root_id)
        .unwrap()
        .component_mut()
        .as_any_mut()
        .downcast_mut::<BackTop>()
        .unwrap()
        .update_visibility(200.0);

    ViewAdapter::reconcile_nodes(
        &mut tree,
        ViewNode::leaf(BackTop::new().visibility_height(240.0)),
    );

    let back_top = tree
        .get(root_id)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<BackTop>()
        .unwrap();
    assert!(back_top.is_visible());
    assert_eq!(
        back_top.snapshot_fields(),
        SnapshotFields::BackTop {
            visibility_height: 240.0,
            visible: true,
        }
    );
}

#[test]
fn reconcile_alert_patches_instance_and_syncs_config() {
    use crate::ui::widgets::Alert;

    let mut tree = ViewAdapter::build_nodes(ViewNode::leaf(Alert::new("old")));
    let root_id = tree.root_id().expect("alert root should exist");
    let before_ptr = tree
        .get(root_id)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<Alert>()
        .unwrap() as *const Alert;

    ViewAdapter::reconcile_nodes(
        &mut tree,
        ViewNode::leaf(
            Alert::new("new")
                .description("details")
                .type_(StatusLevel::Warning)
                .closable(),
        ),
    );

    let alert = tree
        .get(root_id)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<Alert>()
        .unwrap();
    assert_eq!(alert as *const Alert, before_ptr);
    assert_eq!(
        alert.snapshot_fields(),
        SnapshotFields::Alert {
            message: "new".to_string(),
            description: "details".to_string(),
            type_: StatusLevel::Warning,
            closable: true,
            show_icon: true,
        }
    );
}

#[test]
fn reconcile_tag_patches_instance_and_syncs_config() {
    use crate::ui::widgets::{Tag, TagColor};

    let mut tree = ViewAdapter::build_nodes(ViewNode::leaf(Tag::new("old")));
    let root_id = tree.root_id().expect("tag root should exist");
    let before_ptr = tree
        .get(root_id)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<Tag>()
        .unwrap() as *const Tag;

    ViewAdapter::reconcile_nodes(
        &mut tree,
        ViewNode::leaf(
            Tag::new("new")
                .color(TagColor::Success)
                .custom_color(Color::green())
                .closable()
                .checkable(true)
                .font_size(14.0),
        ),
    );

    let tag = tree
        .get(root_id)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<Tag>()
        .unwrap();
    assert_eq!(tag as *const Tag, before_ptr);
    assert_eq!(
        tag.snapshot_fields(),
        SnapshotFields::Tag {
            text: "new".to_string(),
            color: TagColor::Success,
            closable: true,
            font_size: 14.0,
            custom_color: Some(Color::green()),
            checkable: true,
        }
    );
}

#[test]
fn reconcile_empty_patches_instance_and_syncs_config() {
    use crate::ui::widgets::Empty;

    let mut tree = ViewAdapter::build_nodes(ViewNode::leaf(Empty::new().description("old")));
    let root_id = tree.root_id().expect("empty root should exist");
    let before_ptr = tree
        .get(root_id)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<Empty>()
        .unwrap() as *const Empty;

    ViewAdapter::reconcile_nodes(
        &mut tree,
        ViewNode::leaf(Empty::new().description("new").icon("search").image("file")),
    );

    let empty = tree
        .get(root_id)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<Empty>()
        .unwrap();
    assert_eq!(empty as *const Empty, before_ptr);
    assert_eq!(
        empty.snapshot_fields(),
        SnapshotFields::Empty {
            description: "new".to_string(),
            icon_name: "search".to_string(),
            image: "file".to_string(),
        }
    );
}

#[test]
fn reconcile_skeleton_patches_instance_and_syncs_config() {
    use crate::ui::widgets::{Skeleton, SkeletonShape};

    let mut tree = ViewAdapter::build_nodes(ViewNode::leaf(Skeleton::new()));
    let root_id = tree.root_id().expect("skeleton root should exist");
    let before_ptr = tree
        .get(root_id)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<Skeleton>()
        .unwrap() as *const Skeleton;

    ViewAdapter::reconcile_nodes(
        &mut tree,
        ViewNode::leaf(
            Skeleton::new()
                .shape(SkeletonShape::Circle)
                .size(64.0, 64.0),
        ),
    );

    let skeleton = tree
        .get(root_id)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<Skeleton>()
        .unwrap();
    assert_eq!(skeleton as *const Skeleton, before_ptr);
    assert_eq!(
        skeleton.snapshot_fields(),
        SnapshotFields::Skeleton {
            shape: SkeletonShape::Circle,
            width: 64.0,
            height: 64.0,
        }
    );
}

#[test]
fn reconcile_card_patches_instance_and_syncs_config() {
    use crate::ui::widgets::Card;

    let mut tree = ViewAdapter::build_nodes(ViewNode::leaf(Card::new().title("old")));
    let root_id = tree.root_id().expect("card root should exist");
    let before_ptr = tree
        .get(root_id)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<Card>()
        .unwrap() as *const Card;

    ViewAdapter::reconcile_nodes(
        &mut tree,
        ViewNode::leaf(
            Card::new()
                .title("new")
                .bordered(false)
                .hoverable()
                .size(240.0, 120.0)
                .padding(20.0)
                .elevation(3)
                .flex_grow(1.5)
                .actions(vec!["Edit", "Delete"]),
        ),
    );

    let card = tree
        .get(root_id)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<Card>()
        .unwrap();
    assert_eq!(card as *const Card, before_ptr);
    assert_eq!(
        card.snapshot_fields(),
        SnapshotFields::Card {
            title: Some("new".to_string()),
            bordered: false,
            hoverable: true,
            fixed_width: Some(240.0),
            fixed_height: Some(120.0),
            padding: 20.0,
            elevation: 3,
            flex_grow: 1.5,
            actions: vec!["Edit".to_string(), "Delete".to_string()],
        }
    );
}

#[test]
fn reconcile_image_patches_instance_and_syncs_config() {
    use crate::ui::widgets::Image;

    let mut tree = ViewAdapter::build_nodes(ViewNode::leaf(Image::new(40.0, 20.0).src("old.png")));
    let root_id = tree.root_id().expect("image root should exist");
    let before_ptr = tree
        .get(root_id)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<Image>()
        .unwrap() as *const Image;

    ViewAdapter::reconcile_nodes(
        &mut tree,
        ViewNode::leaf(
            Image::new(80.0, 60.0)
                .src("new.png")
                .alt("Preview")
                .fallback("fallback")
                .radius(8.0)
                .preview(false)
                .fit(false),
        ),
    );

    let image = tree
        .get(root_id)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<Image>()
        .unwrap();
    assert_eq!(image as *const Image, before_ptr);
    assert_eq!(
        image.snapshot_fields(),
        SnapshotFields::Image {
            src: "new.png".to_string(),
            alt: "Preview".to_string(),
            fallback: "fallback".to_string(),
            width: 80.0,
            height: 60.0,
            radius: 8.0,
            preview: false,
            fit: false,
        }
    );
}

#[test]
fn reconcile_calendar_preserves_selection_and_syncs_config() {
    use crate::ui::widgets::Calendar;

    let mut tree = ViewAdapter::build_nodes(ViewNode::leaf(Calendar::new()));
    let root_id = tree.root_id().expect("calendar root should exist");
    {
        let calendar = tree
            .get_mut(root_id)
            .unwrap()
            .component_mut()
            .as_any_mut()
            .downcast_mut::<Calendar>()
            .unwrap();
        assert_eq!(
            crate::ui::EventHandler::on_event(
                calendar,
                &SystemEvent::PointerDown {
                    pos: Point::new(5.0, 85.0),
                    button: MouseButton::Left,
                    mods: KeyMod::NONE,
                },
            ),
            EventResult::Handled
        );
    }
    let selected_before = tree
        .get(root_id)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<Calendar>()
        .unwrap()
        .selected_day();
    assert!(selected_before.is_some());

    ViewAdapter::reconcile_nodes(
        &mut tree,
        ViewNode::leaf(Calendar::new().cell_size(32.0).year_jump(true)),
    );

    let calendar = tree
        .get(root_id)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<Calendar>()
        .unwrap();
    assert_eq!(calendar.selected_day(), selected_before);
    assert!(matches!(
        calendar.snapshot_fields(),
        SnapshotFields::Calendar {
            cell_size: 32.0,
            year_jump: true,
            year: 2026,
            month: 6,
            selected: Some(date),
            focused_day,
        } if Some(date.day) == selected_before && date.day == focused_day
    ));
}

#[test]
fn reconcile_descriptions_patches_instance_and_syncs_config() {
    use crate::ui::widgets::{Descriptions, DescriptionsItem};

    let mut tree = ViewAdapter::build_nodes(ViewNode::leaf(Descriptions::new().title("old")));
    let root_id = tree.root_id().expect("descriptions root should exist");
    let before_ptr = tree
        .get(root_id)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<Descriptions>()
        .unwrap() as *const Descriptions;
    let items = vec![DescriptionsItem::new("Name", "Ada").span(2)];

    ViewAdapter::reconcile_nodes(
        &mut tree,
        ViewNode::leaf(
            Descriptions::new()
                .title("Profile")
                .items(items.clone())
                .bordered(true)
                .column(2)
                .label_width(88.0)
                .size(ControlSize::Small),
        ),
    );

    let descriptions = tree
        .get(root_id)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<Descriptions>()
        .unwrap();
    assert_eq!(descriptions as *const Descriptions, before_ptr);
    assert_eq!(
        descriptions.snapshot_fields(),
        SnapshotFields::Descriptions {
            title: "Profile".to_string(),
            items,
            bordered: true,
            column: 2,
            label_width: 88.0,
            descriptions_size: ControlSize::Small,
        }
    );
}

#[test]
fn reconcile_result_patches_instance_and_syncs_config() {
    use crate::ui::widgets::{ResultType, ResultView};

    let mut tree = ViewAdapter::build_nodes(ViewNode::leaf(ResultView::new(ResultType::Info)));
    let root_id = tree.root_id().expect("result root should exist");
    let before_ptr = tree
        .get(root_id)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<ResultView>()
        .unwrap() as *const ResultView;

    ViewAdapter::reconcile_nodes(
        &mut tree,
        ViewNode::leaf(
            ResultView::new(ResultType::Success)
                .title("Done")
                .subtitle("All set")
                .extra_text("Continue"),
        ),
    );

    let result = tree
        .get(root_id)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<ResultView>()
        .unwrap();
    assert_eq!(result as *const ResultView, before_ptr);
    assert_eq!(
        result.snapshot_fields(),
        SnapshotFields::Result {
            result_type: ResultType::Success,
            title: "Done".to_string(),
            subtitle: "All set".to_string(),
            extra_text: "Continue".to_string(),
        }
    );
}

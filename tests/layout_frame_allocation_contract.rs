//! 验证真实嵌套组件树在预热后的重复布局中复用布局工作区。

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use uix::core::{Rect, Size};
use uix::prelude::{
    Button, Calendar, Card, Collapse, CollapsePanel, Container, Content, Dropdown, DropdownItem,
    FontFamily, Footer, Form, FormItem, Grid, GridTrack, Header, Input, Layout as PageLayout, Menu,
    MenuItem, MenuMode, ScrollDirection, ScrollView, Select, Sider, Space, Splitter, Style, Table,
    TableColumn, Tabs, Transfer, TransferItem, VirtualScroll,
};
use uix::ui::__private::WidgetTree;
use uix::ui::{
    EventHandler, IntoWidgetNode, LayoutChild, LayoutEngineScratch, SystemEvent, View, WidgetId,
    WidgetLayout,
};

struct CountingAllocator;

static COUNT_ALLOCATIONS: AtomicBool = AtomicBool::new(false);
static ALLOCATION_COUNT: AtomicUsize = AtomicUsize::new(0);

unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if COUNT_ALLOCATIONS.load(Ordering::Relaxed) {
            ALLOCATION_COUNT.fetch_add(1, Ordering::Relaxed);
        }
        // SAFETY: 原样把有效 Layout 委托给系统分配器。
        unsafe { System.alloc(layout) }
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        if COUNT_ALLOCATIONS.load(Ordering::Relaxed) {
            ALLOCATION_COUNT.fetch_add(1, Ordering::Relaxed);
        }
        // SAFETY: 原样把有效 Layout 委托给系统分配器。
        unsafe { System.alloc_zeroed(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        // SAFETY: 指针与 Layout 来自同一系统分配器。
        unsafe { System.dealloc(ptr, layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        if COUNT_ALLOCATIONS.load(Ordering::Relaxed) {
            ALLOCATION_COUNT.fetch_add(1, Ordering::Relaxed);
        }
        // SAFETY: 指针与旧 Layout 来自系统分配器，新尺寸由调用方提供。
        unsafe { System.realloc(ptr, layout, new_size) }
    }
}

#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

fn nested_layout_tree() -> (WidgetTree, uix::ui::WidgetId) {
    let mut tree = WidgetTree::new();
    let root = tree.set_root(Box::new(Container::new().size(320.0, 200.0)));
    for branch_index in 0..3 {
        let branch = tree.add_child(root, Box::new(Container::new()));
        for leaf_index in 0..4 {
            tree.add_child(
                branch,
                Box::new(
                    Space::new()
                        .width(20.0 + leaf_index as f32)
                        .height(12.0 + branch_index as f32),
                ),
            );
        }
    }
    (tree, root)
}

fn wrapped_container_tree() -> (WidgetTree, uix::ui::WidgetId) {
    let mut tree = WidgetTree::new();
    let root = tree.set_root(Box::new(Container::new().size(320.0, 200.0)));
    for branch_index in 0..3 {
        let mut branch = Container::new().size(100.0, 30.0);
        branch.style.flex_wrap = true;
        let branch = tree.add_child(root, Box::new(branch));
        for leaf_index in 0..5 {
            tree.add_child(
                branch,
                Box::new(
                    Space::new()
                        .width(18.0 + branch_index as f32)
                        .height(12.0 + leaf_index as f32),
                ),
            );
        }
    }
    (tree, root)
}

fn wrapped_space_tree() -> (WidgetTree, uix::ui::WidgetId) {
    let mut tree = WidgetTree::new();
    let root = tree.set_root(Box::new(Container::new().size(320.0, 200.0)));
    for branch_index in 0..3 {
        let branch = tree.add_child(
            root,
            Box::new(Space::new().width(100.0).height(30.0).wrap(true)),
        );
        for leaf_index in 0..5 {
            tree.add_child(
                branch,
                Box::new(
                    Space::new()
                        .width(18.0 + branch_index as f32)
                        .height(12.0 + leaf_index as f32),
                ),
            );
        }
    }
    (tree, root)
}

fn card_layout_tree() -> (WidgetTree, uix::ui::WidgetId) {
    let mut tree = WidgetTree::new();
    let root = tree.set_root(Box::new(Container::new().size(320.0, 200.0)));
    for branch_index in 0..3 {
        let branch = tree.add_child(root, Box::new(Card::new().size(100.0, 60.0)));
        for leaf_index in 0..4 {
            tree.add_child(
                branch,
                Box::new(
                    Space::new()
                        .width(18.0 + branch_index as f32)
                        .height(10.0 + leaf_index as f32),
                ),
            );
        }
    }
    (tree, root)
}

fn grid_layout_tree() -> (WidgetTree, uix::ui::WidgetId) {
    let mut tree = WidgetTree::new();
    let root = tree.set_root(Box::new(Container::new().size(320.0, 200.0)));
    for branch_index in 0..3 {
        let branch = tree.add_child(
            root,
            Box::new(
                Grid::new()
                    .size(100.0, 60.0)
                    .columns(vec![GridTrack::Auto, GridTrack::Fr(1.0)])
                    .rows(vec![GridTrack::Auto, GridTrack::Fr(1.0)]),
            ),
        );
        for leaf_index in 0..4 {
            tree.add_child(
                branch,
                Box::new(
                    Space::new()
                        .width(18.0 + branch_index as f32)
                        .height(10.0 + leaf_index as f32),
                ),
            );
        }
    }
    (tree, root)
}

fn responsive_grid_layout_tree() -> (WidgetTree, uix::ui::WidgetId) {
    let mut tree = WidgetTree::new();
    let root = tree.set_root(Box::new(Container::new().size(320.0, 200.0)));
    for branch_index in 0..1 {
        let branch = tree.add_child(root, Box::new(Grid::responsive().size(300.0, 180.0)));
        for leaf_index in 0..40 {
            tree.add_child(
                branch,
                Box::new(
                    Space::new()
                        .width(18.0 + branch_index as f32)
                        .height(10.0 + leaf_index as f32),
                ),
            );
        }
    }
    (tree, root)
}

fn spanning_grid_layout_tree() -> (WidgetTree, uix::ui::WidgetId) {
    let mut tree = WidgetTree::new();
    let root = tree.set_root(Box::new(Container::new().size(320.0, 200.0)));
    let branch = tree.add_child(
        root,
        Box::new(
            Grid::new()
                .size(300.0, 180.0)
                .columns(vec![GridTrack::Auto; 8]),
        ),
    );
    for index in 0..40 {
        let mut leaf = Container::new().size(18.0 + index as f32, 10.0);
        leaf.style.grid_column_span = 2 + (index % 3) as u32;
        tree.add_child(branch, Box::new(leaf));
    }
    (tree, root)
}

fn scroll_view_layout_tree() -> (WidgetTree, uix::ui::WidgetId) {
    let mut tree = WidgetTree::new();
    let root = tree.set_root(Box::new(Container::new().size(320.0, 200.0)));
    for branch_index in 0..3 {
        let branch = tree.add_child(
            root,
            Box::new(ScrollView::new(ScrollDirection::Vertical).size(100.0, 60.0)),
        );
        for leaf_index in 0..8 {
            tree.add_child(
                branch,
                Box::new(
                    Space::new()
                        .width(80.0 + branch_index as f32)
                        .height(12.0 + leaf_index as f32),
                ),
            );
        }
    }
    (tree, root)
}

fn page_layout_tree() -> (WidgetTree, uix::ui::WidgetId) {
    let mut tree = WidgetTree::new();
    let root = tree.set_root(Box::new(Container::new().size(320.0, 200.0)));
    for branch_index in 0..3 {
        let branch = tree.add_child(root, Box::new(PageLayout::new()));
        for leaf_index in 0..8 {
            tree.add_child(
                branch,
                Box::new(
                    Space::new()
                        .width(80.0 + branch_index as f32)
                        .height(16.0 + leaf_index as f32 * 2.0),
                ),
            );
        }
    }
    (tree, root)
}

fn page_region_layout_tree() -> (WidgetTree, uix::ui::WidgetId) {
    let mut tree = WidgetTree::new();
    let root = tree.set_root(Box::new(Container::new().size(320.0, 200.0)));
    let regions = [
        tree.add_child(root, Box::new(Header::new(48.0))),
        tree.add_child(root, Box::new(Content::new())),
        tree.add_child(root, Box::new(Sider::new(120.0))),
        tree.add_child(root, Box::new(Footer::new(40.0))),
    ];
    for (region_index, region) in regions.into_iter().enumerate() {
        for leaf_index in 0..8 {
            tree.add_child(
                region,
                Box::new(
                    Space::new()
                        .width(72.0 + region_index as f32)
                        .height(12.0 + leaf_index as f32),
                ),
            );
        }
    }
    (tree, root)
}

fn splitter_layout_tree() -> (WidgetTree, uix::ui::WidgetId) {
    let mut tree = WidgetTree::new();
    let root = tree.set_root(Box::new(Container::new().size(320.0, 200.0)));
    for branch_index in 0..3 {
        let branch = tree.add_child(root, Box::new(Splitter::new().panels(8)));
        for leaf_index in 0..8 {
            tree.add_child(
                branch,
                Box::new(
                    Space::new()
                        .width(24.0 + branch_index as f32)
                        .height(20.0 + leaf_index as f32),
                ),
            );
        }
    }
    (tree, root)
}

fn collapse_layout_tree() -> (WidgetTree, uix::ui::WidgetId) {
    let mut tree = WidgetTree::new();
    let root = tree.set_root(Box::new(Container::new().size(320.0, 200.0)));
    let branches = (0..3)
        .map(|branch_index| {
            let panels = (0..8)
                .map(|panel_index| {
                    CollapsePanel::new(
                        format!("分组 {branch_index}-{panel_index}"),
                        format!("第 {panel_index} 组的稳定内容"),
                    )
                    .expanded()
                })
                .collect();
            View::build(Collapse::new().panels(panels)).into_node()
        })
        .collect();
    tree.set_children(root, branches);
    (tree, root)
}

fn table_layout_tree() -> (WidgetTree, uix::ui::WidgetId) {
    let mut tree = WidgetTree::new();
    let root = tree.set_root(Box::new(Container::new().size(320.0, 200.0)));
    let rows = (0..12)
        .map(|index| (index, format!("名称 {index}"), format!("状态 {index}")))
        .collect();
    let table = Table::data(rows, |row| row.0.to_string())
        .expect("测试行键必须唯一")
        .columns(vec![
            TableColumn::new("名称", 160.0)
                .bind(|row: &(usize, String, String)| row.1.clone())
                .render(|_| Container::new()),
            TableColumn::new("状态", 160.0)
                .bind(|row: &(usize, String, String)| row.2.clone())
                .render(|_| Container::new()),
        ])
        .into_node();
    tree.set_children(root, vec![table]);
    (tree, root)
}

fn form_layout_tree() -> (WidgetTree, uix::ui::WidgetId) {
    let mut tree = WidgetTree::new();
    let root = tree.set_root(Box::new(Container::new().size(320.0, 400.0)));
    let items = (0..8)
        .map(|index| {
            let mut item = FormItem::new(&format!("字段 {index}")).into_node();
            item.children = vec![Container::new().into_node()];
            item
        })
        .collect();
    let mut form = Form::new().into_node();
    form.children = items;
    tree.set_children(root, vec![form]);
    (tree, root)
}

fn tabs_layout_tree() -> (WidgetTree, uix::ui::WidgetId) {
    let mut tree = WidgetTree::new();
    let root = tree.set_root(Box::new(Container::new().size(320.0, 200.0)));
    let mut tabs = Tabs::new()
        .tab("概览", "overview")
        .tab("详情", "details")
        .tab("记录", "history")
        .into_node();
    tabs.children = (0..3).map(|_| Container::new().into_node()).collect();
    tree.set_children(root, vec![tabs]);
    (tree, root)
}

fn virtual_scroll_layout_tree() -> (WidgetTree, uix::ui::WidgetId) {
    let mut tree = WidgetTree::new();
    let root = tree.set_root(Box::new(Container::new().size(320.0, 200.0)));
    let mut scroll = VirtualScroll::new()
        .item_count(10_000)
        .item_height(24.0)
        .overscan(8)
        .size(300.0, 180.0)
        .into_node();
    scroll.children = (0..32).map(|_| Container::new().into_node()).collect();
    tree.set_children(root, vec![scroll]);
    (tree, root)
}

fn calendar_layout_tree() -> (WidgetTree, uix::ui::WidgetId) {
    let mut tree = WidgetTree::new();
    let root = tree.set_root(Box::new(Container::new().size(320.0, 320.0)));
    let calendar = Calendar::new()
        .date_cell(|_, _| Container::new())
        .into_node();
    tree.set_children(root, vec![calendar]);
    (tree, root)
}

fn transfer_layout_tree() -> (WidgetTree, uix::ui::WidgetId) {
    let mut tree = WidgetTree::new();
    let root = tree.set_root(Box::new(Container::new().size(320.0, 320.0)));
    let source = (0..64)
        .map(|index| TransferItem::new(format!("source-{index}"), format!("来源 {index}")))
        .collect();
    let target = (0..64)
        .map(|index| TransferItem::new(format!("target-{index}"), format!("目标 {index}")))
        .collect();
    let mut transfer = Transfer::new()
        .source(source)
        .target(target)
        .searchable(true)
        .render_item(|_| Container::new());
    assert_eq!(
        transfer.on_event(&SystemEvent::TextInput {
            text: "SOURCE-1".to_owned(),
        }),
        uix::ui::EventResult::Handled
    );
    let transfer = transfer.into_node();
    tree.set_children(root, vec![transfer]);
    (tree, root)
}

fn select_layout_tree() -> (WidgetTree, uix::ui::WidgetId) {
    let mut tree = WidgetTree::new();
    let root = tree.set_root(Box::new(Container::new().size(320.0, 320.0)));
    let options = (0..128)
        .map(|index| format!("Option {index}"))
        .collect::<Vec<_>>();
    let mut select = Select::searchable().options(options);
    select.open();
    assert_eq!(
        select.on_event(&SystemEvent::TextInput {
            text: "OPTION 1".to_owned(),
        }),
        uix::ui::EventResult::Handled
    );
    let select = select.render_option(|_| Container::new()).into_node();
    tree.set_children(root, vec![select]);
    (tree, root)
}

fn plain_select_layout_tree() -> (WidgetTree, uix::ui::WidgetId) {
    let mut tree = WidgetTree::new();
    let root = tree.set_root(Box::new(Container::new().size(320.0, 320.0)));
    let options = (0..128)
        .map(|index| format!("Option {index}"))
        .collect::<Vec<_>>();
    let mut select = Select::searchable().options(options);
    select.open();
    assert_eq!(
        select.on_event(&SystemEvent::TextInput {
            text: "OPTION 1".to_owned(),
        }),
        uix::ui::EventResult::Handled
    );
    tree.set_children(root, vec![select.into_node()]);
    (tree, root)
}

fn closed_select_layout_tree() -> (WidgetTree, uix::ui::WidgetId) {
    let mut tree = WidgetTree::new();
    let root = tree.set_root(Box::new(Container::new().size(320.0, 320.0)));
    let options = (0..128)
        .map(|index| format!("Option {index}"))
        .collect::<Vec<_>>();
    tree.set_children(root, vec![Select::new().options(options).into_node()]);
    (tree, root)
}

fn dropdown_layout_tree() -> (WidgetTree, uix::ui::WidgetId) {
    let mut tree = WidgetTree::new();
    let root = tree.set_root(Box::new(Container::new().size(320.0, 320.0)));
    let items = (0..128)
        .map(|index| DropdownItem::from_text(format!("Item {index}"), format!("item-{index}")))
        .collect::<Vec<_>>();
    let mut dropdown = Dropdown::new("Menu").keyed_items(items);
    dropdown.open();
    tree.set_children(root, vec![dropdown.into_node()]);
    (tree, root)
}

fn custom_trigger_dropdown_layout_tree() -> (WidgetTree, uix::ui::WidgetId) {
    let mut tree = WidgetTree::new();
    let root = tree.set_root(Box::new(Container::new().size(320.0, 320.0)));
    let items = (0..128)
        .map(|index| DropdownItem::from_text(format!("Item {index}"), format!("item-{index}")))
        .collect::<Vec<_>>();
    let mut dropdown = Dropdown::new("Menu")
        .keyed_items(items)
        .trigger_view(Container::new());
    dropdown.open();
    tree.set_children(root, vec![dropdown.into_node()]);
    (tree, root)
}

fn menu_layout_tree() -> (WidgetTree, uix::ui::WidgetId) {
    let mut tree = WidgetTree::new();
    let root = tree.set_root(Box::new(Container::new().size(320.0, 320.0)));
    let groups = (0..16)
        .map(|group| {
            let children = (0..8)
                .map(|item| {
                    MenuItem::from_text(
                        format!("Item {group}-{item}"),
                        format!("item-{group}-{item}"),
                    )
                })
                .collect::<Vec<_>>();
            MenuItem::from_text(format!("Group {group}"), format!("group-{group}"))
                .children(children)
        })
        .collect::<Vec<_>>();
    let menu = Menu::new().mode(MenuMode::Inline).items(groups);
    tree.set_children(root, vec![menu.into_node()]);
    (tree, root)
}

fn button_layout_tree() -> (WidgetTree, uix::ui::WidgetId) {
    let mut tree = WidgetTree::new();
    let root = tree.set_root(Box::new(Container::new().size(320.0, 320.0)));
    for index in 0..128 {
        tree.add_child(root, Box::new(Button::new(format!("Button {index}"))));
    }
    (tree, root)
}

fn styled_button_layout_tree() -> (WidgetTree, uix::ui::WidgetId) {
    let mut tree = WidgetTree::new();
    let root = tree.set_root(Box::new(Container::new().size(320.0, 320.0)));
    let style = Style {
        font_family: FontFamily::from_names(["UI Sans", "sans-serif"]),
        grid_template_columns: vec![GridTrack::Auto, GridTrack::Fr(1.0)],
        ..Style::default()
    };
    for index in 0..128 {
        tree.add_child(
            root,
            Box::new(Button::new(format!("Styled {index}")).style(style.clone())),
        );
    }
    (tree, root)
}

fn textarea_layout_tree() -> (WidgetTree, uix::ui::WidgetId) {
    let mut tree = WidgetTree::new();
    let root = tree.set_root(Box::new(Container::new().size(320.0, 320.0)));
    let text = (0..32)
        .map(|line| format!("Line {line}"))
        .collect::<Vec<_>>()
        .join("\n");
    for index in 0..128 {
        tree.add_child(
            root,
            Box::new(
                Input::textarea()
                    .rows(4)
                    .with_value(format!("Textarea {index}\n{text}")),
            ),
        );
    }
    (tree, root)
}

fn warmed_layout_allocations(mut tree: WidgetTree, root: uix::ui::WidgetId) -> usize {
    tree.set_frame_dirty(root, Rect::new(0.0, 0.0, 320.0, 200.0));
    tree.layout();
    tree.set_frame_dirty(root, Rect::new(0.0, 0.0, 321.0, 200.0));
    tree.layout();

    tree.set_frame_dirty(root, Rect::new(0.0, 0.0, 320.0, 200.0));
    ALLOCATION_COUNT.store(0, Ordering::Relaxed);
    COUNT_ALLOCATIONS.store(true, Ordering::Release);
    tree.layout();
    COUNT_ALLOCATIONS.store(false, Ordering::Release);
    ALLOCATION_COUNT.load(Ordering::Relaxed)
}

#[test]
fn page_layout_reusing_path_matches_owned_geometry() {
    let layout = PageLayout::new();
    let header = LayoutChild::new(WidgetId::new(1), Size::new(0.0, 48.0));
    let mut content = LayoutChild::new(WidgetId::new(2), Size::zero());
    content.flex_grow = 1.0;
    let footer = LayoutChild::new(WidgetId::new(3), Size::new(0.0, 48.0));
    let children = [header, content, footer];
    let frame = Rect::new(0.0, 0.0, 400.0, 300.0);
    let tree = WidgetTree::new();
    let expected = layout.layout_children(frame, &children, &tree);
    let mut scratch = LayoutEngineScratch::default();
    let mut actual = Vec::new();

    layout.layout_children_into(frame, &children, &tree, &mut scratch, &mut actual);

    assert_eq!(actual, expected);
    assert_eq!(actual[0].1, Rect::new(0.0, 0.0, 400.0, 48.0));
    assert_eq!(actual[1].1, Rect::new(0.0, 48.0, 400.0, 204.0));
    assert_eq!(actual[2].1, Rect::new(0.0, 252.0, 400.0, 48.0));
}

#[test]
fn splitter_reusing_path_matches_owned_geometry() {
    // 同一组三面板输入分别经过兼容入口和树级复用入口。
    let splitter = Splitter::new().panels(3);
    let frame = Rect::new(10.0, 20.0, 300.0, 120.0);
    let tree = WidgetTree::new();
    let child_ids = [WidgetId::new(1), WidgetId::new(2), WidgetId::new(3)];
    let expected_measured = splitter.measure_children(frame, &child_ids, &tree);
    let mut actual_measured = Vec::new();
    splitter.measure_children_into(frame, &child_ids, &tree, &mut actual_measured);
    assert_eq!(actual_measured.len(), expected_measured.len());
    for (actual, expected) in actual_measured.iter().zip(&expected_measured) {
        assert_eq!(actual.id, expected.id);
        assert_eq!(actual.measured_size, expected.measured_size);
    }

    let expected = splitter.layout_children(frame, &expected_measured, &tree);
    let mut scratch = LayoutEngineScratch::default();
    let mut actual = Vec::new();
    splitter.layout_children_into(frame, &actual_measured, &tree, &mut scratch, &mut actual);

    assert_eq!(actual, expected);
    assert_eq!(actual.len(), 3);
    assert_eq!(actual[0].1.x, frame.x);
    // 比例浮点累加后，最后一个面板仍应覆盖到父区域右边缘。
    assert!((actual[2].1.x + actual[2].1.w - (frame.x + frame.w)).abs() < 0.0001);
}

#[test]
fn warmed_nested_layout_reuses_heap_storage() {
    let scenarios = [
        ("Container", nested_layout_tree()),
        ("Container wrap", wrapped_container_tree()),
        ("Space wrap", wrapped_space_tree()),
        ("Card", card_layout_tree()),
        ("Grid", grid_layout_tree()),
        ("Responsive Grid", responsive_grid_layout_tree()),
        ("Spanning Grid", spanning_grid_layout_tree()),
        ("ScrollView", scroll_view_layout_tree()),
        ("Layout regions", page_region_layout_tree()),
        ("Layout", page_layout_tree()),
        ("Splitter", splitter_layout_tree()),
        ("Collapse", collapse_layout_tree()),
        ("Table", table_layout_tree()),
        ("Form", form_layout_tree()),
        ("Tabs", tabs_layout_tree()),
        ("VirtualScroll", virtual_scroll_layout_tree()),
        ("Calendar", calendar_layout_tree()),
        ("Transfer", transfer_layout_tree()),
        ("Closed Select", closed_select_layout_tree()),
        ("Plain Select", plain_select_layout_tree()),
        ("Select", select_layout_tree()),
        ("Dropdown", dropdown_layout_tree()),
        ("Dropdown trigger", custom_trigger_dropdown_layout_tree()),
        ("Menu", menu_layout_tree()),
        ("Button", button_layout_tree()),
        ("Styled Button", styled_button_layout_tree()),
        ("Textarea", textarea_layout_tree()),
    ];
    for (name, (tree, root)) in scenarios {
        let allocations = warmed_layout_allocations(tree, root);
        eprintln!("稳态 {name} 布局堆申请次数: {allocations}");
        assert_eq!(allocations, 0, "预热后的 {name} 布局必须复用全部堆存储");
    }
}

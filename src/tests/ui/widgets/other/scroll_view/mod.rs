use super::*;
    use crate::core::{Point, Rect, Size};
    use crate::draw::painting::PaintContext;
    use crate::ui::layout::{AlignItems, FlexDirection};
    use crate::ui::traits::{EventHandler, WidgetCapabilities, WidgetLayout, WidgetRender};
    use crate::ui::widgets::{Collapse, CollapsePanel, Container, Space};
    use crate::ui::EventResult;

    struct FixedWidget {
        size: Size,
        #[allow(dead_code)]
        id: WidgetId,
    }

    impl WidgetComponent for FixedWidget {
        fn as_any(&self) -> &dyn std::any::Any {
            self
        }

        fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
            self
        }

        fn into_any(self: Box<Self>) -> Box<dyn std::any::Any> {
            self
        }

        fn capabilities(&self) -> WidgetCapabilities {
            WidgetCapabilities::from_bits(WidgetCapabilities::LAYOUT | WidgetCapabilities::RENDER)
        }

        crate::wc_upcast!(FixedWidget; WidgetLayout);
        crate::wc_upcast!(FixedWidget; WidgetRender);
    }

    impl WidgetLayout for FixedWidget {
        fn preferred_size(
            &self,
            _engine: Option<&dyn crate::draw::traits::GraphicsEngine>,
        ) -> Size {
            self.size
        }
    }

    impl WidgetRender for FixedWidget {
        fn render(&self, _frame: Rect, _ctx: &mut PaintContext, _tree: &WidgetTree) {}
    }

    #[test]
    fn scrollview_scroll_to() {
        let sv = ScrollView::new(ScrollDirection::Vertical).scroll_to(0.0, 100.0);
        assert_eq!(sv.scroll_x, 0.0);
        assert_eq!(sv.scroll_y, 100.0);
    }

    #[test]
    fn scrollview_scroll_to_clamped() {
        let sv = ScrollView::new(ScrollDirection::Vertical).scroll_to(-10.0, -50.0);
        assert_eq!(sv.scroll_x, 0.0);
        assert_eq!(sv.scroll_y, 0.0);
    }

    #[test]
    fn scrollview_set_scroll_programmatically() {
        let mut sv = ScrollView::new(ScrollDirection::Both);
        sv.set_scroll_x(50.0);
        sv.set_scroll_y(75.0);
        assert_eq!(sv.scroll_x, 50.0);
        assert_eq!(sv.scroll_y, 75.0);
    }

    #[test]
    fn scrollview_child_builder() {
        let sv = ScrollView::new(ScrollDirection::Vertical).child(FixedWidget {
            size: Size::new(100.0, 200.0),
            id: 0,
        });
        assert!(sv.children.is_set());
        assert_eq!(sv.children.len(), 1);
        let children = sv.children.take();
        assert_eq!(children.len(), 1);
    }

    #[test]
    fn scrollview_layout_children_uses_natural_coordinates() {
        let scrollview = ScrollView::new(ScrollDirection::Vertical)
            .size(200.0, 300.0)
            .scroll_to(0.0, 50.0);

        let mut tree = WidgetTree::new();
        let root_id = tree.set_root(Box::new(scrollview));
        tree.add_child(
            root_id,
            Box::new(FixedWidget {
                size: Size::new(200.0, 600.0),
                id: 1,
            }),
        );

        tree.layout();

        let frame = tree.get(root_id).map(|n| n.frame()).unwrap_or_default();
        let children = tree
            .get(root_id)
            .map(|n| n.children().to_vec())
            .unwrap_or_default();
        let result = tree
            .get(root_id)
            .unwrap()
            .layout_children(frame, &children, &tree);

        let (_, rect) = result.first().expect("expected child rect");
        assert_eq!(rect.x, frame.x);
        assert_eq!(rect.y, frame.y);
    }

    #[test]
    fn scrollview_direction_flags() {
        assert!(ScrollDirection::Vertical.can_scroll_y());
        assert!(!ScrollDirection::Vertical.can_scroll_x());
        assert!(ScrollDirection::Horizontal.can_scroll_x());
        assert!(!ScrollDirection::Horizontal.can_scroll_y());
        assert!(ScrollDirection::Both.can_scroll_x());
        assert!(ScrollDirection::Both.can_scroll_y());
    }

    #[test]
    fn scrollview_not_handled_for_non_scrollbar_pointer_down() {
        let mut sv = ScrollView::new(ScrollDirection::Vertical);
        let result = sv.on_event(&SystemEvent::PointerDown {
            pos: Point::new(10.0, 10.0),
            button: crate::ui::MouseButton::Left,
            mods: crate::native::traits::input::KeyMod::NONE,
        });
        assert_eq!(result, EventResult::NotHandled);
    }

    #[test]
    fn scrollview_expand_child_updates_content_bounds() {
        struct GrowWidget {
            size: std::cell::Cell<f32>,
        }

        impl WidgetComponent for GrowWidget {
            fn as_any(&self) -> &dyn std::any::Any {
                self
            }

            fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
                self
            }

            fn into_any(self: Box<Self>) -> Box<dyn std::any::Any> {
                self
            }

            fn capabilities(&self) -> WidgetCapabilities {
                WidgetCapabilities::from_bits(
                    WidgetCapabilities::LAYOUT
                        | WidgetCapabilities::RENDER
                        | WidgetCapabilities::EVENT,
                )
            }

            crate::wc_upcast!(GrowWidget; WidgetLayout);
            crate::wc_upcast!(GrowWidget; WidgetRender);
            crate::wc_upcast!(GrowWidget; EventHandler);
        }

        impl WidgetLayout for GrowWidget {
            fn preferred_size(&self, _: Option<&dyn crate::draw::traits::GraphicsEngine>) -> Size {
                Size::new(300.0, self.size.get())
            }
        }

        impl WidgetRender for GrowWidget {
            fn render(&self, _: Rect, _: &mut PaintContext, _: &WidgetTree) {}
        }

        impl EventHandler for GrowWidget {
            fn on_event(&mut self, event: &SystemEvent) -> EventResult {
                if matches!(event, SystemEvent::PointerDown { .. }) {
                    self.size.set(self.size.get() * 2.0);
                    EventResult::Handled
                } else {
                    EventResult::NotHandled
                }
            }
        }

        let mut tree = WidgetTree::new();
        let sv_id = tree.set_root(Box::new(
            ScrollView::new(ScrollDirection::Vertical).size(300.0, 200.0),
        ));
        tree.add_child(
            sv_id,
            Box::new(GrowWidget {
                size: std::cell::Cell::new(100.0),
            }),
        );

        tree.layout();
        let max_y = |tree: &WidgetTree| -> f32 {
            let sv = tree.get(sv_id).unwrap();
            let sv_ref: &ScrollView = sv.component().as_any().downcast_ref().unwrap();
            sv_ref.max_scroll_y()
        };
        assert_eq!(max_y(&tree), 0.0);

        tree.dispatch_event(&SystemEvent::PointerDown {
            pos: Point::new(50.0, 10.0),
            button: crate::ui::MouseButton::Left,
            mods: crate::native::traits::input::KeyMod::NONE,
        });
        tree.layout();
        assert_eq!(max_y(&tree), 0.0);

        tree.dispatch_event(&SystemEvent::PointerDown {
            pos: Point::new(50.0, 10.0),
            button: crate::ui::MouseButton::Left,
            mods: crate::native::traits::input::KeyMod::NONE,
        });
        tree.layout();
        let max = max_y(&tree);
        assert!(
            (max - 200.0).abs() < 1.0,
            "expected max_scroll_y near 200, got {max}"
        );
    }

    #[test]
    fn collapse_expand_updates_scrollview_content_bounds() {
        let mut tree = WidgetTree::new();
        let sv_id = tree.set_root(Box::new(
            ScrollView::new(ScrollDirection::Vertical).size(300.0, 200.0),
        ));
        let container_id = tree.add_child(
            sv_id,
            Box::new(Container::new().size(300.0, 0.0).dir(FlexDirection::Column)),
        );
        let space_id = tree.add_child(
            container_id,
            Box::new(
                Space::new()
                    .width(300.0)
                    .height(140.0)
                    .direction(FlexDirection::Column)
                    .align(AlignItems::Stretch),
            ),
        );
        let long_content = "line\nline\nline\nline\nline\nline\nline";
        tree.add_child(
            space_id,
            Box::new(Collapse::new().panels(vec![
                CollapsePanel::new("Panel A", "short"),
                CollapsePanel::new("Panel B", long_content),
                CollapsePanel::new("Panel C", "short"),
            ])),
        );

        tree.layout();
        let max_before = tree
            .get(sv_id)
            .and_then(|n| {
                n.component()
                    .as_any()
                    .downcast_ref::<ScrollView>()
                    .map(|sv| sv.max_scroll_y())
            })
            .unwrap();
        assert_eq!(max_before, 0.0);

        tree.dispatch_event(&SystemEvent::PointerDown {
            pos: Point::new(50.0, 45.0),
            button: crate::ui::MouseButton::Left,
            mods: crate::native::traits::input::KeyMod::NONE,
        });
        tree.layout();
        let max_after = tree
            .get(sv_id)
            .and_then(|n| {
                n.component()
                    .as_any()
                    .downcast_ref::<ScrollView>()
                    .map(|sv| sv.max_scroll_y())
            })
            .unwrap();

        assert!(
            max_after > max_before,
            "expected max_scroll_y to increase after expand, before={max_before}, after={max_after}"
        );
    }

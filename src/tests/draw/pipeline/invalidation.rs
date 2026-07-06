use super::*;

    #[test]
    fn paint_merges_to_dirty_region() {
        let mut q = InvalidationQueue::new();
        q.push(Invalidation::Paint {
            id: 1,
            rect: Some(Rect::new(0.0, 0.0, 10.0, 10.0)),
        });
        q.push(Invalidation::Paint {
            id: 2,
            rect: Some(Rect::new(20.0, 0.0, 10.0, 10.0)),
        });
        let region = q.dirty_region();
        assert!(!region.is_empty());
        assert_eq!(region.rects().len(), 2);
    }

    #[test]
    fn layout_only_does_not_imply_paint() {
        let mut q = InvalidationQueue::new();
        q.push(Invalidation::Layout(0));
        assert!(!q.is_empty());
        assert!(q.has_layout());
        assert!(!q.has_paint_or_composite());
        assert!(q.dirty_region().is_empty());
    }

    #[test]
    fn paint_none_expands_full_frame() {
        let mut q = InvalidationQueue::new();
        q.push(Invalidation::Paint { id: 0, rect: None });
        assert!(q.dirty_region().full_frame);
        assert!(q.needs_full_frame());
    }

    #[test]
    fn composite_scroll_extract() {
        let mut q = InvalidationQueue::new();
        let frame = Rect::new(0.0, 0.0, 100.0, 200.0);
        q.push(Invalidation::Composite {
            rect: frame,
            scroll: Some(ScrollDelta { dx: 0.0, dy: -10.0 }),
        });
        let (r, s) = q.scroll_composite().expect("scroll");
        assert_eq!(r, frame);
        assert!((s.dy + 10.0).abs() < 1e-6);
    }

    #[test]
    fn merge_same_node_paint_rects() {
        let mut q = InvalidationQueue::new();
        q.push(Invalidation::Paint {
            id: 3,
            rect: Some(Rect::new(0.0, 0.0, 10.0, 10.0)),
        });
        q.push(Invalidation::Paint {
            id: 3,
            rect: Some(Rect::new(5.0, 5.0, 10.0, 10.0)),
        });
        assert_eq!(q.items.len(), 1);
        let region = q.dirty_region();
        assert_eq!(region.rects().len(), 1);
        let r = region.rects()[0];
        assert!((r.x - 0.0).abs() < 1e-6);
        assert!((r.w - 15.0).abs() < 1e-6);
    }

    #[test]
    fn node_needs_paint_targeted() {
        let mut q = InvalidationQueue::new();
        q.push(Invalidation::Paint {
            id: 7,
            rect: Some(Rect::new(0.0, 0.0, 5.0, 5.0)),
        });
        assert!(q.node_needs_paint(7));
        assert!(!q.node_needs_paint(8));
    }

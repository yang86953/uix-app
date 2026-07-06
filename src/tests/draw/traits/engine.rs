use super::*;
    use crate::core::Rect;

    #[test]
    fn full_redraw_rects_none() {
        let strategy = UpdateStrategy::FullRedraw;
        assert!(strategy.rects().is_none());
    }

    #[test]
    fn full_redraw_should_clear() {
        let strategy = UpdateStrategy::FullRedraw;
        assert!(strategy.should_clear());
    }

    #[test]
    fn dirty_rects_rects_some() {
        let rects = vec![Rect::new(0.0, 0.0, 10.0, 10.0)];
        let strategy = UpdateStrategy::DirtyRects(rects.clone());
        assert_eq!(strategy.rects(), Some(rects.as_slice()));
    }

    #[test]
    fn dirty_rects_empty_should_clear() {
        let strategy = UpdateStrategy::DirtyRects(vec![]);
        assert!(strategy.should_clear());
    }

    #[test]
    fn update_strategy_debug_clone() {
        let s1 = UpdateStrategy::FullRedraw;
        let _s2 = s1.clone();
        let _debug = format!("{:?}", s1);
    }

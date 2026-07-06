use super::*;
    use crate::draw::NullEngine;
    use crate::ui::widgets::container::Container;

    #[test]
    fn sync_root_frame_mismatch() {
        let mut tree = WidgetTree::new();
        let rid = tree.set_root(Box::new(Container::new()));
        if let Some(root) = tree.get_mut(rid) {
            root.set_frame(Rect::new(0.0, 0.0, 800.0, 600.0));
        }
        let mut engine = NullEngine::new();
        sync_root_frame_to_engine(&mut tree, &mut engine);
        let root = tree.get(rid).unwrap();
        assert!(root.frame().w < 800.0);
        assert!(root.frame().h < 600.0);
    }

    #[test]
    fn sync_root_frame_already_matched() {
        let mut tree = WidgetTree::new();
        tree.set_root(Box::new(Container::new()));
        let mut engine = NullEngine::new();
        sync_root_frame_to_engine(&mut tree, &mut engine);
        if let Some(rid) = tree.root_id() {
            let root = tree.get(rid).unwrap();
            assert_eq!(root.frame(), Rect::new(0.0, 0.0, 0.0, 0.0));
        }
    }

use crate::ui::traits::WidgetAnimation;
use crate::ui::widgets::display::TreeNode;
use crate::ui::widgets::input::TreeSelect;

#[test]
fn tree_select_enter_animation_finishes_open() {
    let mut tree_select = TreeSelect::new().nodes(vec![TreeNode::new("Alpha", "alpha")]);

    tree_select.open();
    assert!(tree_select.is_open());
    assert!(tree_select.is_present());

    assert!(WidgetAnimation::update_animation(&mut tree_select, 0.05));
    assert!(!WidgetAnimation::update_animation(&mut tree_select, 1.0));
    assert!(tree_select.is_open());
    assert!(tree_select.is_present());
}

#[test]
fn tree_select_exit_animation_stays_present_until_finished() {
    let mut tree_select = TreeSelect::new().nodes(vec![TreeNode::new("Alpha", "alpha")]);
    tree_select.open();
    assert!(!WidgetAnimation::update_animation(&mut tree_select, 1.0));

    tree_select.close();
    assert!(!tree_select.is_open());
    assert!(tree_select.is_present());

    assert!(WidgetAnimation::update_animation(&mut tree_select, 0.03));
    assert!(!WidgetAnimation::update_animation(&mut tree_select, 1.0));
    assert!(!tree_select.is_open());
    assert!(!tree_select.is_present());
}

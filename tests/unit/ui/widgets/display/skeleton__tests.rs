//! Skeleton UIX 声明壳回归测试。

use super::*;

/// UIX 声明壳必须保持原 Skeleton 类型、配置和单叶节点形状。
#[test]
fn uix_shell_preserves_single_kernel_leaf() {
    let node = View::build(
        Skeleton::new()
            .shape(SkeletonShape::Text)
            .size(240.0, 48.0)
            .active(true),
    );
    assert!(node.children.is_empty());
    assert!(node.widget.as_any().is::<Skeleton>());
    assert_eq!(
        node.widget.snapshot_fields(),
        SnapshotFields::Skeleton {
            shape: SkeletonShape::Text,
            width: 240.0,
            height: 48.0,
        }
    );
}

/// 无分配生产枚举必须保持原段落行高、间距和末行宽度。
#[test]
fn paragraph_geometry_stays_exact_after_streaming_refactor() {
    let skeleton = Skeleton::new().paragraph(3);
    let rects = skeleton.paragraph_rects_for_test(Rect::new(0.0, 0.0, 120.0, 48.0));
    assert_eq!(
        rects,
        vec![
            Rect::new(0.0, 2.0, 120.0, 12.0),
            Rect::new(0.0, 18.0, 120.0, 12.0),
            Rect::new(0.0, 34.0, 72.0, 12.0),
        ]
    );
}

//! Skeleton UIX 视觉声明回归测试。

use super::*;

/// UIX 视觉声明必须保持原 Skeleton 类型、作者配置和单叶节点形状。
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
    let kernel = node
        .widget
        .as_any()
        .downcast_ref::<Skeleton>()
        .expect("UIX 根必须保留 Skeleton 内核");
    // 作者尺寸优先，段落、圆角、流光和主题角色由 UIX 注入。
    assert_eq!(kernel.height_source, SkeletonHeightSource::Authored);
    assert_eq!(kernel.visual.paragraph_line_height, 12.0);
    assert_eq!(kernel.visual.paragraph_gap, 4.0);
    assert_eq!(kernel.visual.shimmer_width_ratio, 0.35);
    assert_eq!(kernel.visual.shimmer_speed, 0.8);
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

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

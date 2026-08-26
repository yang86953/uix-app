// 引入路径段类型，用于检查 helper 没有丢失 Close 语义。
use crate::draw::geometry::path::PathSegment;
// 引入统一矩形和圆角值。
use crate::core::Rect;
use crate::draw::geometry::types::Radius;

// 验证任意圆角输入会生成有限、闭合且覆盖原矩形边界的路径。
#[test]
fn rounded_rect_path_is_closed_and_bounded() {
    // 构造非对称圆角，覆盖四个独立角的路径分支。
    let path = super::rounded_rect_path(
        Rect::new(10.0, 20.0, 100.0, 60.0),
        Some(Radius {
            tl: 8.0,
            tr: 16.0,
            br: 12.0,
            bl: 4.0,
        }),
    );
    // 路径必须有段且以 Close 结束，保证 tessellator 使用闭合轮廓。
    assert!(!path.is_empty());
    assert!(matches!(path.segments().last(), Some(PathSegment::Close)));
    // 控制点边界应覆盖原矩形，不得产生非有限或反向尺寸。
    // 缺失边界说明 helper 生成了不完整路径，直接让测试失败并保留原因。
    let Some(bounds) = path.bounds() else {
        // 附带路径段数，便于判断 helper 是否丢失了闭合轮廓。
        panic!(
            "rounded rect path has no bounds (segments={})",
            path.segments().len()
        );
    };
    assert!(bounds.x <= 10.0);
    assert!(bounds.y <= 20.0);
    assert!(bounds.x + bounds.w >= 110.0);
    assert!(bounds.y + bounds.h >= 80.0);
}

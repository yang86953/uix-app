// 复用表格实现与父模块已导入的几何类型。
use super::*;
// 引入固定列回归测试使用的列类型。
use super::super::types::TableColumn;
// 引入固定列命中测试使用的坐标类型。
use crate::core::Point;

// 标记固定列重叠句柄绘制层级契约。
#[test]
// 验证重合边缘命中最后绘制的右固定列句柄。
fn overlapping_fixed_resize_handles_hit_topmost_painted_zone() {
    // 构造宽八十像素的可调整左固定列。
    let left = TableColumn::new("左列", 80.0)
        // 启用左列调整宽度交互。
        .resizable(true)
        // 将第一列固定到视口左侧。
        .fixed(super::super::types::Fixed::Left);
    // 构造右固定区中边缘落在八十像素处的第一列。
    let right_inner = TableColumn::new("右内列", 20.0)
        // 启用右内列调整宽度交互。
        .resizable(true)
        // 将第二列固定到视口右侧。
        .fixed(super::super::types::Fixed::Right);
    // 构造右固定区最外侧的第二列。
    let right_outer = TableColumn::new("右外列", 20.0)
        // 将第三列固定到视口右侧。
        .fixed(super::super::types::Fixed::Right);
    // 在一百像素视口中让左列和右内列的右边缘同为八十像素。
    let table = Table::new()
        // 安装会形成重叠边缘的三列。
        .columns(vec![left, right_inner, right_outer])
        // 固定视口尺寸以触发左右固定区重叠。
        .size(100.0, 80.0);
    // 重合点必须选择绘制层级更高的右固定列句柄。
    assert_eq!(
        table.resize_handle_at_point(Point::new(80.0, 10.0)),
        Some(1)
    );
    // 结束固定列重叠句柄命中契约。
}

// 加载圆点必须通过一次相位求值和固定旋转递推保持八向几何。
#[test]
fn loading_dot_recurrence_preserves_cardinal_offsets() {
    // 保存八个访问结果，仅用于验证递推后的几何位置。
    let mut offsets = Vec::new();
    Table::for_each_loading_dot_offset(
        0.0,
        10.0,
        8,
        std::f32::consts::FRAC_1_SQRT_2,
        std::f32::consts::FRAC_1_SQRT_2,
        |_, x, y| offsets.push((x, y)),
    );
    // 八个圆点必须完整访问且不增减序列长度。
    assert_eq!(offsets.len(), 8);
    // 每隔两个圆点应落在一个轴向基点，允许浮点递推误差。
    let expected = [(10.0, 0.0), (0.0, 10.0), (-10.0, 0.0), (0.0, -10.0)];
    for ((actual_x, actual_y), (expected_x, expected_y)) in offsets.iter().step_by(2).zip(expected)
    {
        assert!((actual_x - expected_x).abs() < 0.0001);
        assert!((actual_y - expected_y).abs() < 0.0001);
    }
}

// 页码不变时必须复用同一个字符串缓冲区。
#[test]
fn pagination_label_reuses_cached_allocation() {
    let table = Table::new();
    let first_pointer = {
        let label = table.pagination_label(1, 12);
        assert_eq!(&*label, "1 / 12");
        label.as_ptr()
    };
    let first_capacity = table.pagination_label_cache.borrow().value.capacity();
    let second_pointer = {
        let label = table.pagination_label(1, 12);
        assert_eq!(&*label, "1 / 12");
        label.as_ptr()
    };
    assert_eq!(second_pointer, first_pointer);
    assert_eq!(
        table.pagination_label_cache.borrow().value.capacity(),
        first_capacity
    );
}
// 结束表格列宽句柄测试模块。

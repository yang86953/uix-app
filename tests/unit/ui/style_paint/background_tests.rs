// 引入被测背景平铺纯函数。
use super::background_tiles;
// 引入测试几何值。
use crate::core::{Rect, Size};
// 引入 Style 背景公开值契约。
use crate::ui::theme::style::{
    // 引入单轴定位枚举。
    BackgroundAxisPosition,
    // 引入背景图来源枚举。
    BackgroundImage,
    // 引入二维定位值。
    BackgroundPosition,
    // 引入重复方式枚举。
    BackgroundRepeat,
    // 引入样式聚合值。
    Style,
};

// 建立所有平铺测试共享的背景盒。
fn target() -> Rect {
    // 返回一百乘八十的原点背景盒。
    Rect::new(0.0, 0.0, 100.0, 80.0)
}

// 建立所有平铺测试共享的图片固有尺寸。
fn image_size() -> Size {
    // 返回二十乘十的图片尺寸。
    Size::new(20.0, 10.0)
}

// 建立居中定位测试值。
fn centered() -> BackgroundPosition {
    // 两个轴都使用剩余空间中点。
    BackgroundPosition::new(
        // 水平居中。
        BackgroundAxisPosition::Center,
        // 垂直居中。
        BackgroundAxisPosition::Center,
    )
}

// 验证不重复图片使用剩余空间居中且保持固有尺寸。
#[test]
fn background_runtime_no_repeat_uses_intrinsic_centered_rect() {
    // 生成单张居中图片。
    let tiles = background_tiles(
        // 使用共享背景盒。
        target(),
        // 使用共享固有尺寸。
        image_size(),
        // 使用二维居中定位。
        centered(),
        // 关闭两个轴的重复。
        BackgroundRepeat::NoRepeat,
    );
    // 只应产生一张图片。
    assert_eq!(tiles, vec![Rect::new(40.0, 35.0, 20.0, 10.0)]);
}

// 验证四种重复方式形成确定的轴笛卡尔积。
#[test]
fn background_runtime_repeat_modes_generate_expected_tiles() {
    // 水平重复应产生五列和一行。
    let repeat_x = background_tiles(
        target(),
        image_size(),
        centered(),
        BackgroundRepeat::RepeatX,
    );
    // 水平重复保持定位产生的垂直坐标。
    assert_eq!(repeat_x.len(), 5);
    // 检查第一张图片的位置。
    assert_eq!(repeat_x.first(), Some(&Rect::new(0.0, 35.0, 20.0, 10.0)));
    // 垂直重复应包含跨越上下边缘的九行。
    let repeat_y = background_tiles(
        target(),
        image_size(),
        centered(),
        BackgroundRepeat::RepeatY,
    );
    // 垂直相位为五像素，因此需要九张图片覆盖完整高度。
    assert_eq!(repeat_y.len(), 9);
    // 检查跨越顶部边缘的第一张图片。
    assert_eq!(repeat_y.first(), Some(&Rect::new(40.0, -5.0, 20.0, 10.0)));
    // 双轴重复组合五列与九行。
    let repeat = background_tiles(target(), image_size(), centered(), BackgroundRepeat::Repeat);
    // 检查完整笛卡尔积数量。
    assert_eq!(repeat.len(), 45);
}

// 验证百分比基于背景盒和图片之间的剩余空间。
#[test]
fn background_runtime_percent_uses_residual_space() {
    // 创建带非零原点的背景盒。
    let target = Rect::new(5.0, 7.0, 100.0, 80.0);
    // 创建两个轴都为百分之百的定位。
    let position = BackgroundPosition::new(
        // 水平使用全部剩余空间。
        BackgroundAxisPosition::Percent(1.0),
        // 垂直使用全部剩余空间。
        BackgroundAxisPosition::Percent(1.0),
    );
    // 生成不重复图片。
    let tiles = background_tiles(target, image_size(), position, BackgroundRepeat::NoRepeat);
    // 图片右下边缘应与背景盒右下边缘对齐。
    assert_eq!(tiles, vec![Rect::new(85.0, 77.0, 20.0, 10.0)]);
}

// 验证无效尺寸不会进入平铺循环。
#[test]
fn background_runtime_invalid_extent_returns_empty_tiles() {
    // 零宽图片不能形成有效平铺步长。
    let zero_width = background_tiles(
        // 使用有效背景盒。
        target(),
        // 将图片宽度设为零。
        Size::new(0.0, 10.0),
        // 使用默认左上角定位。
        BackgroundPosition::default(),
        // 使用默认双轴重复。
        BackgroundRepeat::Repeat,
    );
    // 算法必须安全返回空集合。
    assert!(zero_width.is_empty());
}

// 验证 Style 合并保留显式默认值与未声明之间的差异。
#[test]
fn background_runtime_style_defaults_and_explicit_values_reconcile() {
    // 默认样式不声明背景图来源。
    let defaults = Style::default();
    // 默认来源保持未声明。
    assert_eq!(defaults.background_image, None);
    // 默认有效定位为左上角。
    assert_eq!(
        defaults.effective_background_position(),
        BackgroundPosition::default()
    );
    // 默认有效重复沿两个轴进行。
    assert_eq!(
        defaults.effective_background_repeat(),
        BackgroundRepeat::Repeat
    );
    // 建立具有图片、居中和不重复的基础样式。
    let base = Style::default()
        // 设置本地图片路径。
        .with_background_image(BackgroundImage::Url("asset.png".to_owned()))
        // 设置二维居中。
        .with_background_position(centered())
        // 关闭重复。
        .with_background_repeat(BackgroundRepeat::NoRepeat);
    // 用显式 none、左上角和 repeat 覆盖基础样式。
    let merged = base.apply(
        // 从未声明样式开始建立覆盖层。
        Style::default()
            // 显式关闭背景图。
            .with_background_image(BackgroundImage::None)
            // 显式恢复左上角。
            .with_background_position(BackgroundPosition::default())
            // 显式恢复双轴重复。
            .with_background_repeat(BackgroundRepeat::Repeat),
    );
    // 显式 none 必须覆盖原路径。
    assert_eq!(merged.background_image, Some(BackgroundImage::None));
    // 显式默认定位必须覆盖原居中值。
    assert_eq!(
        merged.effective_background_position(),
        BackgroundPosition::default()
    );
    // 显式默认重复必须覆盖原不重复值。
    assert_eq!(
        merged.effective_background_repeat(),
        BackgroundRepeat::Repeat
    );
}

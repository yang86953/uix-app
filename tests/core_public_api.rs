// 引入标准库输入输出错误种类以核对框架错误码映射。
use std::io::ErrorKind;

// 引入核心公开值类型以从使用方视角锁定基础契约。
use uix_app::core::{
    Constraints,
    // 引入边距、错误码、错误值与严重度类型。
    EdgeInsets,
    Errc,
    Error,
    ErrorSeverity,
    // 引入二维几何和窗口身份类型。
    Point,
    Rect,
    Size,
    // 引入组件身份和尺寸约束类型。
    WidgetId,
    WindowId,
    // 结束核心公开值类型导入。
};

// 将公开身份值的构造、读取、比较和显示约束注册为测试。
#[test]
// 验证组件身份和窗口身份保留稳定的值语义。
fn identity_values_preserve_public_parts_and_ordering() {
    // 默认组件身份应指向零号槽位的初始代际。
    let default_widget = WidgetId::default();
    // 核对默认组件身份的槽位。
    assert_eq!(default_widget.slot(), 0);
    // 核对默认组件身份的代际。
    assert_eq!(default_widget.generation(), 0);
    // 核对无树作用域组件身份的稳定文本格式。
    assert_eq!(default_widget.to_string(), "0:0");

    // 创建同一槽位的后续代际身份。
    let recycled_widget = WidgetId::from_parts(7, 3);
    // 核对公开槽位读取保持构造值。
    assert_eq!(recycled_widget.slot(), 7);
    // 核对公开代际读取保持构造值。
    assert_eq!(recycled_widget.generation(), 3);
    // 核对代际参与身份顺序，避免旧身份与新身份混淆。
    assert!(WidgetId::new(7) < recycled_widget);
    // 核对带代际身份的稳定文本格式。
    assert_eq!(recycled_widget.to_string(), "7:3");

    // 应用根窗口身份应与原始零值保持一致。
    assert_eq!(WindowId::root(), WindowId::ROOT);
    // 根窗口身份的原始值必须为零。
    assert_eq!(WindowId::root().raw(), 0);
    // 非根窗口身份应保留调用方提供的原始值。
    assert_eq!(WindowId::new(42).raw(), 42);
    // 窗口身份应按原始值提供稳定顺序。
    assert!(WindowId::ROOT < WindowId::new(1));
    // 结束身份值语义测试。
}

// 将尺寸规范化与约束裁剪契约注册为测试。
#[test]
// 验证尺寸和约束正确处理非数值、负值与上下界。
fn size_and_constraints_preserve_normalization_contracts() {
    // 构造包含非数值宽度和负高度的尺寸。
    let normalized = Size::new(f32::NAN, -5.0);
    // 非数值宽度必须被规范化为零。
    assert_eq!(normalized.w, 0.0);
    // 负高度由调用方决定如何处理，因此必须保留。
    assert_eq!(normalized.h, -5.0);

    // 构造具有明确最小值和最大值的尺寸约束。
    let constraints = Constraints::new(Size::new(10.0, 20.0), Size::new(100.0, 80.0), None);
    // 对同时越过上下界的尺寸执行裁剪。
    let clamped = constraints.clamp(Size::new(5.0, 120.0));
    // 宽度必须提升到最小宽度。
    assert_eq!(clamped.w, 10.0);
    // 高度必须降低到最大高度。
    assert_eq!(clamped.h, 80.0);

    // 宽松约束应使用零尺寸作为最小值。
    let loose = Constraints::loose(Size::new(50.0, 60.0));
    // 核对宽松约束的最小尺寸。
    assert_eq!(loose.min, Size::zero());
    // 核对宽松约束不强制确定尺寸。
    assert_eq!(loose.definite, None);
    // 默认约束应与无约束构造结果一致。
    assert_eq!(Constraints::default(), Constraints::unconstrained());
    // 结束尺寸和约束契约测试。
}

// 将矩形空间运算契约注册为测试。
#[test]
// 验证矩形包含、内缩、交集和并集的边界语义。
fn rect_operations_preserve_closed_and_positive_area_boundaries() {
    // 创建作为空间运算基准的矩形。
    let rect = Rect::new(10.0, 20.0, 30.0, 40.0);
    // 闭合边界应包含右下角点。
    assert!(rect.contains(Point::new(40.0, 60.0)));
    // 边界外的点不得被包含。
    assert!(!rect.contains(Point::new(40.1, 60.0)));

    // 使用四边不同的边距内缩矩形。
    let inset = rect.inset(EdgeInsets::new(2.0, 3.0, 5.0, 7.0));
    // 核对内缩后的位置和尺寸。
    assert_eq!(inset, Rect::new(12.0, 23.0, 23.0, 30.0));
    // 过量边距不得产生负尺寸。
    assert_eq!(rect.inset(EdgeInsets::uniform(100.0)).w, 0.0);

    // 创建与基准矩形部分重叠的矩形。
    let overlapping = Rect::new(30.0, 50.0, 20.0, 20.0);
    // 正面积重叠应返回精确交集。
    assert_eq!(
        rect.intersect(&overlapping),
        Some(Rect::new(30.0, 50.0, 10.0, 10.0))
    );
    // 并集应返回包围两个矩形的最小矩形。
    assert_eq!(rect.union(&overlapping), Rect::new(10.0, 20.0, 40.0, 50.0));
    // 仅边界接触不构成正面积交集。
    assert_eq!(rect.intersect(&Rect::new(40.0, 20.0, 5.0, 5.0)), None);
    // 结束矩形空间运算测试。
}

// 将边距构造和聚合契约注册为测试。
#[test]
// 验证统一边距与 CSS 顺序元组转换保持方向语义。
fn edge_insets_preserve_directional_mapping() {
    // 从上右下左顺序的元组构造边距。
    let edges = EdgeInsets::from((1.0, 2.0, 3.0, 4.0));
    // 元组第四项应映射为左边距。
    assert_eq!(edges.left, 4.0);
    // 元组第一项应映射为上边距。
    assert_eq!(edges.top, 1.0);
    // 元组第二项应映射为右边距。
    assert_eq!(edges.right, 2.0);
    // 元组第三项应映射为下边距。
    assert_eq!(edges.bottom, 3.0);
    // 水平聚合应只包含左右两边。
    assert_eq!(edges.horizontal(), 6.0);
    // 垂直聚合应只包含上下两边。
    assert_eq!(edges.vertical(), 4.0);
    // 标量转换应为四个方向应用同一数值。
    assert_eq!(EdgeInsets::from(2.5), EdgeInsets::uniform(2.5));
    // 结束边距方向语义测试。
}

// 将错误码分类、显示与标准库映射契约注册为测试。
#[test]
// 验证错误码在各稳定数值区间内保持类别和输入输出映射。
fn error_codes_preserve_categories_display_and_io_mapping() {
    // 通用错误码应归入通用类别。
    assert_eq!(Errc::InvalidArgument.category(), "general");
    // 输入输出错误码应归入输入输出类别。
    assert_eq!(Errc::FileNotFound.category(), "io");
    // 网络错误码应归入网络类别。
    assert_eq!(Errc::ConnectionReset.category(), "network");
    // 协议错误码应归入协议类别。
    assert_eq!(Errc::ParseError.category(), "protocol");
    // 并发错误码应归入并发类别。
    assert_eq!(Errc::TaskAbandoned.category(), "concurrency");
    // 平台错误码应归入平台类别。
    assert_eq!(Errc::GraphicsDeviceLost.category(), "platform");
    // 应用保留起点应归入应用类别。
    assert_eq!(Errc::AppDomainBase.category(), "application");
    // 错误码显示值应保持稳定的蛇形命名。
    assert_eq!(Errc::InvalidOperation.to_string(), "invalid_operation");
    // 文件不存在应映射到标准库的 NotFound。
    assert_eq!(Errc::FileNotFound.to_io_kind(), Some(ErrorKind::NotFound));
    // 无直接标准库对应项的框架错误码应返回空值。
    assert_eq!(Errc::GraphicsDeviceLost.to_io_kind(), None);
    // 结束错误码契约测试。
}

// 将错误严重度的顺序和显示契约注册为测试。
#[test]
// 验证严重度只在致命级别触发中止语义。
fn error_severity_preserves_ordering_and_abort_threshold() {
    // 普通错误应低于致命错误。
    assert!(ErrorSeverity::Error < ErrorSeverity::Fatal);
    // 警告不得被识别为致命错误。
    assert!(!ErrorSeverity::Warning.is_fatal());
    // 普通错误不得要求责任边界中止。
    assert!(!ErrorSeverity::Error.should_abort());
    // 致命错误必须要求责任边界中止。
    assert!(ErrorSeverity::Fatal.should_abort());
    // 警告显示值应保持稳定缩写。
    assert_eq!(ErrorSeverity::Warning.to_string(), "WARN");
    // 结束错误严重度测试。
}

// 将错误值、调用位置和原因链契约注册为测试。
#[test]
// 验证错误值保留消息、严重度、位置和完整原因链。
fn error_values_preserve_factories_location_and_source_chain() {
    // 创建具有显式位置的根因。
    let root = Error::with_location(Errc::IoError, "disk unavailable", "storage.rs", 41);
    // 创建位于原因链中间的错误。
    let middle = Error::with_location(Errc::ParseError, "invalid document", "parser.rs", 12)
        .with_source(root);
    // 创建顶层致命错误并附加现有原因链。
    let error = Error::fatal(Errc::InvalidState, "settings load failed").with_source(middle);

    // 原因链深度应包含中间错误和根因。
    assert_eq!(error.depth(), 2);
    // 根因应保留最底层错误码。
    assert_eq!(error.root_cause().code(), Errc::IoError);
    // 根因应保留显式源码文件。
    assert_eq!(error.root_cause().file(), "storage.rs");
    // 根因应保留显式源码行号。
    assert_eq!(error.root_cause().line(), 41);
    // 致命工厂应设置致命严重度。
    assert_eq!(error.severity(), ErrorSeverity::Fatal);
    // 完整说明应包含顶层消息。
    assert!(error.what().contains("settings load failed"));
    // 完整说明应包含中间错误消息。
    assert!(error.what().contains("invalid document"));
    // 完整说明应包含根因消息。
    assert!(error.what().contains("disk unavailable"));

    // 默认错误应表达无错误信号。
    let empty = Error::default();
    // 默认错误必须被识别为 None 错误码。
    assert!(empty.is_none());
    // 无错误信号不得被识别为实际错误值。
    assert!(!empty.has_value());
    // 便利工厂应生成对应的稳定错误码。
    assert!(Error::not_found("missing").is(Errc::NotFound));
    // 结束错误值和原因链测试。
}

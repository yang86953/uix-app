// 引入当前 recorder 实现和 Canvas2D 依赖类型。
use super::*;
// 引入命令枚举以审计实际记录载荷。
use crate::draw::painting::FrameCommand;

// 加载 Additive 基础形状的独立回归测试。
include!("canvas2d_shape_tests.rs");

// 继续在同一测试模块内加载纯平移 transform 的独立回归测试。
include!("canvas2d_test_tail.rs");

// 继续在同一测试模块内加载 Additive sampled soft 分段回归测试。
include!("canvas2d_additive_soft_tests.rs");

// 继续加载 Additive 仿射描边 sampled soft 分段回归测试。
include!("canvas2d_additive_stroke_soft_tests.rs");

// 继续在同一测试模块内加载 Additive opacity 的独立回归测试。
include!("canvas2d_opacity_tests.rs");

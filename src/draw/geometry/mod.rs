//! 基础图形原语：颜色、路径、描边、类型与辅助算法。

/// 颜色表示、解析与混合运算。
pub mod color;
/// 有界线性渐变值与色标。
pub mod gradient;
pub mod flattener;
pub mod path;
pub mod spatial;
pub mod stroker;
pub mod tessellator;
/// 绘制几何共享的变换与样式类型。
pub mod types;

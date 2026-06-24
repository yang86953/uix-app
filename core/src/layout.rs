// ============================================================================
// core/layout.rs — 布局方向与对齐枚举（CSS Flexbox 词汇）
//
// 框架级布局原语。所有使用 Flexbox 布局的层（ui、graphics LayerTree）
// 共享同一套方向/对齐定义，无需跨层依赖。
// ============================================================================

/// Flex 容器主轴方向。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FlexDirection {
    #[default]
    Row,
    Column,
    RowReverse,
    ColumnReverse,
}

/// 主轴对齐方式。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum JustifyContent {
    #[default]
    Start,
    Center,
    End,
    SpaceBetween,
    SpaceAround,
    SpaceEvenly,
    Stretch,
}

/// 交叉轴对齐方式。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AlignItems {
    Start,
    Center,
    End,
    #[default]
    Stretch,
}

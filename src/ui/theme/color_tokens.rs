//! Color token trait — brand, neutral, semantic, shadow, and link colors.
//!
//! `ShadowToken` 与 `IColorTokens` 定义在 `crate::draw::painting`。

pub use crate::draw::painting::{IColorTokens, ShadowToken};

/// Neutral semantic roles mapped onto the concrete token fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NeutralRole {
    Text,
    TextSecondary,
    TextTertiary,
    TextQuaternary,
    TextInverse,
    Border,
    BorderSecondary,
    Fill,
    FillSecondary,
    FillTertiary,
    FillQuaternary,
    BgContainer,
    BgElevated,
    BgLayout,
    BgMask,
}

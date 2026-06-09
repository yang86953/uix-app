//! Typography token trait — font family, sizes, weights, and line height.
//!
//! Domain sub-trait of the Ant Design 5 token system. Combined via
//! `TokenProvider` supertrait in `theme.rs`.

/// Typography design tokens — font family, font sizes, font weights, and line height.
///
/// All font size / weight methods provide Ant Design 5 light mode defaults.
pub trait ITypographyTokens: Send + Sync {
    fn font_family(&self) -> &str;

    fn font_size_sm(&self) -> f32 {
        12.0
    }
    fn font_size(&self) -> f32 {
        14.0
    }
    fn font_size_lg(&self) -> f32 {
        16.0
    }
    fn font_size_xl(&self) -> f32 {
        20.0
    }
    fn font_size_heading_1(&self) -> f32 {
        38.0
    }
    fn font_size_heading_2(&self) -> f32 {
        30.0
    }
    fn font_size_heading_3(&self) -> f32 {
        24.0
    }
    fn font_size_heading_4(&self) -> f32 {
        20.0
    }
    fn font_size_heading_5(&self) -> f32 {
        16.0
    }

    fn font_weight_regular(&self) -> f32 {
        400.0
    }
    fn font_weight_medium(&self) -> f32 {
        500.0
    }
    fn font_weight_semibold(&self) -> f32 {
        600.0
    }
    fn font_weight_bold(&self) -> f32 {
        700.0
    }

    fn line_height(&self) -> f32 {
        1.5715
    }
}

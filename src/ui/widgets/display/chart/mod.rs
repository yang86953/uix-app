//! Chart widgets — bar, pie/donut, and line charts.
//!
//! All chart types render with theme-aware colors and auto-scaling.

fn responsive_extent(responsive: bool, available: f32, intrinsic: f32) -> f32 {
    if responsive && available.is_finite() && available > 0.0 && available < f32::MAX {
        available
    } else {
        intrinsic
    }
}

pub mod advanced;
pub mod bar_chart;
pub mod line_chart;
pub mod pie_chart;
mod value_label;

pub use advanced::*;
pub use bar_chart::*;
pub use line_chart::*;
pub use pie_chart::*;

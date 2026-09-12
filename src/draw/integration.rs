//! CPU-side drawing and text contracts for independent UI libraries.
pub mod geometry {
    pub mod path { pub use crate::draw::geometry::path::*; }
    pub mod spatial { pub use crate::draw::geometry::spatial::PhysicalUnit; }
}
pub mod painting { pub use crate::draw::painting::PaintPass; }
pub mod renderer { pub use crate::draw::Invalidation; pub use crate::draw::renderer::invalidate_paint_handle; }
pub mod scene { pub use crate::draw::scene::PicturePolicy; }
pub mod resources {
    pub mod image { pub use crate::draw::{BitmapHandle, ImageService}; }
    pub mod font {
        pub mod font_service { pub use crate::draw::FontService; }
        pub mod text_backend { pub use crate::draw::resources::font::text_backend::*; }
        pub mod text_index { pub use crate::draw::resources::font::text_index::*; }
        pub mod bidi { pub use crate::draw::resources::font::bidi::BidiAnalysis; }
        pub mod line_break { pub use crate::draw::resources::font::line_break::*; }
    }
}

/// Manages image loading and rendering.
///
/// Uses a slot index instead of a raw pointer for safe access
/// to engine-managed image resources.
#[derive(Default)]
pub struct ImageManager {
    image_path: Option<String>,
    /// Index into the engine's internal image slot array.
    image_index: Option<usize>,
    tint_color: Option<uix_graphics::Color>,
}


impl ImageManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_image(&mut self, path: impl Into<String>) {
        self.image_path = Some(path.into());
    }

    pub fn image_path(&self) -> Option<&str> {
        self.image_path.as_deref()
    }

    /// Set the engine-internal slot index for this image.
    /// The index corresponds to the position in the engine's `Vec<ImageSlot>`.
    pub fn set_index(&mut self, index: usize) {
        self.image_index = Some(index);
    }

    /// Return the engine-internal slot index, if set.
    pub fn index(&self) -> Option<usize> {
        self.image_index
    }

    pub fn clear(&mut self) {
        self.image_index = None;
        self.image_path = None;
    }

    pub fn set_tint(&mut self, color: uix_graphics::Color) {
        self.tint_color = Some(color);
    }

    pub fn tint(&self) -> Option<uix_graphics::Color> {
        self.tint_color
    }
}

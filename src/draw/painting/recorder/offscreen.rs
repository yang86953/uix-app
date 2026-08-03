//! 离屏 Picture 池 — recorder 子模块。
//!
//! 保留 API-neutral 编码流（预算内）或惰性物化后的图片载荷；内存超预算时
//! 物化并回收编码器。由 [`super::CommandRecorder`] 拥有。

use crate::draw::geometry::types::ImageHandle;
use crate::draw::painting::{FrameEncoder, FrameImage};

use super::canvas::FrameRecordingCanvas;

pub(crate) enum RecordedPicturePayload {
    Encoder(FrameEncoder),
    Image(FrameImage),
}

#[derive(Clone, Copy)]
pub(crate) struct ActiveOffscreen {
    pub(super) handle: ImageHandle,
    pub(super) failed: bool,
}

pub(crate) struct RecordedPicture {
    pub(super) canvas: FrameRecordingCanvas,
    pub(super) committed: Option<RecordedPicturePayload>,
}

impl RecordedPicture {
    pub(super) fn new(width: i32, height: i32) -> Self {
        Self {
            canvas: FrameRecordingCanvas::new(width, height),
            committed: None,
        }
    }

    pub(super) fn commit(&mut self, encoder: FrameEncoder) {
        let pixel_budget = (encoder.width() as usize)
            .saturating_mul(encoder.height() as usize)
            .saturating_mul(std::mem::size_of::<u32>());
        self.committed = Some(if encoder.retained_memory_usage() <= pixel_budget {
            RecordedPicturePayload::Encoder(encoder)
        } else {
            RecordedPicturePayload::Image(encoder.render_image())
        });
    }

    pub(super) fn copy_pixels(&self) -> Option<(Vec<u32>, i32)> {
        match self.committed.as_ref()? {
            RecordedPicturePayload::Encoder(encoder) => {
                let image = encoder.render_image();
                Some((image.pixels().to_vec(), image.width()))
            }
            RecordedPicturePayload::Image(image) => Some((image.pixels().to_vec(), image.width())),
        }
    }

    pub(super) fn materialized_image(&mut self) -> Option<FrameImage> {
        let payload = self.committed.take()?;
        let image = match payload {
            RecordedPicturePayload::Encoder(encoder) => encoder.render_image(),
            RecordedPicturePayload::Image(image) => image,
        };
        self.committed = Some(RecordedPicturePayload::Image(image.clone()));
        Some(image)
    }

    pub(super) fn memory_usage(&self) -> usize {
        let working = self.canvas.retained_memory_usage();
        working.saturating_add(match &self.committed {
            Some(RecordedPicturePayload::Encoder(encoder)) => encoder.retained_memory_usage(),
            Some(RecordedPicturePayload::Image(image)) => image
                .pixels()
                .len()
                .saturating_mul(std::mem::size_of::<u32>()),
            None => 0,
        })
    }
}

#[derive(Default)]
pub(crate) struct RecordedPicturePool {
    slots: Vec<Option<RecordedPicture>>,
    free_ids: Vec<u32>,
    next_id: u32,
}

impl RecordedPicturePool {
    pub(super) fn new() -> Self {
        Self::default()
    }

    pub(super) fn clear(&mut self) {
        self.slots.clear();
        self.free_ids.clear();
        self.next_id = 0;
    }

    pub(super) fn create(&mut self, width: i32, height: i32) -> Option<ImageHandle> {
        let (width, height) = FrameRecordingCanvas::prepare_resize(width, height).ok()?;
        let id = self.free_ids.pop().unwrap_or_else(|| {
            let id = self.next_id;
            self.next_id = self.next_id.saturating_add(1);
            id
        });
        let index = id as usize;
        while self.slots.len() <= index {
            self.slots.push(None);
        }
        self.slots[index] = Some(RecordedPicture::new(width, height));
        Some(ImageHandle(id))
    }

    pub(super) fn destroy(&mut self, handle: ImageHandle) {
        let index = handle.0 as usize;
        if index < self.slots.len() && self.slots[index].take().is_some() {
            self.free_ids.push(handle.0);
        }
    }

    pub(super) fn get(&self, handle: &ImageHandle) -> Option<&RecordedPicture> {
        self.slots.get(handle.0 as usize)?.as_ref()
    }

    pub(super) fn get_mut(&mut self, handle: &ImageHandle) -> Option<&mut RecordedPicture> {
        self.slots.get_mut(handle.0 as usize)?.as_mut()
    }

    pub(super) fn compact(&mut self) {
        while self.slots.last().is_some_and(Option::is_none) {
            self.slots.pop();
        }
        self.free_ids.retain(|id| (*id as usize) < self.slots.len());
        self.next_id = self.slots.len() as u32;
    }

    pub(super) fn memory_usage(&self) -> usize {
        self.slots
            .iter()
            .filter_map(Option::as_ref)
            .map(RecordedPicture::memory_usage)
            .fold(0usize, usize::saturating_add)
    }
}

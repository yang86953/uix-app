//! 离屏 Picture 池 — recorder 子模块。
//!
//! 保留 API-neutral 编码流（预算内）或惰性物化后的图片载荷；内存超预算时
//! 物化并回收编码器。由 [`super::CommandRecorder`] 拥有。

// 引入 typed error，避免 recorder extent 失败退化为普通缺失。
use crate::core::Error;
use crate::draw::geometry::types::ImageHandle;
use crate::draw::painting::{FrameEncoder, FrameImage};

use super::canvas::FrameRecordingCanvas;

/// 已提交的 Picture 载荷：API 中立命令流（预算内）或物化图片。
pub(crate) enum RecordedPicturePayload {
    Encoder(FrameEncoder),
    Image(FrameImage),
}

/// 当前活动的离屏绘制会话（handle + 失败标记）。
#[derive(Clone, Copy)]
pub(crate) struct ActiveOffscreen {
    pub(super) handle: ImageHandle,
    pub(super) failed: bool,
}

/// 一个已录制的离屏 Picture：录制画布 + 已提交载荷。
pub(crate) struct RecordedPicture {
    pub(super) canvas: FrameRecordingCanvas,
    pub(super) committed: Option<RecordedPicturePayload>,
}

impl RecordedPicture {
    /// 创建指定尺寸的空 Picture 录制画布。
    pub(super) fn new(width: i32, height: i32) -> Self {
        Self {
            canvas: FrameRecordingCanvas::new(width, height),
            committed: None,
        }
    }

    /// 提交录制结果：保留命令流在内存预算内，否则物化为图片。
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

    /// 复制 Picture 的参考像素（测试 / 诊断）。
    pub(super) fn copy_pixels(&self) -> Option<(Vec<u32>, i32)> {
        match self.committed.as_ref()? {
            RecordedPicturePayload::Encoder(encoder) => {
                let image = encoder.render_image();
                Some((image.pixels().to_vec(), image.width()))
            }
            RecordedPicturePayload::Image(image) => Some((image.pixels().to_vec(), image.width())),
        }
    }

    /// 取出（并缓存）物化图片，供 blit 使用；命令流在此被惰性光栅化。
    pub(super) fn materialized_image(&mut self) -> Option<FrameImage> {
        let payload = self.committed.take()?;
        let image = match payload {
            RecordedPicturePayload::Encoder(encoder) => encoder.render_image(),
            RecordedPicturePayload::Image(image) => image,
        };
        self.committed = Some(RecordedPicturePayload::Image(image.clone()));
        Some(image)
    }

    /// 内存占用：工作画布与已提交载荷之和。
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

/// 录制器持有的离屏 Picture 池：槽位数组 + 空闲 id 复用。
#[derive(Default)]
pub(crate) struct RecordedPicturePool {
    slots: Vec<Option<RecordedPicture>>,
    free_ids: Vec<u32>,
    next_id: u32,
}

impl RecordedPicturePool {
    /// 创建空池。
    pub(super) fn new() -> Self {
        Self::default()
    }

    /// 清空全部槽位与空闲 id。
    pub(super) fn clear(&mut self) {
        self.slots.clear();
        self.free_ids.clear();
        self.next_id = 0;
    }

    // 以检查式结果创建 recorder Picture 槽位。
    pub(super) fn try_create(
        // 借用 recorder pool 的唯一可变 owner。
        &mut self,
        // 接收 Picture 的逻辑宽度。
        width: i32,
        // 接收 Picture 的逻辑高度。
        height: i32,
        // 区分正常无资源、成功 handle 与 typed extent failure。
    ) -> Result<Option<ImageHandle>, Error> {
        // 非正尺寸不创建隐式一像素 Picture。
        if width <= 0 || height <= 0 {
            // 保持场景层约定的正常无资源语义。
            return Ok(None);
        }
        // 保留 extent 溢出或资源预算失败的 typed 分类。
        let (width, height) = FrameRecordingCanvas::prepare_resize(width, height)?;
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
        // 只有槽位和 recorder 状态都建立后才发布 handle。
        Ok(Some(ImageHandle(id)))
    }

    /// 销毁槽位并把 id 归还空闲列表。
    pub(super) fn destroy(&mut self, handle: ImageHandle) {
        let index = handle.0 as usize;
        if index < self.slots.len() && self.slots[index].take().is_some() {
            self.free_ids.push(handle.0);
        }
    }

    /// 按 handle 取 Picture 引用。
    pub(super) fn get(&self, handle: &ImageHandle) -> Option<&RecordedPicture> {
        self.slots.get(handle.0 as usize)?.as_ref()
    }

    /// 按 handle 取 Picture 可变引用。
    pub(super) fn get_mut(&mut self, handle: &ImageHandle) -> Option<&mut RecordedPicture> {
        self.slots.get_mut(handle.0 as usize)?.as_mut()
    }

    /// 收缩尾部空槽并整理空闲 id / 自增计数器。
    pub(super) fn compact(&mut self) {
        while self.slots.last().is_some_and(Option::is_none) {
            self.slots.pop();
        }
        self.free_ids.retain(|id| (*id as usize) < self.slots.len());
        self.next_id = self.slots.len() as u32;
    }

    /// 全部存活 Picture 的内存占用总和。
    pub(super) fn memory_usage(&self) -> usize {
        self.slots
            .iter()
            .filter_map(Option::as_ref)
            .map(RecordedPicture::memory_usage)
            .fold(0usize, usize::saturating_add)
    }
}

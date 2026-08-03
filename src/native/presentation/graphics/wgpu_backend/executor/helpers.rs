//! 顶点缓冲与纹理辅助。

use super::*;

#[derive(Clone, Copy)]
pub(crate) enum LocalCoordinates {
    Pixels,
    Normalized,
    Centered,
}

impl LocalCoordinates {
    pub(super) fn values(self, rect: Rect) -> [[f32; 2]; 6] {
        match self {
            Self::Pixels => [
                [0.0, 0.0],
                [rect.w, 0.0],
                [rect.w, rect.h],
                [0.0, 0.0],
                [rect.w, rect.h],
                [0.0, rect.h],
            ],
            Self::Normalized => [
                [0.0, 0.0],
                [1.0, 0.0],
                [1.0, 1.0],
                [0.0, 0.0],
                [1.0, 1.0],
                [0.0, 1.0],
            ],
            Self::Centered => {
                let radius = rect.w * 0.5;
                [
                    [-radius, -radius],
                    [radius, -radius],
                    [radius, radius],
                    [-radius, -radius],
                    [radius, radius],
                    [-radius, radius],
                ]
            }
        }
    }
}

pub(super) fn create_vertex_buffer(device: &wgpu::Device, size: u64) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("uix-shared-vertex-buffer"),
        size,
        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}

pub(super) fn write_bgra_texture(
    queue: &wgpu::Queue,
    texture: &wgpu::Texture,
    width: u32,
    height: u32,
    pixels: &[u32],
) -> Result<()> {
    let unpadded = width.saturating_mul(4);
    let padded = align_up_u32(unpadded, COPY_BYTES_PER_ROW_ALIGNMENT);
    let source = bytemuck::cast_slice::<u32, u8>(pixels);
    if padded == unpadded {
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            source,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(unpadded),
                rows_per_image: Some(height),
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );
        return Ok(());
    }
    let mut padded_bytes = Vec::new();
    padded_bytes
        .try_reserve_exact((padded as usize).saturating_mul(height as usize))
        .map_err(|error| {
            Error::new(
                Errc::GraphicsOutOfMemory,
                format!("wgpu image blit row padding allocation failed: {error}"),
            )
        })?;
    for row in 0..height as usize {
        let start = row * unpadded as usize;
        padded_bytes.extend_from_slice(&source[start..start + unpadded as usize]);
        padded_bytes.resize(padded_bytes.len() + (padded - unpadded) as usize, 0);
    }
    queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        &padded_bytes,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(padded),
            rows_per_image: Some(height),
        },
        wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
    );
    Ok(())
}

pub(super) fn physical_scissor(
    extent: (u32, u32),
    viewport: (f32, f32),
    scissor: Option<(i32, i32, i32, i32)>,
) -> (u32, u32, u32, u32) {
    let Some((x, y, width, height)) = scissor else {
        return (0, 0, extent.0, extent.1);
    };
    let scale_x = extent.0 as f32 / viewport.0.max(1.0);
    let scale_y = extent.1 as f32 / viewport.1.max(1.0);
    let left = ((x as f32 * scale_x).floor() as i64).clamp(0, extent.0 as i64) as u32;
    let top = ((y as f32 * scale_y).floor() as i64).clamp(0, extent.1 as i64) as u32;
    let right =
        (((x + width) as f32 * scale_x).ceil() as i64).clamp(left as i64, extent.0 as i64) as u32;
    let bottom =
        (((y + height) as f32 * scale_y).ceil() as i64).clamp(top as i64, extent.1 as i64) as u32;
    (left, top, right - left, bottom - top)
}

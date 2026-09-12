//! S4 背景瓦片盒相对圆角掩码的 CPU 像素证明（反例修正版）。
//!
//! 掩码在背景盒逻辑坐标系求值：跨盒偏移（cover/auto 溢出瓦片）、跨中
//! 线大角（tl=80）、dpr 有效缩放、4096 钳制与 LRU 有界性。采样点 SDF
//! 预期来自独立几何计算（切线区域圆角），fixture 为全不透明 1200x800
//! PNG 经公开 `load_from_bytes` 同步解码。

use super::ImageService;
use crate::core::Rect;
use crate::draw::geometry::types::Radius;
use crate::draw::resources::image::BitmapHandle;

// 提取像素的 (a, r, g, b)；缓冲布局为 (a<<24)|(r<<16)|(g<<8)|b。
fn channels(pixel: u32) -> (u32, u32, u32, u32) {
    (
        (pixel >> 24) & 0xFF,
        (pixel >> 16) & 0xFF,
        (pixel >> 8) & 0xFF,
        pixel & 0xFF,
    )
}

fn opaque_service() -> (ImageService, BitmapHandle) {
    let bytes =
        std::fs::read("assets/images/acceptance-1461-before.png").expect("fixture PNG 应可读取");
    let service = ImageService::new();
    let handle = service
        .load_from_bytes(&bytes)
        .expect("fixture PNG 应可同步解码");
    (service, handle)
}

// 等值瓦片 + 盒相对 tl=80：跨中线大角必须在盒坐标正确裁剪。
#[test]
fn box_relative_tl80_clips_crossing_midline() {
    let (service, handle) = opaque_service();
    let tile = service
        .rounded_background_tile(
            handle,
            Rect::new(0.0, 0.0, 200.0, 100.0),
            Rect::new(0.0, 0.0, 200.0, 100.0),
            Radius {
                tl: 80.0,
                tr: 0.0,
                br: 0.0,
                bl: 0.0,
            },
            1.0,
        )
        .expect("盒相对瓦片应生成");
    service.with_slot(tile, |slot| {
        let pixels = slot.pixels();
        let width = slot.width() as usize;
        // 独立几何：tl 圆心 (80,80)；(15.5,20.5) 距 87.7、(2.5,55.5) 距
        // 81.3 均在圆外（反例：旧 half_min 钳制把它们错误保留为不透明）。
        for (x, y) in [(15usize, 20usize), (2, 55), (2, 2), (15, 2)] {
            let (a, _, _, _) = channels(pixels[y * width + x]);
            assert_eq!(a, 0, "({x},{y}) 大角圆外必须透明");
        }
        // 切线右侧直边与底部直边保持不透明。
        for (x, y) in [(100usize, 50usize), (2, 90)] {
            let (a, _, _, _) = channels(pixels[y * width + x]);
            assert_eq!(a, 255, "({x},{y}) 直边形内必须不透明");
        }
        Some(())
    });
}

// 跨盒偏移瓦片（auto 溢出几何：tile(-500,-350,1200,800)）在盒坐标裁剪。
#[test]
fn offset_oversized_tile_clips_at_box() {
    let (service, handle) = opaque_service();
    let tile = service
        .rounded_background_tile(
            handle,
            Rect::new(-500.0, -350.0, 1200.0, 800.0),
            Rect::new(0.0, 0.0, 200.0, 100.0),
            Radius::uniform(30.0),
            1.0,
        )
        .expect("跨盒瓦片应生成");
    service.with_slot(tile, |slot| {
        let pixels = slot.pixels();
        let width = slot.width() as usize;
        // 设备像素 = 逻辑坐标 + (500,350)：盒内 (2,2)→瓦片像素 (502,352)。
        for (box_x, box_y) in [(2usize, 2usize), (15, 2)] {
            let (px, py) = (box_x + 500, box_y + 350);
            let (a, _, _, _) = channels(pixels[py * width + px]);
            assert_eq!(a, 0, "盒内 ({box_x},{box_y}) 圆角外必须透明");
        }
        let (a, _, _, _) = channels(pixels[(20 + 350) * width + (15 + 500)]);
        assert_eq!(a, 255, "盒内 (15,20) 形内必须不透明");
        Some(())
    });
}

// dpr=2 有效缩放：掩码在逻辑空间求值，设备像素按有效比例采样。
#[test]
fn dpr_two_keeps_logical_corner_geometry() {
    let (service, handle) = opaque_service();
    // 盒 100x50、R20（相邻和 40≤50 不归一）。
    let tile = service
        .rounded_background_tile(
            handle,
            Rect::new(0.0, 0.0, 100.0, 50.0),
            Rect::new(0.0, 0.0, 100.0, 50.0),
            Radius::uniform(20.0),
            2.0,
        )
        .expect("dpr=2 瓦片应生成");
    service.with_slot(tile, |slot| {
        let pixels = slot.pixels();
        let width = slot.width() as usize;
        assert_eq!(slot.width(), 200, "设备宽度应为逻辑两倍");
        // 设备像素 (5,5) 中心逻辑 (2.75,2.75)：距 R20 圆心 (20,20) 为
        // 24.4 > 20 → 透明。
        let (a, _, _, _) = channels(pixels[5 * width + 5]);
        assert_eq!(a, 0, "dpr=2 下逻辑圆外必须透明");
        // 设备像素 (100,25) 中心逻辑 (50.25,12.75)：形内。
        let (a, _, _, _) = channels(pixels[25 * width + 100]);
        assert_eq!(a, 255, "dpr=2 下逻辑形内必须不透明");
        Some(())
    });
}

// 4096 钳制后有效缩放仍保持逻辑圆角几何（不静默改变圆角）。
#[test]
fn clamp_4096_keeps_logical_geometry() {
    let (service, handle) = opaque_service();
    // 盒 6000x3000、R1000；瓦片钳制到 4096x2048，有效缩放 ≈0.6827。
    let tile = service
        .rounded_background_tile(
            handle,
            Rect::new(0.0, 0.0, 6000.0, 3000.0),
            Rect::new(0.0, 0.0, 6000.0, 3000.0),
            Radius {
                tl: 1000.0,
                tr: 0.0,
                br: 0.0,
                bl: 0.0,
            },
            1.0,
        )
        .expect("钳制瓦片应生成");
    service.with_slot(tile, |slot| {
        assert_eq!(slot.width(), 4096);
        let pixels = slot.pixels();
        let width = slot.width() as usize;
        let scale: f32 = 6000.0 / 4096.0;
        // 逻辑 (100,100)：距圆心 (1000,1000) 为 1272 > 1000 → 圆外。
        let px = (100.0 / scale).floor() as usize;
        let (a, _, _, _) = channels(pixels[px * width + px]);
        assert_eq!(a, 0, "钳制缩放下逻辑圆外必须透明");
        // 逻辑 (600,600)：距圆心 565 < 1000 → 圆内。
        let px = (600.0 / scale).floor() as usize;
        let (a, _, _, _) = channels(pixels[px * width + px]);
        assert_eq!(a, 255, "钳制缩放下逻辑圆内必须不透明");
        Some(())
    });
}

// LRU 有界：插入超过上限的派生瓦片后缓存条目数不超过上限。
#[test]
fn derived_tile_cache_is_bounded() {
    let (service, handle) = opaque_service();
    let mut offsets = 0.0f32;
    for _ in 0..(service.rounded_background_cache_cap() + 8) {
        let tile = Rect::new(offsets, 0.0, 60.0, 40.0);
        service
            .rounded_background_tile(
                handle,
                tile,
                Rect::new(0.0, 0.0, 200.0, 100.0),
                Radius::uniform(12.0),
                1.0,
            )
            .expect("派生瓦片应生成");
        offsets += 0.5;
    }
    assert!(
        service.rounded_background_cache_len() <= service.rounded_background_cache_cap(),
        "派生瓦片缓存必须有界（LRU 淘汰）"
    );
}

    // 引入共享引用计数数组，构造最小的轮廓输入。
    use std::sync::Arc;

    // 引入被测的透明 RHI 句柄。
    use crate::native::present::rhi::TextureHandle;

    // 引入父模块的 atlas page 和 renderer 私有 helper。
    use super::{
        MSDF_ATLAS_LIMIT_BYTES, MSDF_ATLAS_MAX_PAGES, MSDF_ATLAS_PAGE_SIZE, MsdfAtlasPage,
        RhiRenderer,
    };

    // 验证固定 page 数量不会突破跨帧 atlas 的硬预算。
    #[test]
    fn msdf_atlas_budget_is_fixed() {
        // 计算单页 RGBA8 的物理字节数。
        let page_bytes = MSDF_ATLAS_PAGE_SIZE as usize * MSDF_ATLAS_PAGE_SIZE as usize * 4;
        // 四页恰好覆盖配置的 16 MiB 上限。
        assert_eq!(MSDF_ATLAS_MAX_PAGES * page_bytes, MSDF_ATLAS_LIMIT_BYTES);
    }

    // 验证 gutter 会复制四条边界，阻断 atlas 相邻字形的线性采样串色。
    #[test]
    fn msdf_padding_copies_edge_pixels() {
        // 准备一个可逐字节检查的 2x2 RGBA 输入。
        let payload = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16];
        // 执行单像素 gutter 的 padding lowering。
        let padded = match RhiRenderer::pad_msdf_payload(&payload, 2, 2) {
            // 合法输入必须成功生成 padded payload。
            Ok(padded) => padded,
            // padding 失败说明低层校验与测试输入矛盾。
            Err(error) => panic!("valid padding: {error:?}"),
        };
        // 顶部 gutter、两行内容和底部 gutter 都应保留完整 4x4 RGBA 数据。
        assert_eq!(
            padded,
            vec![
                1, 2, 3, 4, 1, 2, 3, 4, 5, 6, 7, 8, 5, 6, 7, 8, 1, 2, 3, 4, 1, 2, 3, 4, 5, 6, 7, 8,
                5, 6, 7, 8, 9, 10, 11, 12, 9, 10, 11, 12, 13, 14, 15, 16, 13, 14, 15, 16, 9, 10,
                11, 12, 9, 10, 11, 12, 13, 14, 15, 16, 13, 14, 15, 16,
            ]
        );
    }

    // 验证 shelf allocator 的换行不会覆盖前一行，也不会推进无效 cursor。
    #[test]
    fn msdf_shelf_allocator_preserves_non_overlapping_slots() {
        // 创建一个空的测试 atlas page。
        let mut page = MsdfAtlasPage {
            texture: TextureHandle::from_raw(1),
            cursor_x: 0,
            cursor_y: 0,
            row_height: 0,
        };
        // 第一项必须从 page 左上角开始。
        assert_eq!(RhiRenderer::pack_atlas_slot(&mut page, 4, 3), Some((0, 0)));
        // 第二项沿同一 shelf 横向排布。
        assert_eq!(RhiRenderer::pack_atlas_slot(&mut page, 5, 2), Some((4, 0)));
        // 下一项放不下时必须从下一 shelf 的左侧开始。
        assert_eq!(
            RhiRenderer::pack_atlas_slot(&mut page, MSDF_ATLAS_PAGE_SIZE - 8, 4),
            Some((0, 3))
        );
        // 超出 page 高度时返回 None，并保持 cursor 不发生回绕。
        page.cursor_y = MSDF_ATLAS_PAGE_SIZE - 2;
        page.cursor_x = 0;
        page.row_height = 2;
        assert_eq!(RhiRenderer::pack_atlas_slot(&mut page, 8, 3), None);
        assert_eq!(page.cursor_y, MSDF_ATLAS_PAGE_SIZE - 2);
    }

    // 验证 MSDF quad 的 affine 四角和 atlas UV 会原样进入统一 float8 ABI。
    #[test]
    fn msdf_vertices_preserve_affine_corners_and_uv() {
        // 准备最小的 MSDF quad 几何与颜色。
        let quad = super::RhiMsdfQuad {
            x: 10.0,
            y: 20.0,
            w: 8.0,
            h: 6.0,
            corners: [[10.0, 20.0], [18.0, 21.0], [17.0, 27.0], [9.0, 26.0]],
            range: 4.0,
            edges: Arc::from([1.0, 1.0, 7.0, 1.0]),
            pixel_w: 8,
            pixel_h: 6,
            rgba: [0.1, 0.2, 0.3, 0.4],
            scissor: None,
        };
        // 生成指定 atlas placement 的两个三角形顶点。
        let vertices = RhiRenderer::msdf_quad_vertices_with_uv(&quad, [0.2, 0.3, 0.8, 0.9]);
        // 首个顶点必须保留左上 affine corner 与左上 UV。
        assert_eq!(&vertices[0..4], &[10.0, 20.0, 0.2, 0.3]);
        // 第二个三角形的末顶点必须保留左下 affine corner 与左下 UV。
        assert_eq!(&vertices[40..44], &[9.0, 26.0, 0.2, 0.9]);
        // 每个顶点都必须携带完全相同的 tint。
        assert_eq!(&vertices[4..8], &[0.1, 0.2, 0.3, 0.4]);
    }

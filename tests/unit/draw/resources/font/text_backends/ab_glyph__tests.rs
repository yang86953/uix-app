// 引入被测后端，验证卸载边界而不依赖应用初始化。
use super::AbGlyphBackend;
// 引入字体后端 trait，使测试可以调用加载、卸载和内存统计接口。
use crate::draw::TextBackend;
// 引入字体布局选项与对齐枚举，验证 shaping 到光栅的完整链路。
use crate::draw::{HAlign, VAlign};
// 引入后端内部布局选项类型。
use super::TextLayoutOptions;
// 引入共享所有权、线程同步与只读匿名映射，覆盖槽位并发和两种字节 owner。
use std::sync::{Arc, Barrier};

// 构造所有生命周期测试共用的有限宽度文本约束。
fn lifecycle_layout_options() -> TextLayoutOptions {
    TextLayoutOptions {
        max_width: 160.0,
        max_height: 0.0,
        line_height: 24.0,
        word_wrap: true,
        h_align: HAlign::Left,
        v_align: VAlign::Top,
        font_size: 17.0,
    }
}

// 把固定测试字体复制进匿名映射，再冻结为只读 mmap，避免文件并发修改风险。
fn readonly_map(data: &[u8]) -> memmap2::Mmap {
    let mut mapped = memmap2::MmapMut::map_anon(data.len()).expect("应能建立匿名字体映射");
    mapped.copy_from_slice(data);
    mapped.make_read_only().expect("匿名字体映射应能转为只读")
}

#[test]
fn unload_releases_owned_font_data() {
    // 使用仓库内稳定的 Lucide 字体作为最小可解析输入。
    let data = include_bytes!("../../../../../../assets/fonts/lucide.ttf");
    // 创建独立后端，避免测试之间共享字体槽位。
    let mut backend = AbGlyphBackend::new();
    // 加载字体并记录稳定句柄。
    let handle = backend
        .load_font(data)
        .expect("Lucide font must load in the ab_glyph backend");
    // 加载期应建立与同一槽位字节绑定的 OpenType 面。
    assert!(backend.fonts[handle.0 as usize].shaping_face.is_some());
    // 加载后底层字体数据必须计入后端内存统计。
    assert!(backend.memory_usage() > 0);
    // 卸载字体应释放底层数据而不是只使句柄失效。
    backend.unload_font(&handle);
    // 卸载必须先结束 shaping 面借用，不能留下失效字体表视图。
    assert!(backend.fonts[handle.0 as usize].shaping_face.is_none());
    // 卸载后句柄不可用，避免继续访问已释放的借用。
    assert!(!backend.is_valid(&handle));
    // 卸载后不应残留字体数据占用。
    assert_eq!(backend.memory_usage(), 0);
}

// 验证任一字节 owner 解析失败都不会发布半构造的自引用槽位。
#[test]
fn failed_parsing_never_publishes_font_slot() {
    let invalid = b"not an OpenType font";
    let mut backend = AbGlyphBackend::new();

    assert!(backend.load_font(invalid).is_err());
    assert!(backend.load_font_owned(invalid.to_vec()).is_err());
    assert!(
        backend
            .load_font_shared(Arc::<[u8]>::from(invalid.as_slice()))
            .is_err()
    );
    assert!(backend.load_font_mapped(readonly_map(invalid)).is_err());
    assert!(backend.fonts.is_empty());
    assert_eq!(backend.memory_usage(), 0);
}

// 验证共享 Arc 的最后一个强 owner 在显式卸载后立即释放。
#[test]
fn shared_font_owner_drops_after_unload() {
    let data =
        Arc::<[u8]>::from(include_bytes!("../../../../../../assets/fonts/lucide.ttf").as_slice());
    let weak = Arc::downgrade(&data);
    let mut backend = AbGlyphBackend::new();
    let handle = backend
        .load_font_shared(Arc::clone(&data))
        .expect("共享 Lucide 字体应能加载");
    drop(data);
    assert!(weak.upgrade().is_some());

    let layout = backend.layout_text(&handle, "A B C", &lifecycle_layout_options());
    assert!(!layout.glyphs.is_empty());
    backend.unload_font(&handle);

    assert!(weak.upgrade().is_none(), "卸载后不得残留字体字节强引用");
    assert!(!backend.is_valid(&handle));
    assert!(backend.fonts[handle.0 as usize].font.is_none());
    assert!(backend.fonts[handle.0 as usize].shaping_face.is_none());
    assert!(backend.fonts[handle.0 as usize]._data.is_none());
    assert_eq!(backend.memory_usage(), 0);
}

// 验证未显式卸载时，后端整体 Drop 也按相同顺序释放共享字体 owner。
#[test]
fn backend_drop_releases_active_shared_font_owner() {
    let data =
        Arc::<[u8]>::from(include_bytes!("../../../../../../assets/fonts/lucide.ttf").as_slice());
    let weak = Arc::downgrade(&data);
    {
        let mut backend = AbGlyphBackend::new();
        backend
            .load_font_shared(Arc::clone(&data))
            .expect("共享 Lucide 字体应能加载");
        drop(data);
        assert!(weak.upgrade().is_some());
    }
    assert!(
        weak.upgrade().is_none(),
        "后端 Drop 后不得残留字体字节强引用"
    );
}

// 验证仓库实际的追加新槽等价替换路径可反复加载、塑形和卸载。
#[test]
fn repeated_load_shape_unload_releases_shared_and_mapped_slots() {
    let bytes = include_bytes!("../../../../../../assets/fonts/lucide.ttf");
    let shared = Arc::<[u8]>::from(bytes.as_slice());
    let options = lifecycle_layout_options();
    let mut backend = AbGlyphBackend::new();

    for cycle in 0..64_u32 {
        let handle = backend
            .load_font_shared(Arc::clone(&shared))
            .expect("每轮共享字体都应加载成功");
        assert_eq!(handle.0, cycle);
        assert!(
            !backend
                .layout_text(&handle, "A B C", &options)
                .glyphs
                .is_empty()
        );
        backend.unload_font(&handle);
        assert!(!backend.is_valid(&handle));
        assert!(
            backend
                .layout_text(&handle, "A", &options)
                .glyphs
                .is_empty()
        );
        assert_eq!(backend.memory_usage(), 0);
    }

    for _ in 0..8 {
        let handle = backend
            .load_font_mapped(readonly_map(bytes))
            .expect("每轮只读 mmap 字体都应加载成功");
        assert!(
            !backend
                .layout_text(&handle, "A B C", &options)
                .glyphs
                .is_empty()
        );
        backend.unload_font(&handle);
        assert!(!backend.is_valid(&handle));
        assert_eq!(backend.memory_usage(), 0);
    }
}

// 验证缓存字体面满足后端公开 Send + Sync 契约，并允许并发只读塑形。
#[test]
fn cached_shaping_face_supports_concurrent_readers() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<AbGlyphBackend>();

    let data = Arc::<[u8]>::from(
        include_bytes!("../../../../../../assets/fonts/NotoSansCJKsc-Regular.otf").as_slice(),
    );
    let mut backend = AbGlyphBackend::new();
    let handle = backend
        .load_font_shared(data)
        .expect("固定 Noto CJK 字体应能加载");
    let backend = Arc::new(backend);
    let readers = 4;
    let barrier = Arc::new(Barrier::new(readers));
    let iterations = if cfg!(miri) { 2 } else { 128 };
    let mut threads = Vec::with_capacity(readers);

    for _ in 0..readers {
        let backend = Arc::clone(&backend);
        let barrier = Arc::clone(&barrier);
        threads.push(std::thread::spawn(move || {
            barrier.wait();
            let options = lifecycle_layout_options();
            let text = "中文 English العربية Résumé e\u{301}";
            for _ in 0..iterations {
                let layout = backend.layout_text(&handle, text, &options);
                assert!(!layout.glyphs.is_empty());
                assert_eq!(
                    layout.glyphs.last().map(|glyph| glyph.char_end),
                    Some(text.chars().count())
                );
            }
        }));
    }
    for thread in threads {
        thread.join().expect("并发只读塑形线程不应失败");
    }

    let mut backend = Arc::try_unwrap(backend).expect("并发读者结束后应只剩唯一 owner");
    backend.unload_font(&handle);
    assert_eq!(backend.memory_usage(), 0);
}

// 验证缓存的 OpenType 面保持中英文换行、cluster 与字形输出完全稳定。
#[test]
fn cached_shaping_face_preserves_multiscript_layout() {
    // 使用仓库固定 Noto CJK 字体，避免依赖系统字体与平台回退。
    let data = include_bytes!("../../../../../../assets/fonts/NotoSansCJKsc-Regular.otf");
    let mut backend = AbGlyphBackend::new();
    let handle = backend
        .load_font(data)
        .expect("Noto CJK 字体应建立 ab_glyph 与 rustybuzz 共用槽位");
    let options = lifecycle_layout_options();
    let text = "中文动态编辑 English words Résumé e\u{301}";

    let first = backend.layout_text(&handle, text, &options);
    let repeated = backend.layout_text(&handle, text, &options);
    assert!(first.lines.len() >= 2, "有限宽度应产生真实换行");
    assert_eq!(first.glyphs.len(), repeated.glyphs.len());
    assert_eq!(first.lines.len(), repeated.lines.len());
    assert_eq!(first.width.to_bits(), repeated.width.to_bits());
    assert_eq!(first.height.to_bits(), repeated.height.to_bits());
    for (left, right) in first.glyphs.iter().zip(&repeated.glyphs) {
        assert_eq!(left.glyph_id, right.glyph_id);
        assert_eq!(
            (left.char_index, left.char_end),
            (right.char_index, right.char_end)
        );
        assert_eq!(
            (left.x.to_bits(), left.y.to_bits()),
            (right.x.to_bits(), right.y.to_bits())
        );
        assert_eq!(left.font, handle);
        assert!(left.char_end > left.char_index);
    }
    assert!(first.lines.iter().all(|line| line.glyph_count > 0));
    assert_eq!(
        first.lines.last().map(|line| line.end_char),
        Some(text.chars().count())
    );

    // shaping 输出字形仍必须由同一槽位的光栅路径消费。
    assert!(first.glyphs.iter().any(|glyph| {
        let raster = backend.rasterize_glyph(&handle, glyph.glyph_id, options.font_size);
        !raster.coverage.is_empty() || raster.outline_mesh.is_some()
    }));
}

// 验证 rustybuzz glyph id 可由既有 ab_glyph 光栅路径直接消费。
#[cfg(target_os = "windows")]
#[test]
fn shaped_glyph_ids_rasterize_with_existing_backend() {
    // 读取包含阿拉伯 GSUB/GPOS 的标准 Windows 字体。
    let data = std::fs::read(r"C:\Windows\Fonts\segoeui.ttf")
        // 缺少标准字体时明确暴露环境不满足 Windows 目标矩阵。
        .expect("应能读取 Segoe UI 复杂脚本测试字体");
    // 创建独立后端，隔离其他字体槽位。
    let mut backend = AbGlyphBackend::new();
    // 通过正式加载入口同时建立 ab_glyph 与 rustybuzz 所需数据。
    let handle = backend
        // 加载完整字体文件。
        .load_font(&data)
        // 解析失败时明确指出字体后端契约。
        .expect("Segoe UI 应能由字体后端加载");
    // 构造不换行的标准 shaping 约束。
    let options = TextLayoutOptions {
        // 使用足够大的有限宽度。
        max_width: 4096.0,
        // 高度由文本自身决定。
        max_height: 0.0,
        // 使用稳定行高。
        line_height: 30.0,
        // 本测试不触发自动换行。
        word_wrap: false,
        // 使用左对齐观察原始视觉字形。
        h_align: HAlign::Left,
        // 使用顶部对齐观察原始基线。
        v_align: VAlign::Top,
        // 使用稳定测试字号。
        font_size: 20.0,
        // 结束布局选项构造。
    };
    // 通过正式 TextBackend 接口执行阿拉伯 shaping。
    let layout = backend.layout_text(&handle, "سلام", &options);
    // shaping 必须产生可供光栅化的字形。
    assert!(!layout.glyphs.is_empty());
    // 至少一个真实字形必须由既有轮廓光栅路径成功解析。
    assert!(
        layout
            // 遍历 shaping 输出字形。
            .glyphs
            // 尝试通过同一字体句柄光栅化。
            .iter()
            // 要求至少一个字形产生非空像素或轮廓网格。
            .any(|glyph| {
                // 执行既有统一字形光栅入口。
                let raster = backend.rasterize_glyph(&handle, glyph.glyph_id, options.font_size);
                // 面积覆盖或轮廓网格任一存在即证明编号兼容。
                !raster.coverage.is_empty() || raster.outline_mesh.is_some()
            })
    );
    // 结束 shaping 到光栅兼容测试。
}

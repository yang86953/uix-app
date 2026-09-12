//! `draw/resources/font/service/font_layout.rs` 的 cfg(test) 单元测试体；模块层级不变，经 #[path] 引用，不进发布包。

use super::*;

#[test]
fn layout_counter_counts_only_actual_shaping() {
    let mut service = FontService::new();
    let font = service
        .load_font(include_bytes!("../../../../../assets/fonts/lucide.ttf"))
        .expect("test font loads");
    let opts = TextLayoutOptions {
        max_width: 0.0,
        max_height: 0.0,
        line_height: 0.0,
        word_wrap: false,
        h_align: HAlign::Left,
        v_align: VAlign::Top,
        font_size: 14.0,
    };
    // 计数器是进程级全局，lib 测试并行时其他字体用例会叠加噪声：
    // 冷布局断言下界，命中路径的断言窗口缩到单次调用，保持精确。
    take_text_layout_calls();
    let first = service.layout_text_shared(&font, "steady", &opts);
    assert!(take_text_layout_calls() >= 1);
    // 缓存命中的重复布局复用 Arc 且不产生任何 shaping。
    take_text_layout_calls();
    let second = service.layout_text_shared(&font, "steady", &opts);
    assert_eq!(take_text_layout_calls(), 0);
    assert!(Arc::ptr_eq(&first, &second));
    // 空文本走共享空布局，同样不计入 shaping。
    take_text_layout_calls();
    let _ = service.layout_text_shared(&font, "", &opts);
    assert_eq!(take_text_layout_calls(), 0);
}

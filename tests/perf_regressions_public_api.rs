//! 性能修复的行为回归：shaping 源索引、UAX #14 断行、动态标签重绑定与布局缓存预算。
//! 只断言公开 API 的实际行为，不加入易抖动的耗时门槛。
#![cfg(feature = "test-harness")]

use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};

use uix_app::draw::{FontHandle, FontService, HAlign, VAlign};
use uix_app::draw::resources::font::text_backend::{PositionedGlyph, TextLayoutOptions};
use uix_app::prelude::*;
use uix_app::ui::test_harness::TestApp;

const FONT_PATH: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/fonts/uix-test-body.ttf"
);

fn fonts() -> (FontService, FontHandle) {
    let mut service = FontService::new();
    let font = service
        .load_font(&std::fs::read(FONT_PATH).expect("测试字体应存在"))
        .expect("字体应加载成功");
    (service, font)
}

fn opts(word_wrap: bool, max_width: f32) -> TextLayoutOptions {
    TextLayoutOptions {
        max_width,
        max_height: 0.0,
        font_size: 14.0,
        line_height: 21.0,
        word_wrap,
        h_align: HAlign::Left,
        v_align: VAlign::Top,
    }
}

// 断言字形 cluster 区间合法：字符下标递进且不越过全文长度。
fn assert_valid_glyph_ranges(glyphs: &[PositionedGlyph], total_chars: usize) {
    for glyph in glyphs {
        assert!(
            glyph.char_index < glyph.char_end,
            "cluster 区间必须至少覆盖一个字符: {}..{}",
            glyph.char_index,
            glyph.char_end
        );
        assert!(
            glyph.char_end <= total_chars,
            "cluster 终点 {} 不得越过全文长度 {}",
            glyph.char_end,
            total_chars
        );
    }
}

// 断言全部 cluster 区间恰好铺满源文本（合并后无空洞、无越界）。
fn assert_glyph_ranges_cover_text(glyphs: &[PositionedGlyph], total_chars: usize) {
    let mut ranges: Vec<(usize, usize)> = glyphs
        .iter()
        .map(|glyph| (glyph.char_index, glyph.char_end))
        .collect();
    ranges.sort_unstable();
    let mut covered = 0usize;
    for (start, end) in ranges {
        assert!(start <= covered, "cluster 区间不得与已覆盖区域重叠");
        assert_eq!(start, covered, "cluster 区间必须连续铺满源文本");
        covered = end;
    }
    assert_eq!(covered, total_chars, "cluster 区间必须覆盖完整文本");
}

// ── 修复 1：shaping 源索引映射 ────────────────────────────────

#[test]
fn shaping_cluster_ranges_map_every_source_script_exactly() {
    let (service, font) = fonts();
    let cases: &[(&str, String)] = &[
        ("ascii", "abcdefghij".repeat(500)),
        ("cjk", "中文测试字符".repeat(500)),
        ("nbsp", "a\u{a0}".repeat(500)),
        ("arabic_rtl", "مرحبا".repeat(400)),
        ("combining", "e\u{301}".repeat(300)),
        (
            "mixed",
            "abc中文مرحبا123中a\u{301}文".repeat(100),
        ),
    ];
    for (name, text) in cases {
        let total = text.chars().count();
        let layout = service.layout_text_shared(&font, text, &opts(false, 0.0));
        assert!(!layout.glyphs.is_empty(), "{name} 应产生字形");
        assert_valid_glyph_ranges(&layout.glyphs, total);
        assert_glyph_ranges_cover_text(&layout.glyphs, total);
        // 不换行时单行覆盖全文逻辑区间。
        assert_eq!(layout.lines.len(), 1, "{name} 不换行应只有一行");
        assert_eq!(layout.lines[0].start_char, 0);
        assert_eq!(layout.lines[0].end_char, total);
    }
}

#[test]
fn shaping_monotonic_scripts_keep_one_char_per_cluster() {
    let (service, font) = fonts();
    for (name, text) in [("ascii", "a".repeat(2_000)), ("cjk", "中".repeat(2_000))] {
        let layout = service.layout_text_shared(&font, &text, &opts(false, 0.0));
        // 无组合与连字的脚本是 1 字符 = 1 cluster 的强回归：直接验证映射表。
        for (index, glyph) in layout.glyphs.iter().enumerate() {
            assert_eq!(glyph.char_index, index, "{name} 字形 {index} 的逻辑起点");
            assert_eq!(glyph.char_end, index + 1, "{name} 字形 {index} 的逻辑终点");
        }
    }
}

#[test]
fn wrapped_bidi_and_mixed_text_lines_cover_all_source_chars() {
    let (service, font) = fonts();
    let text = "abc中文مرحبا123中文".repeat(300);
    let total = text.chars().count();
    let layout = service.layout_text_shared(&font, &text, &opts(true, 220.0));
    assert!(layout.lines.len() > 1, "窄容器应形成多行");
    assert_valid_glyph_ranges(&layout.glyphs, total);
    // 每行字形区间连续且铺满整个字形数组。
    let mut cursor = 0usize;
    for line in &layout.lines {
        assert_eq!(line.glyph_start, cursor, "行字形起点必须连续");
        cursor += line.glyph_count;
        assert!(line.end_char > line.start_char, "行逻辑区间非空");
        assert!(line.end_char <= total);
    }
    assert_eq!(cursor, layout.glyphs.len(), "全部字形归属某一行");
}

// ── 修复 2：不可断行文本的断点搜索 ────────────────────────────

#[test]
fn nbsp_runs_stay_single_line_even_when_overwide() {
    let (service, font) = fonts();
    let pairs = 8_000usize;
    let text = "a\u{a0}".repeat(pairs);
    let total = text.chars().count();
    assert_eq!(total, pairs * 2);
    let layout = service.layout_text_shared(&font, &text, &opts(true, 200.0));
    assert_eq!(layout.lines.len(), 1, "NBSP 不提供任何合法断点");
    assert!(layout.width > 200.0, "超宽内容允许溢出而不是强行拆行");
    assert_eq!(layout.lines[0].end_char, total);
}

#[test]
fn oversized_alphanumeric_word_emergency_wraps_at_cluster_bounds() {
    let (service, font) = fonts();
    let text = "abcdefghij".repeat(1_000);
    let layout = service.layout_text_shared(&font, &text, &opts(true, 200.0));
    assert!(layout.lines.len() > 1, "超长字母数字词必须紧急折行");
    let mut cursor = 0usize;
    for line in &layout.lines {
        assert_eq!(line.glyph_start, cursor);
        cursor += line.glyph_count;
    }
    assert_eq!(cursor, layout.glyphs.len());
    assert_glyph_ranges_cover_text(&layout.glyphs, 10_000);
}

#[test]
fn word_wrapped_spaces_text_keeps_line_and_glyph_partition() {
    let (service, font) = fonts();
    let text = "an ordinary sentence with spaces ".repeat(300);
    let total = text.chars().count();
    let layout = service.layout_text_shared(&font, &text, &opts(true, 200.0));
    assert!(layout.lines.len() > 1);
    let mut cursor = 0usize;
    let mut chars = 0usize;
    for line in &layout.lines {
        assert_eq!(line.glyph_start, cursor);
        cursor += line.glyph_count;
        assert_eq!(line.start_char, chars, "行逻辑区间必须连续铺满源文本");
        chars = line.end_char;
    }
    assert_eq!(cursor, layout.glyphs.len());
    assert_eq!(chars, total);
}

#[test]
fn mixed_breakable_and_unbreakable_runs_wrap_correctly() {
    let (service, font) = fonts();
    // NBSP 粘连的词不可拆，普通空格边界可拆：两种边界混合时仍应正确换行。
    let glued_words = "ab\u{a0}cd\u{a0}ef gh\u{a0}ij kl\u{a0}mn ";
    let text = glued_words.repeat(400);
    let layout = service.layout_text_shared(&font, &text, &opts(true, 150.0));
    assert!(layout.lines.len() > 1, "普通空格边界应触发换行");
    let joined: String = layout
        .lines
        .iter()
        .map(|line| line.start_char..line.end_char)
        .fold(String::new(), |mut acc, range| {
            acc.push_str(
                &text.chars()
                    .skip(range.start)
                    .take(range.end - range.start)
                    .collect::<String>(),
            );
            acc
        });
    assert_eq!(joined, text, "行逻辑区间拼接应还原完整源文本");
}

// ── 修复 3：动态标签依赖重绑定 ────────────────────────────────

#[test]
fn single_label_change_no_longer_reprobes_unrelated_labels() {
    let n = 1_000usize;
    let states: Vec<_> = (0..n).map(|_| State::new(0usize)).collect();
    let counts: Arc<Vec<AtomicUsize>> = Arc::new((0..n).map(|_| AtomicUsize::new(0)).collect());
    // 直接在真实 WidgetTree 上驱动布局：不挂 AppState，排除语义快照
    // 读取文本闭包的干扰，纯粹统计依赖重探测次数。
    let children: Vec<_> = states
        .iter()
        .enumerate()
        .map(|(i, state)| {
            let state = state.clone();
            let counts = Arc::clone(&counts);
            dynamic_label(move || {
                counts[i].fetch_add(1, Ordering::Relaxed);
                state.get().to_string()
            })
            .width(50.0)
            .height(21.0)
        })
        .collect();
    let mut tree = uix_app::ui::__private::build_view_tree_for_test(column_fit(children));
    tree.layout();
    for count in counts.iter() {
        count.store(0, Ordering::Relaxed);
    }
    states[0].set(1);
    tree.layout();
    let unrelated = counts
        .iter()
        .skip(1)
        .filter(|count| count.load(Ordering::Relaxed) > 0)
        .count();
    let total: usize = counts.iter().map(|count| count.load(Ordering::Relaxed)).sum();
    assert_eq!(unrelated, 0, "未失效标签不得重跑文本闭包（重探测）");
    assert!(total > 0, "被更新的标签仍需重新探测依赖");
    assert!(total <= 4, "局部变更只应重探测受影响标签: total={total}");
}

#[test]
fn single_label_change_keeps_semantic_snapshot_reads_bounded() {
    // TestApp 挂载 AppState 后，语义注册表每轮布局会重读标签文本；
    // 修复前每个无关标签每轮 settle 执行 2 次闭包（语义 + 全树重探测），
    // 修复后只剩 1 次语义读取。本断言锁定重探测消除后的上界。
    let n = 200usize;
    let states: Vec<_> = (0..n).map(|_| State::new(0usize)).collect();
    let counts: Arc<Vec<AtomicUsize>> = Arc::new((0..n).map(|_| AtomicUsize::new(0)).collect());
    let build_states = states.clone();
    let build_counts = Arc::clone(&counts);
    let mut app = TestApp::new((800.0, 600.0), move || {
        let children: Vec<_> = build_states
            .iter()
            .enumerate()
            .map(|(i, state)| {
                let state = state.clone();
                let counts = Arc::clone(&build_counts);
                dynamic_label(move || {
                    counts[i].fetch_add(1, Ordering::Relaxed);
                    state.get().to_string()
                })
                .width(50.0)
                .height(21.0)
                .automation_id(format!("text-{i}"))
            })
            .collect();
        column_fit(children)
    });
    app.settle().unwrap();
    for count in counts.iter() {
        count.store(0, Ordering::Relaxed);
    }
    states[0].set(1);
    app.settle().unwrap();
    // 闭包计数在语义快照读取之前完成，避免测试自身的断言读取混入统计。
    let over = counts
        .iter()
        .skip(1)
        .filter(|count| count.load(Ordering::Relaxed) > 1)
        .count();
    let total: usize = counts.iter().map(|count| count.load(Ordering::Relaxed)).sum();
    assert_eq!(over, 0, "无关标签每轮最多一次语义读取，不得再叠加全树重探测");
    assert!(total <= n + 4, "局部变更的闭包总数必须线性有界: total={total}");
    assert_eq!(app.text("text-0").as_deref(), Ok("1"));
}

#[test]
fn shared_state_rebinding_stays_linear_across_label_counts() {
    // 共享同一 State 的标签在重绑定时必须保持可扩展；同时验证失效传播仍然
    // 触达全部标签（这是行为断言，不是耗时门槛）。
    for n in [500usize, 2_000, 8_000] {
        let common = State::new(0usize);
        let children: Vec<_> = (0..n)
            .map(|_| {
                let state = common.clone();
                dynamic_label(move || state.get().to_string())
                    .width(50.0)
                    .height(21.0)
            })
            .collect();
        let mut tree = uix_app::ui::__private::build_view_tree_for_test(column_fit(children));
        tree.bind_reactive_widget_states();
        // 公开兼容入口仍保持全树重探测语义。
        tree.bind_reactive_widget_states();
        tree.reset_invalidation();
        common.set(1);
        let invalidated = tree.layout_traverse();
        assert!(
            invalidated.len() >= n,
            "共享 State 变更仍须失效全部 {n} 个标签, 实际 {}",
            invalidated.len()
        );
    }
}

#[test]
fn conditional_dependency_switch_rebinds_to_new_state_set() {
    let flag = State::new(false);
    let a = State::new("A1".to_owned());
    let b = State::new("B1".to_owned());
    let (bf, ba, bb) = (flag.clone(), a.clone(), b.clone());
    let mut app = TestApp::new((300.0, 200.0), move || {
        let (bf, ba, bb) = (bf.clone(), ba.clone(), bb.clone());
        column_fit((dynamic_label(move || if bf.get() { ba.get().clone() } else { bb.get().clone() })
            .width(120.0)
            .height(21.0)
            .automation_id("switcher"),))
    });
    assert_eq!(app.text("switcher").as_deref(), Ok("B1"));
    // 切换依赖集合：现在读取 a 而不是 b。
    flag.set(true);
    app.settle().unwrap();
    assert_eq!(app.text("switcher").as_deref(), Ok("A1"));
    // 新依赖变化必须生效。
    a.set("A2".to_owned());
    app.settle().unwrap();
    assert_eq!(app.text("switcher").as_deref(), Ok("A2"));
    // 旧依赖变化不得回写当前展示值。
    b.set("B2".to_owned());
    app.settle().unwrap();
    assert_eq!(app.text("switcher").as_deref(), Ok("A2"));
    // 切回 b 后必须立即反映 b 的最新值（未变更期间未被订阅也能恢复正确文本）。
    flag.set(false);
    app.settle().unwrap();
    assert_eq!(app.text("switcher").as_deref(), Ok("B2"));
}

#[test]
fn unmounted_label_rebinds_after_reinsert() {
    let visible = State::new(true);
    let text = State::new("第一版".to_owned());
    let (bv, bt) = (visible.clone(), text.clone());
    let mut app = TestApp::new((300.0, 200.0), move || {
        let body = bt.clone();
        let label = dynamic_label(move || body.get()).width(120.0).height(21.0).automation_id("rebind");
        let children: Vec<_> = if bv.get() { vec![label] } else { vec![] };
        column_fit(children)
    });
    assert_eq!(app.text("rebind").as_deref(), Ok("第一版"));
    // 卸载后源状态继续变化。
    visible.set(false);
    app.settle().unwrap();
    assert!(app.text("rebind").is_err(), "卸载后标签应缺席");
    text.set("第二版".to_owned());
    app.settle().unwrap();
    // 重新挂载必须立即绑定当前值，而不是保留首屏旧文本。
    visible.set(true);
    app.settle().unwrap();
    assert_eq!(app.text("rebind").as_deref(), Ok("第二版"));
    // 重挂载后的订阅链路继续可用。
    text.set("第三版".to_owned());
    app.settle().unwrap();
    assert_eq!(app.text("rebind").as_deref(), Ok("第三版"));
}

#[test]
fn shared_state_updates_labels_in_both_windows_and_isolation_holds() {
    let shared = State::new(0usize);
    let iso_a = State::new("win-a".to_owned());
    let iso_b = State::new("win-b".to_owned());
    let (s1, ia) = (shared.clone(), iso_a.clone());
    let mut app_a = TestApp::new((240.0, 120.0), move || {
        let (s, i) = (s1.clone(), ia.clone());
        column_fit((
            dynamic_label(move || s.get().to_string()).width(80.0).height(21.0).automation_id("shared-a"),
            dynamic_label(move || i.get().clone()).width(80.0).height(21.0).automation_id("only-a"),
        ))
    });
    let (s2, ib) = (shared.clone(), iso_b.clone());
    let mut app_b = TestApp::new((240.0, 120.0), move || {
        let (s, i) = (s2.clone(), ib.clone());
        column_fit((
            dynamic_label(move || s.get().to_string()).width(80.0).height(21.0).automation_id("shared-b"),
            dynamic_label(move || i.get().clone()).width(80.0).height(21.0).automation_id("only-b"),
        ))
    });
    // 共享状态同时驱动两个窗口的标签。
    shared.set(7);
    app_a.settle().unwrap();
    app_b.settle().unwrap();
    assert_eq!(app_a.text("shared-a").as_deref(), Ok("7"));
    assert_eq!(app_b.text("shared-b").as_deref(), Ok("7"));
    // 窗口私有状态的失效不得泄漏到另一窗口。
    iso_a.set("win-a-2".to_owned());
    app_a.settle().unwrap();
    app_b.settle().unwrap();
    assert_eq!(app_a.text("only-a").as_deref(), Ok("win-a-2"));
    assert_eq!(app_b.text("only-b").as_deref(), Ok("win-b"));
}

// ── 修复 4：布局缓存字节预算 ──────────────────────────────────

#[test]
fn long_text_history_is_bounded_by_cache_byte_budget() {
    let (mut service, font) = fonts();
    let base = service.memory_usage();
    let body = "a".repeat(10_000);
    let mut texts = Vec::new();
    for revision in 1..=128u32 {
        let text = format!("{revision:04} {body}");
        texts.push(text.clone());
        let layout = service.layout_text_shared(&font, &text, &opts(false, 0.0));
        assert_eq!(layout.lines.len(), 1);
        drop(layout);
        if revision == 1 {
            // 缓存仍然工作：首个版本必然保留。
            assert!(
                service.memory_usage() > base,
                "首个长文本版本仍应进入缓存"
            );
        }
    }
    let delta = service.memory_usage().saturating_sub(base);
    assert!(
        delta <= 9 * 1024 * 1024,
        "128 个长文本历史必须受字节预算约束, 实际保留 {delta} B"
    );
    // 预算淘汰后重排被逐出的旧版本必须得到一致的正确结果。
    let evicted = texts[0].clone();
    let relayout = service.layout_text_shared(&font, &evicted, &opts(false, 0.0));
    assert_eq!(relayout.glyphs.len(), evicted.chars().count());
    assert_eq!(relayout.lines.len(), 1);
    // 最近版本仍在缓存内：重复布局命中并返回等价数据。
    let recent = texts[127].clone();
    let again = service.layout_text_shared(&font, &recent, &opts(false, 0.0));
    assert_eq!(again.glyphs.len(), recent.chars().count());
    service.clear_glyph_cache();
    assert!(service.memory_usage() < base + 1024 * 1024, "显式清空仍可回收");
}

#[test]
fn oversized_single_layout_stays_correct_without_entering_cache() {
    let (service, font) = fonts();
    let base = service.memory_usage();
    // 超过单条 admission 上限：> 1 MiB 的字形数据。
    let per_glyph = std::mem::size_of::<PositionedGlyph>();
    let oversized_chars = (1024 * 1024 / per_glyph + 10_000) * 2;
    let text = "a".repeat(oversized_chars);
    let layout = service.layout_text_shared(&font, &text, &opts(false, 0.0));
    assert_eq!(
        layout.glyphs.len(),
        oversized_chars,
        "超大文本布局本身必须完整正确"
    );
    drop(layout);
    let delta = service.memory_usage().saturating_sub(base);
    assert!(
        delta < 128 * 1024,
        "被拒绝入缓存的超大布局不得驻留服务内存, 实际 {delta} B"
    );
}

#[test]
fn short_text_hot_cache_keeps_working_under_budget() {
    let (service, font) = fonts();
    let base = service.memory_usage();
    // 远超条目数上限的短文本：FIFO 条目上限继续生效，总内存仍然有界。
    for revision in 0..3_000u32 {
        let text = format!("短文本 rev {revision}");
        let layout = service.layout_text_shared(&font, &text, &opts(true, 120.0));
        assert!(!layout.glyphs.is_empty());
    }
    let delta = service.memory_usage().saturating_sub(base);
    assert!(
        delta <= 9 * 1024 * 1024,
        "短文本历史同样受预算约束, 实际 {delta} B"
    );
}

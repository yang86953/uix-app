//! `native/backends/linux/host/system_info/fonts.rs` 的 cfg(test) 单元测试体；模块层级不变，经 #[path] 引用，不进发布包。

use super::*;

#[test]
fn collection_members_remain_distinct_and_malformed_indices_are_rejected() {
    let sources = parse_font_sources(
        "/fonts/Noto CJK.ttc\t2\n/fonts/Noto CJK.ttc\t0\n/fonts/Noto CJK.ttc\t2\n/fonts/latin.ttf\t0\n/fonts/bad.ttc\tno\n/fonts/missing-index.ttc\n/fonts/negative.ttc\t-1\n(null)\t0\n\t0\n",
    );
    assert_eq!(
        sources,
        vec![
            SystemFontSource {
                path: "/fonts/Noto CJK.ttc".into(),
                face_index: 2
            },
            SystemFontSource {
                path: "/fonts/Noto CJK.ttc".into(),
                face_index: 0
            },
            SystemFontSource {
                path: "/fonts/latin.ttf".into(),
                face_index: 0
            },
        ]
    );
    assert_eq!(
        unique_paths(sources),
        ["/fonts/Noto CJK.ttc", "/fonts/latin.ttf"]
    );
}

#[test]
fn candidate_budget_is_preserved_with_many_faces_in_one_file() {
    let output = (0..100)
        .map(|index| format!("/fonts/collection.ttc\t{index}\n"))
        .collect::<String>();
    let sources = parse_font_sources(&output);
    assert_eq!(sources.len(), MAX_CANDIDATES);
    assert_eq!(sources.last().unwrap().face_index, 31);
}

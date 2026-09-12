// 引入核心 View 生成与文档解析入口。
use super::{generate_view, parse_document};

// 生成只含一条列模板的元素并返回令牌文本。
fn tokens_for(source: &str) -> Result<String, super::Diagnostic> {
    let document = parse_document(&format!(
        r#"<Container style="display: grid; gridTemplateColumns: {source};"></Container>"#
    ))
    .expect("轨道模板语法应合法");
    generate_view(&document.root).map(|tokens| tokens.to_string())
}

// 验证四种有界组合保留到运行时 GridTrack::MinMax，且与普通轨道混排。
#[test]
fn generates_supported_minmax_combinations() {
    let tokens = tokens_for(
        "minmax(240px, 1fr) minmax(20%, 400px) minmax(100px, 300px) minmax(10%, 2fr) 1fr auto 80px",
    )
    .expect("有界轨道应映射");
    assert!(tokens.contains("GridTrackMin :: Px (240"), "{tokens}");
    assert!(tokens.contains("GridTrackMax :: Fr (1"), "{tokens}");
    assert!(tokens.contains("GridTrackMin :: Percent (20"), "{tokens}");
    assert!(tokens.contains("GridTrackMax :: Px (400"), "{tokens}");
    assert!(tokens.contains("GridTrack :: Fr (1"), "{tokens}");
    assert!(tokens.contains("GridTrack :: Auto"), "{tokens}");
    assert!(tokens.contains("GridTrack :: Px (80"), "{tokens}");
    assert_eq!(tokens.matches("GridTrack :: MinMax").count(), 4);
}

// 验证不支持的下界/上界与形状都给出确定诊断。
#[test]
fn rejects_unsupported_minmax_forms() {
    for (source, expect) in [
        ("minmax(auto, 1fr)", "不支持 auto/fr"),
        ("minmax(1fr, 2fr)", "不支持 auto/fr"),
        ("minmax(100px, auto)", "不支持 auto/百分比"),
        ("minmax(100px, 50%)", "不支持 auto/百分比"),
        ("minmax(100px)", "需要两个参数"),
        ("minmax(100px, 200px, 300px)", "需要两个参数"),
        ("minmax(minmax(1px, 2px), 1fr)", "不支持嵌套"),
        ("minmax(-10px, 1fr)", "有限非负"),
        ("repeat(3, 1fr)", "缺少 px/fr 单位"),
    ] {
        let error = tokens_for(source).expect_err(&format!("{source} 必须失败"));
        assert!(
            error.message.contains(expect),
            "{source} 的诊断应包含 {expect:?}，实际 {:?}",
            error.message
        );
    }
}

//! `uix-lang-compiler/formatter.rs` 的 cfg(test) 单元测试体；模块层级不变，经 #[path] 引用，不进发布包。

use super::format_source;
use crate::source_graph::SourceId;

#[test]
fn formatter_is_idempotent_and_preserves_semantic_ast() {
    let source = "<Column><Text>A</Text><Button>B</Button></Column>";
    let source_id = SourceId::from_source_name("demo.uix");
    let first = format_source(source, source_id).expect("首次格式化必须成功");
    assert_eq!(
        first.formatted,
        "<Column>\n  <Text>A</Text>\n  <Button>B</Button>\n</Column>"
    );
    let second = format_source(&first.formatted, source_id).expect("二次格式化必须成功");
    assert_eq!(second.formatted, first.formatted);
    assert_eq!(second.cst.reconstruct(), second.formatted);
}

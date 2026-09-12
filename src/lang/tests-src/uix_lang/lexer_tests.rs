//! `uix-lang-compiler/uix_lang/lexer.rs` 的 cfg(test) 单元测试体；模块层级不变，经 #[path] 引用，不进发布包。

use super::Cursor;

#[test]
fn incremental_line_column_cache_preserves_unicode_and_backward_spans() {
    let source = "甲乙\nA好\n末";
    let cursor = Cursor::new(source);

    let second_character = cursor.span_between("甲".len(), "甲乙".len());
    assert_eq!((second_character.line, second_character.column), (1, 2));

    let last_line = source.find('末').expect("夹具必须包含末字");
    let last_character = cursor.span_between(last_line, source.len());
    assert_eq!((last_character.line, last_character.column), (3, 1));

    let backward = source.find('好').expect("夹具必须包含好字");
    let backward_span = cursor.span_between(backward, backward + '好'.len_utf8());
    assert_eq!((backward_span.line, backward_span.column), (2, 2));

    let forward_again = cursor.span_between(last_line, source.len());
    assert_eq!((forward_again.line, forward_again.column), (3, 1));
}

include!(concat!(env!("OUT_DIR"), "/usage_markdown_compile.rs"));

#[test]
fn usage_markdown_compile_inventory_is_explicit() {
    assert_eq!(USAGE_RUST_BLOCKS_TOTAL, 98);
    assert_eq!(USAGE_RUST_BLOCKS_COMPILED, 30);
}

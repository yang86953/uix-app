mod generated {
    include!(concat!(env!("OUT_DIR"), "/usage_markdown_compile.rs"));
}

#[test]
fn usage_markdown_compile_inventory_tracks_enabled_features() {
    assert_eq!(generated::USAGE_RUST_BLOCKS_TOTAL, 122);
    assert_eq!(generated::USAGE_RUST_BLOCK_FEATURES, &["test-harness"]);
    assert_eq!(
        generated::USAGE_RUST_BLOCKS_COMPILED,
        121 + usize::from(cfg!(feature = "test-harness"))
    );
}

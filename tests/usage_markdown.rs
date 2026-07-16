mod generated {
    include!(concat!(env!("OUT_DIR"), "/usage_markdown_compile.rs"));
}

#[test]
fn usage_markdown_compile_inventory_tracks_enabled_features() {
    assert_eq!(generated::USAGE_RUST_BLOCKS_TOTAL, 136);
    assert_eq!(
        generated::USAGE_RUST_BLOCK_FEATURES,
        &["settings-serde", "test-harness"]
    );
    assert_eq!(
        generated::USAGE_RUST_BLOCKS_COMPILED,
        134 + usize::from(cfg!(feature = "settings-serde"))
            + usize::from(cfg!(feature = "test-harness"))
    );
}

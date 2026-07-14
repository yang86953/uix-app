use std::fs;
use std::path::Path;

#[test]
fn platform_test_modules_mirror_native_visibility() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let expected_gates = [
        (
            "tests/native/backends/mod.rs",
            "#[cfg(windows)]\nmod windows;",
        ),
        (
            "tests/native/graphics/mod.rs",
            "#[cfg(feature = \"d3d11\")]\nmod d3d11;",
        ),
        (
            "tests/native/graphics/mod.rs",
            "#[cfg(feature = \"d3d12\")]\nmod d3d12;",
        ),
        (
            "tests/native/graphics/mod.rs",
            "#[cfg(feature = \"opengles\")]\nmod opengl;",
        ),
        (
            "tests/native/graphics/mod.rs",
            "#[cfg(feature = \"vulkan\")]\nmod vulkan;",
        ),
        (
            "tests/native/graphics/opengl/platform/mod.rs",
            "#[cfg(windows)]\nmod wgl;",
        ),
        (
            "tests/native/graphics/d3d11/platform/mod.rs",
            "#[cfg(windows)]\nmod context;",
        ),
        (
            "tests/native/graphics/d3d11/platform/mod.rs",
            "#[cfg(windows)]\nmod pipeline;",
        ),
        (
            "tests/native/graphics/d3d12/platform/mod.rs",
            "#[cfg(windows)]\nmod context;",
        ),
        (
            "tests/native/graphics/d3d12/platform/mod.rs",
            "#[cfg(windows)]\nmod pipeline;",
        ),
        (
            "tests/native/graphics/platform/mod.rs",
            "#[cfg(windows)]\nmod windows;",
        ),
    ];

    for (path, expected) in expected_gates {
        let source = fs::read_to_string(root.join(path))
            .unwrap_or_else(|error| panic!("failed to read {path}: {error}"))
            .replace("\r\n", "\n");
        assert!(
            source.contains(expected),
            "platform test module must mirror its native visibility: {path}: {expected}"
        );
    }
}

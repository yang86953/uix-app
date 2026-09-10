//! 正文默认字体的公开契约：显式字体包优先、系统发现来源、缺字体 typed 失败。
//! 不依赖任何机器安装字体，也不再依赖仓库内置的完整正文字体。

use std::sync::atomic::{AtomicUsize, Ordering};

use uix_app::core::Errc;
use uix_app::draw::{FontBundle, FontService};
use uix_app::platform::services::{FontSystemInfo, SystemFontSource};

// 确定性测试 fixture：仅覆盖测试文本的 OFL 子集（见 THIRD_PARTY_NOTICES.md）。
const FIXTURE: &[u8] = include_bytes!("fixtures/fonts/uix-test-body.ttf");
const FIXTURE_PATH: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/fonts/uix-test-body.ttf"
);

// 可注入的系统字体发现替身：记录被征询次数，路径由用例给定。
struct FakeDiscovery {
    paths: Vec<Result<Vec<String>, String>>,
    consulted: AtomicUsize,
}

impl FakeDiscovery {
    fn with_paths(paths: &[&str]) -> Self {
        Self {
            paths: vec![Ok(paths.iter().map(|p| (*p).to_string()).collect())],
            consulted: AtomicUsize::new(0),
        }
    }
    fn empty() -> Self {
        Self {
            paths: vec![Ok(Vec::new())],
            consulted: AtomicUsize::new(0),
        }
    }
    fn consulted(&self) -> usize {
        self.consulted.load(Ordering::SeqCst)
    }
}

impl FontSystemInfo for FakeDiscovery {
    fn default_font_paths(&self) -> Result<Vec<String>, uix_app::core::Error> {
        self.consulted.fetch_add(1, Ordering::SeqCst);
        match self.paths.first() {
            Some(Ok(paths)) => Ok(paths.clone()),
            Some(Err(message)) => Err(uix_app::core::Error::new(
                Errc::NotFound,
                message.clone(),
            )),
            None => Ok(Vec::new()),
        }
    }

    fn probe_cjk_font_paths(&self) -> Vec<String> {
        Vec::new()
    }

    fn probe_family_font_path(&self, _family: &str) -> Option<String> {
        None
    }

    fn scan_fallback_font_path(&self) -> Option<String> {
        None
    }
}

fn fixture_bundle() -> FontBundle {
    FontBundle::from_static("UIX Test Body", FIXTURE)
}

#[test]
fn explicit_bundle_takes_priority_and_skips_discovery() {
    let mut fonts = FontService::new();
    let discovery = FakeDiscovery::with_paths(&["/nonexistent/uix-fake-body.ttf"]);
    let deterministic = fonts
        .install_body_font(Some(fixture_bundle()), &discovery)
        .expect("explicit bundle must install");
    assert!(deterministic, "explicit bundle reports deterministic mode");
    // 显式分支不得征询平台发现；即使系统发现指向不可用路径也无影响。
    assert_eq!(discovery.consulted(), 0, "discovery must not be consulted");
    assert_eq!(
        fonts.font_family(&fonts.loaded_font_handle),
        Some("UIX Test Body"),
        "loaded body font family must come from the explicit bundle"
    );
}

#[test]
fn system_discovery_loads_body_font_without_explicit_bundle() {
    let mut fonts = FontService::new();
    let discovery = FakeDiscovery::with_paths(&[FIXTURE_PATH]);
    let deterministic = fonts
        .install_body_font(None, &discovery)
        .expect("discovered fixture path must load");
    assert!(!deterministic, "system discovery reports non-deterministic mode");
    assert_eq!(discovery.consulted(), 1, "discovery must be consulted once");
    // OS-native 路径以请求的默认族标签注册主字体；可绘制正文已装载即可，
    // 真实字形面允许随平台字体策略变化。
    assert!(
        fonts.font_family(&fonts.loaded_font_handle).is_some(),
        "a real body font must be loaded from the discovered system source"
    );
}

#[test]
fn missing_body_font_fails_with_typed_error_instead_of_bitmap_fallback() {
    let mut fonts = FontService::new();
    let discovery = FakeDiscovery::empty();
    let error = fonts
        .install_body_font(None, &discovery)
        .expect_err("missing body font must fail startup");
    assert_eq!(error.code(), Errc::NotFound);
    assert!(
        error.message().contains("FontBundle"),
        "error must name the remediation: {}",
        error.message()
    );
    // 失败后主字体句柄不得假装持有真实字体。
    assert_eq!(
        fonts.font_family(&fonts.loaded_font_handle),
        None,
        "bitmap fallback must not pretend to be a loaded body font"
    );
}

#[test]
fn discovery_error_path_also_fails_closed() {
    let mut fonts = FontService::new();
    let discovery = FakeDiscovery {
        paths: vec![Err("registry unavailable".to_string())],
        consulted: AtomicUsize::new(0),
    };
    let error = fonts
        .install_body_font(None, &discovery)
        .expect_err("discovery failure without explicit bundle must fail startup");
    assert_eq!(error.code(), Errc::NotFound);
    assert_eq!(fonts.font_family(&fonts.loaded_font_handle), None);
}

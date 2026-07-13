use crate::draw::font::font_service::FontService;
use crate::native::test_harness::fake_system_info::FakeSystemInfo;
use crate::native::traits::system::ISystemInfo;
use std::cell::Cell;

/// 统计 `probe_cjk_font_paths` 调用，验证启动只走「主字体 + CJK」路径。
struct CountingSystemInfo {
    inner: FakeSystemInfo,
    cjk_probe_calls: Cell<usize>,
    default_paths: Vec<String>,
}

impl CountingSystemInfo {
    fn with_paths(paths: Vec<String>) -> Self {
        Self {
            inner: FakeSystemInfo::new(),
            cjk_probe_calls: Cell::new(0),
            default_paths: paths,
        }
    }
}

impl ISystemInfo for CountingSystemInfo {
    fn os_info(&self) -> crate::native::traits::system::OsInfo {
        self.inner.os_info()
    }

    fn cpu_count(&self) -> u32 {
        self.inner.cpu_count()
    }

    fn memory_info(&self) -> crate::native::traits::system::MemoryInfo {
        self.inner.memory_info()
    }

    fn hostname(&self) -> String {
        self.inner.hostname()
    }

    fn username(&self) -> String {
        self.inner.username()
    }

    fn up_time(&self) -> u64 {
        self.inner.up_time()
    }

    fn default_font_paths(&self) -> Vec<String> {
        self.inner
            .default_font_calls
            .set(self.inner.default_font_calls.get() + 1);
        self.default_paths.clone()
    }

    fn probe_cjk_font_paths(&self) -> Vec<String> {
        self.cjk_probe_calls.set(self.cjk_probe_calls.get() + 1);
        Vec::new()
    }
}

#[test]
fn load_default_system_font_stops_after_first_primary_and_probes_cjk_once() {
    let lucide = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/assets/fonts/lucide.ttf"
    );
    // 同一主字体路径重复多次：旧实现会把后续全部当 fallback 同步装载。
    let info = CountingSystemInfo::with_paths(vec![
        lucide.to_string(),
        lucide.to_string(),
        lucide.to_string(),
    ]);

    let mut fonts = FontService::new();
    fonts.load_default_system_font(14.0, &info);

    assert_eq!(info.inner.default_font_calls.get(), 1);
    assert_eq!(info.cjk_probe_calls.get(), 1);
    // 主字体 1 + 未装 CJK；不得把重复路径再装成 fallback。
    assert_eq!(fonts.font_count(), 1);
    assert_eq!(fonts.fallback_count(), 0);
}

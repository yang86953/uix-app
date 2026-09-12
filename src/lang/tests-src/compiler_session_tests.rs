//! `uix-lang-compiler/src/compiler_session.rs` 的 cfg(test) 单元测试体；模块层级不变，经 #[path] 引用，不进发布包。

// 测试可观测的阶段执行统计类型（自源文件移入，源内仅保留 cfg(test) 字段与计数插桩）。
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub(super) struct CompilerSessionStats {
    pub(super) source_parses: usize,
    pub(super) source_files: usize,
    pub(super) analysis_runs: usize,
    pub(super) analysis_hits: usize,
    pub(super) snapshot_hits: usize,
    pub(super) analysis_handle: usize,
    pub(super) lowering_runs: usize,
    pub(super) lowering_hits: usize,
    pub(super) lowering_handle: usize,
    pub(super) compile_runs: usize,
    pub(super) compile_hits: usize,
    pub(super) compile_handle: usize,
    pub(super) check_runs: usize,
    pub(super) check_hits: usize,
}

impl super::CompilerSession {
    // 返回确定性阶段执行统计，仅用于证明缓存命中，不进入公开 API。
    pub(super) fn test_stats(&self) -> CompilerSessionStats {
        CompilerSessionStats {
            source_parses: self.source_cache.parse_runs(),
            source_files: self.source_cache.len(),
            ..self.stats
        }
    }
}

use super::*;

fn import_fixture() -> (PathBuf, PathBuf) {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../tests/fixtures/uix_lang/imports/root.uix");
    let helper = root
        .parent()
        .expect("根文件必须有父目录")
        .join("shared/helper.uix");
    (root, helper)
}

#[test]
fn unchanged_graph_reuses_parse_analysis_and_check_stages() {
    let (root, _) = import_fixture();
    let mut session = CompilerSession::new();
    let first = session
        .check_file(&root, CompileTarget::View)
        .expect("初始源码图必须通过检查");
    let first_stats = session.test_stats();

    let second = session
        .check_file(&root, CompileTarget::View)
        .expect("未变化源码图必须复用检查结果");
    let second_stats = session.test_stats();

    assert_eq!(first.compilation_key, second.compilation_key);
    assert_eq!(second_stats.source_parses, first_stats.source_parses);
    assert_eq!(second_stats.analysis_runs, first_stats.analysis_runs);
    assert_eq!(second_stats.check_runs, first_stats.check_runs);
    assert_ne!(first_stats.analysis_handle, 0);
    assert_eq!(second_stats.analysis_handle, first_stats.analysis_handle);
    assert_eq!(second_stats.analysis_hits, first_stats.analysis_hits + 1);
    assert_eq!(second_stats.check_hits, first_stats.check_hits + 1);
    assert!(
        session
            .pipelines
            .values()
            .all(|pipeline| pipeline.check_ready),
        "检查缓存只保留 readiness，不应复制完整 CheckOutput",
    );
}

#[test]
fn unchanged_single_file_overlay_skips_resolver_rebuild() {
    let root = PathBuf::from("/tmp/uix-compiler-session-single-file.uix");
    let mut overlays = BTreeMap::new();
    overlays.insert(
        root.clone(),
        "<Column><Text>稳定内容</Text></Column>".to_string(),
    );
    let mut session = CompilerSession::new();
    session
        .check_file_with_overlays(&root, &overlays, CompileTarget::View)
        .expect("初始单文件 overlay 必须通过检查");
    let initial = session.test_stats();

    session
        .check_file_with_overlays(&root, &overlays, CompileTarget::View)
        .expect("未变化单文件 overlay 必须命中快照");
    let unchanged = session.test_stats();
    assert_eq!(unchanged.snapshot_hits, initial.snapshot_hits + 1);
    assert_eq!(unchanged.source_parses, initial.source_parses);
    assert_eq!(unchanged.analysis_runs, initial.analysis_runs);

    overlays.insert(
        root.clone(),
        "<Column><Text>变化内容</Text></Column>".to_string(),
    );
    session
        .check_file_with_overlays(&root, &overlays, CompileTarget::View)
        .expect("变化后的单文件 overlay 必须重新分析");
    let changed = session.test_stats();
    assert_eq!(changed.snapshot_hits, unchanged.snapshot_hits);
    assert_eq!(changed.source_parses, unchanged.source_parses + 1);
    assert_eq!(changed.analysis_runs, unchanged.analysis_runs + 1);
}

#[test]
fn unchanged_multi_file_overlay_skips_resolver_rebuild() {
    let fixture = std::env::temp_dir().join(format!(
        "uix-session-stable-multi-file-{}",
        std::process::id()
    ));
    let root = fixture.join("root.uix");
    let dependency = fixture.join("dependency.uix");
    let mut overlays = BTreeMap::new();
    overlays.insert(
        root.clone(),
        "@import('./dependency.uix', 'Helper')\n<App><Helper /></App>".to_string(),
    );
    overlays.insert(
        dependency,
        "@export('Helper')\n<Widget name=\"Helper\"><Text>稳定依赖</Text></Widget>\n<Helper />"
            .to_string(),
    );
    let mut session = CompilerSession::new();
    session
        .check_file_with_overlays(&root, &overlays, CompileTarget::App)
        .expect("初始多文件 overlay 必须通过检查");
    let initial = session.test_stats();

    session
        .check_file_with_overlays(&root, &overlays, CompileTarget::App)
        .expect("未变化多文件 overlay 必须命中快照");
    let unchanged = session.test_stats();

    assert_eq!(unchanged.snapshot_hits, initial.snapshot_hits + 1);
    assert_eq!(unchanged.source_parses, initial.source_parses);
    assert_eq!(unchanged.analysis_runs, initial.analysis_runs);
    assert_eq!(unchanged.analysis_handle, initial.analysis_handle);
}

#[test]
fn dependency_overlay_reparses_only_the_changed_file() {
    let (root, helper) = import_fixture();
    let mut session = CompilerSession::new();
    let initial = session
        .check_file(&root, CompileTarget::View)
        .expect("初始源码图必须通过检查");
    let initial_stats = session.test_stats();
    let mut overlays = BTreeMap::new();
    overlays.insert(
        helper,
        "@export('Helper')\n<Widget name=\"Helper\"><Text>变更依赖</Text></Widget>\n<Helper />"
            .to_string(),
    );

    let changed = session
        .check_file_with_overlays(&root, &overlays, CompileTarget::View)
        .expect("依赖 overlay 必须通过检查");
    let changed_stats = session.test_stats();

    assert_ne!(
        initial.compilation_key.dependency_hash,
        changed.compilation_key.dependency_hash
    );
    assert_eq!(changed_stats.source_parses, initial_stats.source_parses + 1);
    assert_eq!(changed_stats.analysis_runs, initial_stats.analysis_runs + 1);
    assert_eq!(changed_stats.check_runs, initial_stats.check_runs + 1);
}

#[test]
fn check_and_compile_share_analysis_and_lowering_stages() {
    let (root, _) = import_fixture();
    let mut session = CompilerSession::new();
    session
        .check_file(&root, CompileTarget::View)
        .expect("检查必须成功");
    let checked = session.test_stats();

    session
        .compile_file(&root, CompileTarget::View)
        .expect("编译必须成功");
    let compiled = session.test_stats();
    session
        .compile_file(&root, CompileTarget::View)
        .expect("重复编译必须命中缓存");
    let repeated = session.test_stats();

    assert_eq!(compiled.analysis_runs, checked.analysis_runs);
    assert_eq!(compiled.lowering_runs, checked.lowering_runs);
    assert_ne!(checked.lowering_handle, 0);
    assert_eq!(compiled.lowering_handle, checked.lowering_handle);
    assert_eq!(compiled.lowering_hits, checked.lowering_hits + 1);
    assert_eq!(compiled.compile_runs, checked.compile_runs + 1);
    assert_ne!(compiled.compile_handle, 0);
    assert_eq!(repeated.compile_runs, compiled.compile_runs);
    assert_eq!(repeated.compile_hits, compiled.compile_hits + 1);
    assert_eq!(repeated.compile_handle, compiled.compile_handle);
}

#[test]
fn repeated_syntax_failure_reuses_the_stable_diagnostic() {
    let (root, _) = import_fixture();
    let mut session = CompilerSession::new();
    let mut overlays = BTreeMap::new();
    overlays.insert(root.clone(), "<App><Text></App>".to_string());

    let first = session
        .check_file_with_overlays(&root, &overlays, CompileTarget::App)
        .expect_err("无效语法必须返回诊断");
    let first_stats = session.test_stats();
    let second = session
        .check_file_with_overlays(&root, &overlays, CompileTarget::App)
        .expect_err("重复无效语法必须返回相同诊断");
    let second_stats = session.test_stats();

    assert_eq!(first, second);
    assert_eq!(second_stats.source_parses, first_stats.source_parses);
    assert_eq!(second_stats.analysis_runs, first_stats.analysis_runs);
}

#[test]
fn repeated_semantic_failure_reuses_analysis_without_changing_diagnostic() {
    let (root, _) = import_fixture();
    let mut session = CompilerSession::new();
    let mut overlays = BTreeMap::new();
    overlays.insert(root.clone(), "<App><Unknown /></App>".to_string());

    let first = session
        .check_file_with_overlays(&root, &overlays, CompileTarget::App)
        .expect_err("未知组件必须返回语义诊断");
    let first_stats = session.test_stats();
    let second = session
        .check_file_with_overlays(&root, &overlays, CompileTarget::App)
        .expect_err("重复未知组件必须返回相同诊断");
    let second_stats = session.test_stats();

    assert_eq!(first, second);
    assert_eq!(second_stats.source_parses, first_stats.source_parses);
    assert_eq!(second_stats.analysis_runs, first_stats.analysis_runs);
    assert_eq!(second_stats.analysis_hits, first_stats.analysis_hits + 1);
}

#[test]
fn evicting_dependency_invalidates_its_owning_root_pipeline() {
    let (root, helper) = import_fixture();
    let mut session = CompilerSession::new();
    session
        .check_file(&root, CompileTarget::View)
        .expect("初始源码图必须通过检查");
    let initial = session.test_stats();

    let affected = session.evict_file(&helper);

    assert_eq!(affected, vec![normalized_overlay_path(&root)]);
    assert_eq!(session.test_stats().source_files, 0);
    session
        .check_file(&root, CompileTarget::View)
        .expect("依赖淘汰后必须能重新建立源码图");
    let rebuilt = session.test_stats();
    assert!(rebuilt.source_parses > initial.source_parses);
    assert_eq!(rebuilt.analysis_runs, initial.analysis_runs + 1);
}

#[test]
fn failed_dependency_cache_is_released_with_the_affected_root() {
    let fixture = std::env::temp_dir().join(format!(
        "uix-session-failed-dependency-{}",
        std::process::id()
    ));
    let root = fixture.join("root.uix");
    let dependency = fixture.join("dependency.uix");
    let mut overlays = BTreeMap::new();
    overlays.insert(
        root.clone(),
        "@import('./dependency.uix', 'Helper')\n<App><Helper /></App>".to_string(),
    );
    overlays.insert(dependency.clone(), "<Widget".to_string());
    let mut session = CompilerSession::new();

    session
        .check_file_with_overlays(&root, &overlays, CompileTarget::App)
        .expect_err("依赖语法错误必须阻止检查");
    assert_eq!(session.test_stats().source_files, 2);

    assert_eq!(session.evict_file(&dependency), vec![root]);
    assert_eq!(session.test_stats().source_files, 0);
}

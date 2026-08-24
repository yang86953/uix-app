//! Compiler System 会话缓存：按源码图身份复用 Syntax、Semantic 与 Emit 阶段。

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::uix_import::{
    SourceStageCache, normalized_overlay_path, resolve_file_with_overlays_cached,
};
use crate::{
    AnalyzedUnit, CheckOutput, CompilationKey, CompileOutput, CompileTarget, CompiledArtifact,
    CompilerDiagnostic, RustUiPlan, analyze_resolved_file, compile_cached_artifact,
    inferred_target, lower_analyzed, materialize_check_output, materialize_compile_output,
};

// 区分同一会话中不同根和目标形状的流水线；每个身份只保留最新源码图。
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct PipelineIdentity {
    root: PathBuf,
    target: CompileTarget,
}

// 保存同一完整 CompilationKey 下可逐阶段复用的确定性结果。
#[derive(Debug, Clone)]
struct CachedPipeline {
    compilation_key: CompilationKey,
    tracked_files: Vec<PathBuf>,
    // 阶段产物不可变且可能很大；缓存命中只共享所有权，不深拷贝完整语义模型。
    analysis: Result<Arc<AnalyzedUnit>, CompilerDiagnostic>,
    lowering: Option<Result<Arc<RustUiPlan>, CompilerDiagnostic>>,
    // Emit 缓存只保存 analysis 之外的增量产物，避免重复持有 SourceGraph 与 TypedUiIr。
    compile: Option<Result<Arc<CompiledArtifact>, CompilerDiagnostic>>,
    // lowering 成功后检查已就绪；公开 CheckOutput 每次从共享 analysis 物化。
    check_ready: bool,
}

/// 持有多个编译请求之间可复用、且有明确会话生命周期的 Compiler System 缓存。
///
/// 单次 `CompilerSystem` 命令继续保持无状态；LSP、热重载等长寿命 Adapter 应持有本
/// 类型。每个根/目标只保留最新流水线，单文件 Syntax 缓存也只保留最新源码版本。
#[derive(Debug, Default)]
pub struct CompilerSession {
    source_cache: SourceStageCache,
    pipelines: BTreeMap<PipelineIdentity, CachedPipeline>,
    // 即使当前源码图失败，也保存本次触及文件的根级所有权，保证复用与释放都有界。
    failed_roots: BTreeMap<PathBuf, BTreeSet<PathBuf>>,
    #[cfg(test)]
    stats: CompilerSessionStats,
}

impl CompilerSession {
    /// 创建没有已解析文件或流水线产物的新会话。
    pub fn new() -> Self {
        Self::default()
    }

    /// 编译真实根文件，并复用同一会话内未变化的阶段产物。
    pub fn compile_file(
        &mut self,
        path: &Path,
        target: CompileTarget,
    ) -> Result<CompileOutput, CompilerDiagnostic> {
        self.compile_file_with_snapshot(path, &BTreeMap::new(), target)
    }

    /// 检查真实根文件，并复用同一会话内未变化的阶段产物。
    pub fn check_file(
        &mut self,
        path: &Path,
        target: CompileTarget,
    ) -> Result<CheckOutput, CompilerDiagnostic> {
        self.check_file_with_snapshot(path, &BTreeMap::new(), Some(target))
    }

    /// 按根元素自动选择目标并复用检查阶段产物。
    pub fn check_file_auto(&mut self, path: &Path) -> Result<CheckOutput, CompilerDiagnostic> {
        self.check_file_with_snapshot(path, &BTreeMap::new(), None)
    }

    /// 使用编辑器 overlay 检查指定目标，缓存只绑定本次完整源码图身份。
    pub fn check_file_with_overlays(
        &mut self,
        path: &Path,
        overlays: &BTreeMap<PathBuf, String>,
        target: CompileTarget,
    ) -> Result<CheckOutput, CompilerDiagnostic> {
        self.check_file_with_snapshot(path, overlays, Some(target))
    }

    /// 使用编辑器 overlay 自动选择目标并复用未变化阶段。
    pub fn check_file_with_overlays_auto(
        &mut self,
        path: &Path,
        overlays: &BTreeMap<PathBuf, String>,
    ) -> Result<CheckOutput, CompilerDiagnostic> {
        self.check_file_with_snapshot(path, overlays, None)
    }

    /// 释放一个根拥有的全部流水线，并清理不再被其他根引用的单文件缓存。
    pub fn evict_root(&mut self, path: &Path) {
        let root = normalized_overlay_path(path);
        self.pipelines.retain(|identity, _| identity.root != root);
        self.failed_roots.remove(&root);
        self.prune_source_cache();
    }

    /// 释放直接或递归依赖指定文件的全部根流水线，并返回受影响根。
    pub fn evict_file(&mut self, path: &Path) -> Vec<PathBuf> {
        let path = normalized_overlay_path(path);
        let mut affected_roots = BTreeSet::new();
        self.pipelines.retain(|identity, pipeline| {
            let affected = identity.root == path || pipeline.tracked_files.contains(&path);
            if affected {
                affected_roots.insert(identity.root.clone());
            }
            !affected
        });
        self.failed_roots.retain(|root, touched_paths| {
            let affected = *root == path || touched_paths.contains(&path);
            if affected {
                affected_roots.insert(root.clone());
            }
            !affected
        });
        self.prune_source_cache();
        affected_roots.into_iter().collect()
    }

    /// 热重载使用当前 overlay 复用完整 AOT 流水线。
    #[cfg(feature = "hot-reload")]
    pub(crate) fn compile_file_with_overlays(
        &mut self,
        path: &Path,
        overlays: &BTreeMap<PathBuf, String>,
        target: CompileTarget,
    ) -> Result<CompileOutput, CompilerDiagnostic> {
        self.compile_file_with_snapshot(path, overlays, target)
    }

    fn compile_file_with_snapshot(
        &mut self,
        path: &Path,
        overlays: &BTreeMap<PathBuf, String>,
        target: CompileTarget,
    ) -> Result<CompileOutput, CompilerDiagnostic> {
        let (identity, analysis) = self.analyze_file(path, overlays, Some(target))?;
        if let Some(cached) = self
            .pipelines
            .get(&identity)
            .and_then(|pipeline| pipeline.compile.as_ref())
        {
            #[cfg(test)]
            {
                self.stats.compile_hits = self.stats.compile_hits.saturating_add(1);
            }
            let artifact = cached.clone()?;
            #[cfg(test)]
            {
                self.stats.compile_handle = Arc::as_ptr(&artifact) as usize;
            }
            return Ok(materialize_compile_output(&analysis, &artifact));
        }
        let plan = self.lower(&identity, &analysis)?;
        #[cfg(test)]
        {
            self.stats.compile_runs = self.stats.compile_runs.saturating_add(1);
        }
        let result = compile_cached_artifact(&analysis, &plan).map(Arc::new);
        self.pipelines
            .get_mut(&identity)
            .expect("分析成功后必须保留对应流水线")
            .compile = Some(result.clone());
        let artifact = result?;
        #[cfg(test)]
        {
            self.stats.compile_handle = Arc::as_ptr(&artifact) as usize;
        }
        Ok(materialize_compile_output(&analysis, &artifact))
    }

    fn check_file_with_snapshot(
        &mut self,
        path: &Path,
        overlays: &BTreeMap<PathBuf, String>,
        requested_target: Option<CompileTarget>,
    ) -> Result<CheckOutput, CompilerDiagnostic> {
        let (identity, analysis) = self.analyze_file(path, overlays, requested_target)?;
        if self
            .pipelines
            .get(&identity)
            .is_some_and(|pipeline| pipeline.check_ready)
        {
            #[cfg(test)]
            {
                self.stats.check_hits = self.stats.check_hits.saturating_add(1);
            }
            return Ok(materialize_check_output(&analysis));
        }
        self.lower(&identity, &analysis)?;
        #[cfg(test)]
        {
            self.stats.check_runs = self.stats.check_runs.saturating_add(1);
        }
        let result = materialize_check_output(&analysis);
        self.pipelines
            .get_mut(&identity)
            .expect("分析成功后必须保留对应流水线")
            .check_ready = true;
        Ok(result)
    }

    fn analyze_file(
        &mut self,
        path: &Path,
        overlays: &BTreeMap<PathBuf, String>,
        requested_target: Option<CompileTarget>,
    ) -> Result<(PipelineIdentity, Arc<AnalyzedUnit>), CompilerDiagnostic> {
        let root = normalized_overlay_path(path);
        self.source_cache.begin_request();
        let resolved = resolve_file_with_overlays_cached(
            path,
            overlays,
            &mut self.source_cache,
            requested_target == Some(CompileTarget::Items),
        );
        let touched_paths = self.source_cache.finish_request();
        let resolved = match resolved {
            Ok(resolved) => {
                self.failed_roots.remove(&root);
                resolved
            }
            Err(diagnostic) => {
                self.failed_roots.insert(root, touched_paths);
                self.prune_source_cache();
                return Err(CompilerDiagnostic::from_import(diagnostic));
            }
        };
        let target = requested_target.unwrap_or_else(|| inferred_target(&resolved.document));
        let identity = PipelineIdentity { root, target };
        let compilation_key = CompilationKey::new(&resolved.source_graph, target);
        if let Some(cached) = self
            .pipelines
            .get(&identity)
            .filter(|pipeline| pipeline.compilation_key == compilation_key)
        {
            #[cfg(test)]
            {
                self.stats.analysis_hits = self.stats.analysis_hits.saturating_add(1);
            }
            let analysis = cached.analysis.clone()?;
            #[cfg(test)]
            {
                self.stats.analysis_handle = Arc::as_ptr(&analysis) as usize;
            }
            return Ok((identity, analysis));
        }

        #[cfg(test)]
        {
            self.stats.analysis_runs = self.stats.analysis_runs.saturating_add(1);
        }
        let tracked_files = resolved.tracked_files.clone();
        let analysis = analyze_resolved_file(resolved, path, Some(target)).map(Arc::new);
        self.pipelines.insert(
            identity.clone(),
            CachedPipeline {
                compilation_key,
                tracked_files,
                analysis: analysis.clone(),
                lowering: None,
                compile: None,
                check_ready: false,
            },
        );
        self.prune_source_cache();
        let analysis = analysis?;
        #[cfg(test)]
        {
            self.stats.analysis_handle = Arc::as_ptr(&analysis) as usize;
        }
        Ok((identity, analysis))
    }

    fn lower(
        &mut self,
        identity: &PipelineIdentity,
        analysis: &AnalyzedUnit,
    ) -> Result<Arc<RustUiPlan>, CompilerDiagnostic> {
        if let Some(cached) = self
            .pipelines
            .get(identity)
            .and_then(|pipeline| pipeline.lowering.as_ref())
        {
            #[cfg(test)]
            {
                self.stats.lowering_hits = self.stats.lowering_hits.saturating_add(1);
            }
            let plan = cached.clone()?;
            #[cfg(test)]
            {
                self.stats.lowering_handle = Arc::as_ptr(&plan) as usize;
            }
            return Ok(plan);
        }
        #[cfg(test)]
        {
            self.stats.lowering_runs = self.stats.lowering_runs.saturating_add(1);
        }
        let result = lower_analyzed(analysis).map(Arc::new);
        self.pipelines
            .get_mut(identity)
            .expect("分析成功后必须保留对应流水线")
            .lowering = Some(result.clone());
        let plan = result?;
        #[cfg(test)]
        {
            self.stats.lowering_handle = Arc::as_ptr(&plan) as usize;
        }
        Ok(plan)
    }

    fn prune_source_cache(&mut self) {
        let retained = self
            .pipelines
            .values()
            .flat_map(|pipeline| pipeline.tracked_files.iter().cloned())
            .chain(
                self.failed_roots
                    .values()
                    .flat_map(|paths| paths.iter().cloned()),
            )
            .collect::<BTreeSet<_>>();
        self.source_cache.retain_paths(&retained);
    }

    // 返回确定性阶段执行统计，仅用于证明缓存命中，不进入公开 API。
    #[cfg(test)]
    fn test_stats(&self) -> CompilerSessionStats {
        CompilerSessionStats {
            source_parses: self.source_cache.parse_runs(),
            source_files: self.source_cache.len(),
            ..self.stats
        }
    }
}

// 保存测试可观测的实际阶段执行与缓存命中次数。
#[cfg(test)]
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
struct CompilerSessionStats {
    source_parses: usize,
    source_files: usize,
    analysis_runs: usize,
    analysis_hits: usize,
    analysis_handle: usize,
    lowering_runs: usize,
    lowering_hits: usize,
    lowering_handle: usize,
    compile_runs: usize,
    compile_hits: usize,
    compile_handle: usize,
    check_runs: usize,
    check_hits: usize,
}

#[cfg(test)]
mod tests {
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
}

//! 可选的开发期纯 UI 热重载入口。
//!
//! 本 Adapter 只接收调用方提供的源码快照并重新运行共享 AOT 编译链。它不监视
//! 文件、不解释 `TypedUiIr`、不加载生成代码，也不执行 UI 回调或业务逻辑。

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::uix_import::normalized_overlay_path;
use crate::{CompilationKey, CompileOutput, CompileTarget, CompilerDiagnostic, CompilerSession};

/// 表示一次开发期 AOT 重编译是否产生新的确定性 UI 结果。
#[derive(Debug)]
pub enum HotReloadResult {
    /// 新源码图已通过完整语义与 Rust Emitter 门禁，可交给外部开发工具处理。
    Ready(CompileOutput),
    /// 源码图与上次成功结果相同，外部工具无需重复处理。
    Unchanged { compilation_key: CompilationKey },
}

/// 持有一个根 UI 的开发会话快照与最后一次成功编译身份。
///
/// 文件监视、进程重启和产物加载由外部开发工具负责；本类型不取得这些生命周期。
#[derive(Debug)]
pub struct HotReloadAdapter {
    root: PathBuf,
    overlays: BTreeMap<PathBuf, String>,
    last_ready_key: Option<CompilationKey>,
    // 会话拥有阶段缓存；关闭 Adapter 即释放全部解析、语义与生成产物。
    compiler: CompilerSession,
}

impl HotReloadAdapter {
    /// 创建尚未编译、没有内存 overlay 的开发会话。
    pub fn new(root: impl AsRef<Path>) -> Self {
        Self {
            root: normalized_overlay_path(root.as_ref()),
            overlays: BTreeMap::new(),
            last_ready_key: None,
            compiler: CompilerSession::new(),
        }
    }

    /// 新增或替换一个未落盘 UIX 文件快照。
    pub fn set_overlay(
        &mut self,
        path: impl AsRef<Path>,
        source: impl Into<String>,
    ) -> Option<String> {
        self.overlays
            .insert(normalized_overlay_path(path.as_ref()), source.into())
    }

    /// 移除一个未落盘快照，后续重编译重新读取磁盘内容。
    pub fn remove_overlay(&mut self, path: impl AsRef<Path>) -> Option<String> {
        self.overlays
            .remove(&normalized_overlay_path(path.as_ref()))
    }

    /// 清空全部未落盘快照，保留最后一次成功编译身份用于失效判定。
    pub fn clear_overlays(&mut self) {
        self.overlays.clear();
    }

    /// 返回当前会话保存的未落盘文件数量。
    pub fn overlay_count(&self) -> usize {
        self.overlays.len()
    }

    /// 返回最后一次通过全部 AOT 门禁的编译身份。
    pub fn last_ready_key(&self) -> Option<&CompilationKey> {
        self.last_ready_key.as_ref()
    }

    /// 使用当前 overlay 运行完整共享 AOT 编译链。
    ///
    /// 失败不会覆盖最后一次成功身份；相同身份返回 `Unchanged`。
    pub fn reload(&mut self) -> Result<HotReloadResult, CompilerDiagnostic> {
        let output = self.compiler.compile_file_with_overlays(
            &self.root,
            &self.overlays,
            CompileTarget::View,
        )?;
        if self.last_ready_key.as_ref() == Some(&output.compilation_key) {
            return Ok(HotReloadResult::Unchanged {
                compilation_key: output.compilation_key,
            });
        }
        self.last_ready_key = Some(output.compilation_key.clone());
        Ok(HotReloadResult::Ready(output))
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::{HotReloadAdapter, HotReloadResult};

    fn import_fixture() -> (PathBuf, PathBuf) {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../tests/fixtures/uix_lang/imports/root.uix");
        let helper = root
            .parent()
            .expect("根文件必须有父目录")
            .join("shared/helper.uix");
        (root, helper)
    }

    fn ready(result: HotReloadResult) -> crate::CompileOutput {
        match result {
            HotReloadResult::Ready(output) => output,
            HotReloadResult::Unchanged { .. } => panic!("预期新的 AOT UI 结果"),
        }
    }

    #[test]
    fn recursive_overlay_invalidates_once_without_changing_root_identity() {
        let (root, helper) = import_fixture();
        let mut adapter = HotReloadAdapter::new(root);
        let initial = ready(adapter.reload().expect("初始 UI 必须可编译"));

        let unchanged = adapter.reload().expect("相同源码图必须可重复检查");
        assert!(matches!(unchanged, HotReloadResult::Unchanged { .. }));

        adapter.set_overlay(
            helper,
            "@export('Helper')\n<Widget name=\"Helper\"><Text>热更新依赖</Text></Widget>\n<Helper />",
        );
        let changed = ready(adapter.reload().expect("递归依赖 overlay 必须可编译"));
        assert_eq!(
            initial.compilation_key.root_content_hash,
            changed.compilation_key.root_content_hash
        );
        assert_ne!(
            initial.compilation_key.dependency_hash,
            changed.compilation_key.dependency_hash
        );
    }

    #[test]
    fn failed_overlay_preserves_last_successful_identity() {
        let (root, helper) = import_fixture();
        let mut adapter = HotReloadAdapter::new(root);
        let initial = ready(adapter.reload().expect("初始 UI 必须可编译"));
        let initial_key = initial.compilation_key.clone();

        adapter.set_overlay(
            &helper,
            "@export('Helper')\n<Widget name=\"Helper\"><Mystery /></Widget>\n<Helper />",
        );
        assert!(adapter.reload().is_err());
        assert_eq!(adapter.last_ready_key(), Some(&initial_key));

        assert!(adapter.remove_overlay(&helper).is_some());
        assert_eq!(adapter.overlay_count(), 0);
        let recovered = adapter.reload().expect("移除无效 overlay 后必须恢复");
        assert!(matches!(recovered, HotReloadResult::Unchanged { .. }));
    }
}

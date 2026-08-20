"""验证 PipelineBinding 只能由共享资源表按真实语义签发。"""

# 引入标准单元测试框架。
import unittest
# 引入仓库路径解析。
from pathlib import Path

# 保存仓库根目录，避免测试依赖调用方工作目录。
ROOT = Path(__file__).resolve().parents[1]


# 冻结共享 pipeline 身份签发与校验边界。
class PipelineBindingResourceContractTests(unittest.TestCase):
    # 生产构造器必须只对共享 RHI Module 可见。
    def test_shared_resource_table_is_the_only_production_issuer(self) -> None:
        # 读取不可拆的 pipeline 身份定义。
        pipeline = (ROOT / "src/native/presentation/rhi/pipeline.rs").read_text(encoding="utf-8")
        # 读取唯一保存真实 kind 与 Adapter 私有资源的共享表。
        resource_table = (ROOT / "src/native/presentation/rhi/pipeline_resource_table.rs").read_text(encoding="utf-8")
        # 生产构造器只能由共享 RHI Module 调用。
        self.assertIn("pub(super) const fn new(", pipeline)
        # 可控伪造入口必须显式限制为单元测试构建。
        self.assertIn("#[cfg(test)]", pipeline)
        # 测试入口必须继续限制在 crate 内部。
        self.assertIn("pub(crate) const fn for_test(", pipeline)
        # 共享表必须同时保存原生资源及其真实 PipelineKind。
        self.assertIn("RhiResourceTable<PipelineHandle, Entry<T>>", resource_table)
        # 插入资源必须是生产绑定的唯一签发入口。
        self.assertIn("PipelineBinding::new(handle, kind)", resource_table)
        # 查询和销毁都必须先按完整 binding 核对真实 kind。
        self.assertIn("self.entries.get(binding.handle())?", resource_table)
        # 错误 kind 必须在共享表内统一拒绝。
        self.assertIn("entry.kind != binding.kind()", resource_table)
        # 关闭路径必须从同一资源表逆序取得资源，不能维护平行身份清单。
        self.assertIn("self.entries.drain_reverse().map(|entry| entry.resource)", resource_table)


# 允许直接运行本文件进行最小契约验证。
if __name__ == "__main__":
    # 交给标准 unittest runner 执行。
    unittest.main()

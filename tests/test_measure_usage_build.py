"""验证使用方构建采集器的解析图归一化。"""

# 导入标准单元测试框架。
import unittest

# 导入被测采集模块。
from scripts import measure_usage_build


# 覆盖 capability 依赖断言使用的解析图辅助函数。
class ResolvedGraphTests(unittest.TestCase):
    # 构造同时包含激活与未激活 package 的 Cargo metadata 摘要。
    def metadata(self) -> dict:
        # 返回两个实际节点和一个仅存在于清单信息中的 optional package。
        return {
            "packages": [
                {"id": "root", "name": "root", "version": "1.0.0", "source": None},
                {"id": "active", "name": "active", "version": "2.0.0", "source": "registry"},
                {"id": "optional", "name": "optional", "version": "3.0.0", "source": "registry"},
            ],
            "resolve": {
                "nodes": [
                    {
                        "id": "root",
                        "deps": [{"name": "active", "pkg": "active"}],
                        "features": ["selected"],
                    },
                    {"id": "active", "deps": [], "features": []},
                ]
            },
        }

    # 确认依赖存在性断言只观察实际 resolve 节点。
    def test_resolved_package_names_exclude_inactive_optional_package(self) -> None:
        # 提取实际解析到的 package 名称。
        names = measure_usage_build.resolved_package_names(self.metadata())
        # 未激活的 optional package 不得进入结果。
        self.assertEqual(names, {"root", "active"})

    # 确认结构化摘要使用实际依赖边与启用 feature。
    def test_resolved_graph_summary_uses_resolve_edges(self) -> None:
        # 生成稳定排序的解析图摘要。
        summary = measure_usage_build.resolved_graph_summary(self.metadata())
        # 摘要的 package 和节点数量必须一致。
        self.assertEqual(summary["package_count"], 2)
        # 取出根 package 的实际解析记录。
        root = next(package for package in summary["packages"] if package["name"] == "root")
        # 根 package 只记录实际激活的依赖边。
        self.assertEqual(root["dependencies"], ["active"])
        # 根 package 同时记录 Cargo 选择的 feature。
        self.assertEqual(root["features"], ["selected"])

    # 确认 compile-fail 门禁同时检查所有必需诊断片段。
    def test_missing_error_fragments_reports_incomplete_diagnostics(self) -> None:
        # 构造只包含类型名、不包含错误类别的编译诊断。
        result = {"_stdout": "", "_stderr": "QRCode is unavailable"}
        # 检查两个预期诊断片段。
        missing = measure_usage_build.missing_error_fragments(
            result, ("unresolved import", "QRCode")
        )
        # 仅未出现的错误类别必须被报告。
        self.assertEqual(missing, ["unresolved import"])


# 允许直接运行本文件执行测试。
if __name__ == "__main__":
    # 把 unittest 退出状态传递给调用方。
    unittest.main()

# 声明测试文件使用 UTF-8 编码。
# -*- coding: utf-8 -*-
# 说明本测试守护文档编译围栏与外部消费者登记的一一对应关系。
"""Guard rust uix-compile fence coverage by public API consumer tests."""

# 启用延迟解析类型标注。
from __future__ import annotations

# 引入计数器以发现重复标识。
from collections import Counter
# 引入正则表达式以解析两侧稳定标识。
import re
# 引入标准库单元测试框架。
import unittest
# 引入跨平台路径类型。
from pathlib import Path


# 定位仓库根目录。
ROOT = Path(__file__).resolve().parents[1]
# 定位文档权威目录。
DOCS_ROOT = ROOT / "docs"
# 定位公开 API 消费者目录。
TESTS_ROOT = ROOT / "tests"
# 识别 Markdown Rust 围栏声明的编译标识。
DOC_COMPILE_ID = re.compile(r"\buix-compile=([A-Za-z0-9_-]+)\b")
# 识别外部消费者模块登记的编译标识。
CONSUMER_COMPILE_ID = re.compile(r'COMPILE_ID:\s*&str\s*=\s*"([^"]+)"')


# 从稳定排序的文件集合中收集全部匹配标识。
def collect_compile_ids(paths: list[Path], pattern: re.Pattern[str]) -> list[str]:
    # 保存原始顺序不去重，以便准确发现重复声明。
    compile_ids: list[str] = []
    # 逐个读取参与当前契约的文件。
    for path in sorted(paths):
        # 使用 UTF-8 读取仓库文本事实。
        source = path.read_text(encoding="utf-8")
        # 按源码顺序追加当前文件中的全部标识。
        compile_ids.extend(match.group(1) for match in pattern.finditer(source))
    # 返回完整标识序列。
    return compile_ids


# 返回稳定排序的重复标识及其出现次数。
def duplicate_compile_ids(compile_ids: list[str]) -> list[str]:
    # 统计每个标识的声明次数。
    counts = Counter(compile_ids)
    # 生成包含次数的稳定诊断。
    return sorted(f"{compile_id} ({count})" for compile_id, count in counts.items() if count > 1)


# 定义文档编译围栏覆盖契约。
class DocsCompileCoverageTests(unittest.TestCase):
    # 确认每个文档围栏恰好由一个外部消费者登记。
    def test_document_compile_ids_match_public_api_consumers(self) -> None:
        # 收集全部 Markdown 文档文件。
        doc_paths = list(DOCS_ROOT.rglob("*.md"))
        # 收集约定命名的 Rust 文档消费者入口。
        consumer_paths = list(TESTS_ROOT.glob("*docs_public_api.rs"))
        # 从文档权威侧提取围栏标识。
        doc_ids = collect_compile_ids(doc_paths, DOC_COMPILE_ID)
        # 从消费者事实侧提取登记标识。
        consumer_ids = collect_compile_ids(consumer_paths, CONSUMER_COMPILE_ID)
        # 文档必须至少声明一个真实编译围栏。
        self.assertNotEqual(doc_ids, [], "docs 中没有发现任何 uix-compile 围栏")
        # 消费者目录必须至少登记一个编译标识。
        self.assertNotEqual(consumer_ids, [], "tests 中没有发现任何文档消费者 COMPILE_ID")
        # 同一文档标识只能由一个围栏拥有。
        self.assertEqual(
            # 输出文档侧重复标识。
            duplicate_compile_ids(doc_ids),
            # 空列表表示无重复。
            [],
            # 给出稳定且可直接搜索的错误说明。
            "文档 uix-compile 标识重复",
        )
        # 同一消费者标识只能由一个测试模块拥有。
        self.assertEqual(
            # 输出消费者侧重复标识。
            duplicate_compile_ids(consumer_ids),
            # 空列表表示无重复。
            [],
            # 给出稳定且可直接搜索的错误说明。
            "消费者 COMPILE_ID 标识重复",
        )
        # 计算尚未登记消费者的文档标识。
        missing_consumers = sorted(set(doc_ids) - set(consumer_ids))
        # 计算已经失去文档权威来源的多余消费者标识。
        extra_consumers = sorted(set(consumer_ids) - set(doc_ids))
        # 两侧必须严格一一对应，并在失败时同时报告双向差异。
        self.assertEqual(
            # 使用具名元组保持诊断字段稳定。
            (missing_consumers, extra_consumers),
            # 空差异表示覆盖完整。
            ([], []),
            # 报告两侧总量以辅助定位批量遗漏。
            f"文档/消费者标识不一致：docs={len(doc_ids)} consumers={len(consumer_ids)}",
        )


# 允许直接执行当前测试文件。
if __name__ == "__main__":
    # 使用标准 unittest 入口执行全部用例。
    unittest.main()

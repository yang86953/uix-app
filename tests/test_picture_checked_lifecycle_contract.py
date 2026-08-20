# -*- coding: utf-8 -*-
# 说明本文件锁定 Picture 绘制生命周期的 checked-only 生产契约。
"""Keep Picture paint lifecycle failures on checked renderer boundaries."""
# 启用现代 Python 注解求值规则。
from __future__ import annotations
# 引入标准单元测试框架。
import unittest
# 引入稳定的源码路径操作。
from pathlib import Path
# 定位仓库根目录。
ROOT = Path(__file__).resolve().parents[1]
# 登记已经退出生产依赖图的未检查方法名。
UNCHECKED_METHODS = ("create_offscreen", "destroy_offscreen", "begin_offscreen_paint", "flush_offscreen_paint", "end_offscreen_paint", "blit_offscreen", "blit_offscreen_src")
# 登记必须继续存在的 checked 方法名。
CHECKED_METHODS = ("try_create_offscreen", "try_destroy_offscreen", "try_begin_offscreen_paint", "try_flush_offscreen_paint", "try_end_offscreen_paint", "try_blit_offscreen_src")
# 定义 Picture 生命周期源码门禁。
class PictureCheckedLifecycleContractTests(unittest.TestCase):
    # 验证生产 draw 源码只保留 typed Result 生命周期。
    def test_picture_paint_lifecycle_has_no_unchecked_entry_or_call(self) -> None:
        # 按稳定路径顺序读取全部 draw Rust 源码。
        source = "\n".join(path.read_text(encoding="utf-8") for path in sorted((ROOT / "src/draw").rglob("*.rs")))
        # 逐项拒绝未检查 trait、实现或固有方法定义。
        for method in UNCHECKED_METHODS:
            # 未检查定义不得重新进入生产依赖图。
            self.assertNotIn(f"fn {method}(", source)
            # 未检查调用不得绕过 typed failure 边界。
            self.assertNotIn(f".{method}(", source)
        # 逐项确认 checked 公共边界没有被一并删除。
        for method in CHECKED_METHODS:
            # checked 方法必须继续由生产源码定义。
            self.assertIn(f"fn {method}(", source)
    # 验证最终 present 在主 surface 提交前检查式结束 Picture。
    def test_gpu_present_uses_checked_picture_end(self) -> None:
        # 读取 GPU 最终 present 状态机。
        source = (ROOT / "src/draw/backend/gpu/execution/render_present.rs").read_text(encoding="utf-8")
        # active Picture 必须通过 Result 边界结束并传播失败。
        self.assertIn("self.try_end_offscreen_paint()?;", source)
    # 验证 GPU Picture 创建不再把 RHI 资源失败降成 None。
    def test_gpu_picture_create_propagates_typed_rhi_failures(self) -> None:
        # 读取 GPU RenderBackend 的唯一 Picture 资源 owner 实现。
        source = (ROOT / "src/draw/backend/gpu/execution/render_backend.rs").read_text(encoding="utf-8")
        # 定位检查式创建事务的开始边界。
        start = source.index("fn try_create_offscreen")
        # 定位紧随其后的检查式销毁事务。
        end = source.index("fn try_destroy_offscreen", start)
        # 只审计创建资源的生产代码片段。
        create = source[start:end]
        # owner-thread context 失败必须直接传播。
        self.assertIn("let context = self.gpu_ctx.rhi_context()?;", create)
        # texture 创建结果必须用问号传播 typed RHI 错误。
        self.assertIn("})?;", create)
        # 禁止把资源错误重新折叠为 Option。
        self.assertNotIn(".ok()?", create)
        # 禁止只记录日志后把错误伪装成普通缓存不可用。
        self.assertNotIn("tracing::warn!", create)
# 支持直接运行本门禁文件。
if __name__ == "__main__":
    # 交给标准测试运行器执行。
    unittest.main()

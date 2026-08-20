# 使用路径对象读取 Application 窗口创建与 FileDrop 策略契约。
from pathlib import Path
# 标准库测试框架保持聚焦验证无额外依赖。
import unittest

# 解析仓库根目录。
ROOT = Path(__file__).resolve().parents[1]
# 窗口创建 Adapter 是 FileDrop 产品策略唯一 owner。
WINDOW_CREATION = ROOT / "src/app/window/window_creation.rs"
# 主窗创建入口。
APPLICATION = ROOT / "src/app/application/application/mod.rs"
# 运行时次窗创建入口。
SECONDARY_CREATE = ROOT / "src/app/application/lifecycle/runtime/create.rs"
# 公开独立 Window 创建入口。
PUBLIC_WINDOW = ROOT / "src/app/window/window.rs"
# 验证所有 Application 窗口统一启用 FileDrop，同时保持平台能力缺失兼容。
class AppFileDropWindowCreationTests(unittest.TestCase):
    # Adapter 必须在发布窗口 owner 前调用 enable_file_drop(true)。
    def test_factory_enables_file_drop_after_native_creation(self) -> None:
        # 读取唯一窗口创建 Adapter。
        source = WINDOW_CREATION.read_text(encoding="utf-8")
        # 原生创建必须先发生。
        create = source.index("window_manager.create_window")
        # 随后启用产品能力。
        enable = source.index("enable_file_drop(true)", create)
        # 成功后才向调用方发布窗口。
        publish = source.index("Ok(_) => Ok(window)", enable)
        # 验证单向创建链。
        self.assertLess(create, enable)
        self.assertLess(enable, publish)
        # 能力入口只允许调用一次。
        self.assertEqual(source.count("enable_file_drop(true)"), 1)

    # 只有稳定 NotImplemented 可以降级继续创建。
    def test_factory_tolerates_only_stable_capability_absence(self) -> None:
        # 读取结果分类与创建回滚。
        source = WINDOW_CREATION.read_text(encoding="utf-8")
        # 限定纯分类函数。
        classify_start = source.index("fn classify_file_drop_enable")
        # 测试模块标记分类函数末尾。
        classify_end = source.index("#[cfg(test)]", classify_start)
        # 保存分类实现。
        classify = source[classify_start:classify_end]
        # NotImplemented 必须映射为健康 false。
        self.assertIn("Errc::NotImplemented => Ok(false)", classify)
        # 其他错误必须原样传播。
        self.assertIn("Err(error) => Err(error)", classify)
        # 分类不得依赖日志文本或平台名称。
        self.assertNotIn("tracing::", classify)

    # 真实启用失败必须关闭半配置窗口并保留清理原因链。
    def test_factory_rolls_back_real_enable_failure(self) -> None:
        # 读取创建事务。
        source = WINDOW_CREATION.read_text(encoding="utf-8")
        # 真实失败分支必须执行 close。
        close = source.index("match window.close()")
        # 清理失败必须保存原始启用失败。
        source_chain = source.index("cleanup_error.with_source(primary_error)", close)
        # 先清理再组装原因链。
        self.assertLess(close, source_chain)
        # Adapter 不得用 warn 替代失败传播。
        self.assertNotIn("tracing::", source)

    # 三类 Application 窗口必须全部复用同一 Adapter。
    def test_main_secondary_and_public_window_share_factory(self) -> None:
        # 读取三个创建入口。
        sources = {
            # 主窗口组合根。
            "main": APPLICATION.read_text(encoding="utf-8"),
            # 运行时次窗。
            "secondary": SECONDARY_CREATE.read_text(encoding="utf-8"),
            # 公开独立窗口。
            "public": PUBLIC_WINDOW.read_text(encoding="utf-8"),
        }
        # 每个入口必须且只能调用一次统一工厂。
        for name, source in sources.items():
            # 错误消息携带入口名称。
            self.assertEqual(source.count("create_app_window("), 1, name)
        # 三个入口不得绕过 Adapter 直接调用原生窗口工厂。
        self.assertNotIn("window_manager().create_window", sources["main"])
        self.assertNotIn("window_manager().create_window", sources["secondary"])
        self.assertNotIn("window_manager().create_window", sources["public"])

    # 本任务涉及文件必须保持项目 900 行门槛。
    def test_touched_files_stay_within_limit(self) -> None:
        # 逐一检查工厂、三个调用点与聚焦测试。
        for path in (
            # 唯一创建 Adapter。
            WINDOW_CREATION,
            # 主窗组合根。
            APPLICATION,
            # 次窗创建入口。
            SECONDARY_CREATE,
            # 公开 Window 入口。
            PUBLIC_WINDOW,
            # 本聚焦契约测试。
            Path(__file__),
        ):
            # 计算物理行数。
            line_count = len(path.read_text(encoding="utf-8").splitlines())
            # 文件不得超过 900 行。
            self.assertLessEqual(line_count, 900, path)


# 允许直接运行本聚焦测试文件。
if __name__ == "__main__":
    # 使用标准 unittest runner 输出精确用例数。
    unittest.main()

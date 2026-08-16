# 使用路径对象读取仓库内的 Wayland window factory 生命周期契约。
from pathlib import Path
# 使用标准库测试框架保持聚焦验证无额外依赖。
import unittest

# 解析测试文件所在仓库根目录。
ROOT = Path(__file__).resolve().parents[1]
# 定位 Wayland 窗口工厂 Module。
WINDOW_FACTORY = ROOT / "src/native/backends/linux/wayland/window.rs"


# 验证 closed Wayland backend 不会通过窗口工厂复活协议状态。
class WaylandWindowFactoryClosedGateTests(unittest.TestCase):
    # 截取 create_window 实现。
    def factory_source(self) -> str:
        # 读取窗口工厂 Module。
        source = WINDOW_FACTORY.read_text(encoding="utf-8")
        # 定位 trait 入口。
        start = source.index("fn create_window")
        # 文件仅包含该 trait 实现，使用末尾作为边界。
        return source[start:]

    # 确认 closed gate 是首个 backend owner 决策。
    def test_closed_gate_precedes_seat_and_identity_mutation(self) -> None:
        # 截取窗口工厂实现。
        factory = self.factory_source()
        # 定位 closed 事实读取。
        closed = factory.index("if self.closed")
        # 定位 seat/input capability 建立。
        seat = factory.index("self.ensure_seat_and_input()")
        # 定位窗口身份读取。
        identity = factory.index("WindowId::new(self.next_window_id)")
        # 定位窗口身份推进。
        increment = factory.index("self.next_window_id += 1")
        # closed gate 必须先于 seat/input owner 访问。
        self.assertLess(closed, seat)
        # closed gate 必须先于 WindowId 发布。
        self.assertLess(closed, identity)
        # closed gate 必须先于身份序号推进。
        self.assertLess(closed, increment)

    # 确认关闭分支返回稳定 typed lifecycle error。
    def test_closed_gate_returns_invalid_state(self) -> None:
        # 截取窗口工厂实现。
        factory = self.factory_source()
        # 以 seat 建立标记限定 closed 分支。
        end = factory.index("self.ensure_seat_and_input()")
        # 保存 gate 片段。
        gate = factory[:end]
        # closed 分支必须返回 InvalidState。
        self.assertIn("Errc::InvalidState", gate)
        # 诊断必须保留 create_window 上下文。
        self.assertIn("Wayland create_window requested after backend shutdown", gate)
        # gate 不得伪装成功。
        self.assertNotIn("Ok(", gate)
        # gate 不得入队重复失败。
        self.assertNotIn("enqueue", gate)
        # gate 不得执行协议 roundtrip。
        self.assertNotIn("roundtrip", gate)

    # 确认健康创建事务保持既有 owner 链。
    def test_healthy_factory_keeps_existing_owner_chain(self) -> None:
        # 截取窗口工厂实现。
        factory = self.factory_source()
        # 健康路径仍必须初始化逐窗协议 owner。
        self.assertIn("ops.init(", factory)
        # presenter 仍与同一 surface owner 组装。
        self.assertIn("WaylandPresenter::new", factory)
        # 共享窗口 core 仍最终接管 state/ops/presenter。
        self.assertIn("PlatformWindowCore::new(state, ops, Box::new(presenter))", factory)

    # 确认本任务涉及文件满足项目规模门槛。
    def test_touched_files_stay_within_limit(self) -> None:
        # 逐一检查本任务修改的源码与测试文件。
        for path in (WINDOW_FACTORY, Path(__file__)):
            # 计算当前文件物理行数。
            line_count = len(path.read_text(encoding="utf-8").splitlines())
            # 文件必须保持不超过 900 行。
            self.assertLessEqual(line_count, 900, path)


# 允许直接运行本聚焦测试文件。
if __name__ == "__main__":
    # 使用标准 unittest runner 输出精确用例数。
    unittest.main()

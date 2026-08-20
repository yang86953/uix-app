# 使用路径对象读取仓库内的 Wayland text-input 生命周期契约。
from pathlib import Path
# 使用标准库测试框架保持聚焦验证无额外依赖。
import unittest

# 解析测试文件所在仓库根目录。
ROOT = Path(__file__).resolve().parents[1]
# 定位 text-input Module。
TEXT_INPUT = ROOT / "src/native/backends/linux/windowing/wayland/text_input.rs"


# 验证 closed backend 不再允许 IME session 状态复活。
class WaylandTextInputClosedGateTests(unittest.TestCase):
    # 确认 lifecycle gate 只读取 closed 事实并返回 typed error。
    def test_gate_rejects_closed_backend_without_owner_side_effects(self) -> None:
        # 读取 text-input Module。
        source = TEXT_INPUT.read_text(encoding="utf-8")
        # 限定 lifecycle gate。
        gate_start = source.index("fn ensure_text_input_open")
        # shutdown 端口标记 gate 末尾。
        gate_end = source.index("pub(crate) fn shutdown_text_input", gate_start)
        # 保存 gate 片段。
        gate = source[gate_start:gate_end]
        # gate 必须检查 backend owner 的 closed 事实。
        self.assertIn("if self.closed", gate)
        # closed 必须返回稳定 InvalidState。
        self.assertIn("Errc::InvalidState", gate)
        # 诊断必须保留调用 operation。
        self.assertIn("Wayland text_input {operation} requested after backend shutdown", gate)
        # 健康 backend 必须继续返回成功。
        self.assertIn("Ok(())", gate)
        # gate 不得取得任何共享 owner lock。
        self.assertNotIn(".lock()", gate)
        # gate 不得推进 generation 或 enabled 原子状态。
        self.assertNotIn("Ordering::", gate)
        # gate 不得向 pending source 入队副作用。
        self.assertNotIn("enqueue", gate)
        # gate 不得修改 closed 事实。
        self.assertNotIn("self.closed =", gate)

    # 确认四个 trait 入口都把 lifecycle gate 作为首个 owner 决策。
    def test_all_text_input_entries_check_gate_first(self) -> None:
        # 读取 text-input Module。
        source = TEXT_INPUT.read_text(encoding="utf-8")
        # 描述每个入口、下一个边界、operation 与首次既有 owner 访问。
        entries = [
            # 目标窗口设置入口。
            (
                # 当前入口标记。
                "fn set_target_window",
                # 下一个入口标记。
                "fn start(&mut self)",
                # gate operation。
                "set_target_window",
                # 首次状态写入。
                "self.text_input_window_id = Some(window_id)",
            ),
            # session start 入口。
            (
                # 当前入口标记。
                "fn start(&mut self)",
                # 下一个入口标记。
                "fn stop(&mut self)",
                # gate operation。
                "start",
                # 首次目标 owner 读取。
                "let window_id = self.text_input_window_id",
            ),
            # session stop 入口。
            (
                # 当前入口标记。
                "fn stop(&mut self)",
                # 下一个入口标记。
                "fn set_cursor_rect",
                # gate operation。
                "stop",
                # 首次 active owner 读取。
                "let active_window_id = self.active_text_input_window_id",
            ),
            # cursor rect 入口。
            (
                # 当前入口标记。
                "fn set_cursor_rect",
                # 使用文件末尾作为边界。
                None,
                # gate operation。
                "set_cursor_rect",
                # 首次 proxy owner 读取。
                "let Some(text_input) = self.text_input.as_ref()",
            ),
        ]
        # 逐入口验证 gate 先于任何 owner 访问。
        for start_marker, end_marker, operation, first_owner in entries:
            # 定位当前入口起点。
            start = source.index(start_marker)
            # 定位当前入口终点或文件末尾。
            end = source.index(end_marker, start) if end_marker is not None else len(source)
            # 保存单个 trait 入口。
            entry = source[start:end]
            # 定位 operation 对应的 lifecycle gate。
            gate = entry.index(f'self.ensure_text_input_open("{operation}")?')
            # 定位首次既有 owner 访问。
            owner = entry.index(first_owner)
            # gate 必须早于任何状态或协议 owner 访问。
            self.assertLess(gate, owner, operation)

    # 确认 closed gate 不改变健康路径的协议操作集合。
    def test_healthy_paths_keep_existing_protocol_operations(self) -> None:
        # 读取 text-input Module。
        source = TEXT_INPUT.read_text(encoding="utf-8")
        # start 仍必须创建并启用 text-input proxy。
        self.assertIn("manager.get_text_input(seat)", source)
        # start 仍必须提交 enable。
        self.assertIn("ti.enable()", source)
        # stop 仍必须提交 disable。
        self.assertIn("ti.disable()", source)
        # cursor 更新仍必须提交矩形请求。
        self.assertIn("text_input.set_cursor_rectangle", source)

    # 确认本任务涉及文件满足项目规模门槛。
    def test_touched_file_stays_within_limit(self) -> None:
        # 计算 text-input Module 当前物理行数。
        line_count = len(TEXT_INPUT.read_text(encoding="utf-8").splitlines())
        # 文件必须保持不超过 900 行。
        self.assertLessEqual(line_count, 900)


# 允许直接运行本聚焦测试文件。
if __name__ == "__main__":
    # 使用标准 unittest runner 输出精确用例数。
    unittest.main()

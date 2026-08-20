# 使用路径对象读取 Wayland clipboard 同步事务。
from pathlib import Path
# 使用标准库测试框架保持聚焦验证无额外依赖。
import unittest

# 解析测试文件所在仓库根目录。
ROOT = Path(__file__).resolve().parents[1]
# 定位 Wayland clipboard Adapter。
CLIPBOARD = ROOT / "src/native/backends/linux/windowing/wayland/clipboard.rs"


# 验证 set_text 只在真实 selection 请求后提交本地状态。
class WaylandClipboardSetTextTransactionTests(unittest.TestCase):
    # 读取并限定 set_text 实现片段。
    def set_text_source(self) -> str:
        # 读取完整 clipboard Module。
        source = CLIPBOARD.read_text(encoding="utf-8")
        # 定位同步写入入口。
        start = source.index("fn set_text")
        # has_text 标记实现片段终点。
        end = source.index("fn has_text", start)
        # 返回单一事务片段。
        return source[start:end]

    # 确认缺失协议能力返回 typed error 而非本地成功。
    def test_missing_protocol_capabilities_fail_before_owner_changes(self) -> None:
        # 取得被测同步事务。
        set_text = self.set_text_source()
        # manager 缺失必须稳定分类为 NotImplemented。
        self.assertIn("wl_data_device_manager is unavailable", set_text)
        # data device 缺失必须稳定分类为 InvalidOperation。
        self.assertIn("no wl_data_device for active seat", set_text)
        # 两个 capability 检查都必须位于首个共享 owner lock 前。
        owner_lock = set_text.index("self.owns_clipboard.lock()")
        # manager 检查必须先发生。
        self.assertLess(set_text.index("self.data_device_manager"), owner_lock)
        # data device 检查必须先发生。
        self.assertLess(set_text.index("self.data_device.clone()"), owner_lock)
        # capability 分支不得保留空操作成功。
        pre_owner = set_text[:owner_lock]
        # 所有早退都应传播 typed error。
        self.assertNotIn("return Ok(())", pre_owner)

    # 确认全部同步 owner 在协议动作前健康且失败不改旧 selection。
    def test_owner_and_serial_failures_precede_source_replacement(self) -> None:
        # 取得被测同步事务。
        set_text = self.set_text_source()
        # 定位三份同步 owner 检查。
        owns_lock = set_text.index("self.owns_clipboard.lock()")
        # 文本 owner 沿固定顺序随后取得。
        text_lock = set_text.index("self.clipboard_text.lock()")
        # serial owner 最后验证协议授权。
        serial_lock = set_text.index("self.last_input_serial.lock()")
        # 缺少 serial 的 typed error 标记 preflight 完成。
        missing_serial = set_text.index("no pointer or keyboard serial for selection")
        # 旧 source 只能在全部检查成功后替换。
        replace_source = set_text.index("self.clipboard_source.take()")
        # owner 锁序必须稳定。
        self.assertLess(owns_lock, text_lock)
        # 文本 owner 必须早于 serial owner。
        self.assertLess(text_lock, serial_lock)
        # serial 错误路径必须早于旧 source 替换。
        self.assertLess(missing_serial, replace_source)
        # preflight 不得改写 ownership。
        self.assertNotIn("*owns_clipboard = false", set_text[:replace_source])
        # Adapter 不得在返回 typed error 前重复写 WARN。
        self.assertNotIn("tracing::warn!", set_text[:replace_source])

    # 确认协议请求后才发布三份成功状态。
    def test_selection_request_precedes_success_state_commit(self) -> None:
        # 取得被测同步事务。
        set_text = self.set_text_source()
        # data source 创建是首个 selection 协议对象动作。
        create_source = set_text.index("dm.create_data_source()")
        # selection 请求是状态提交边界。
        set_selection = set_text.index("dd.set_selection(Some(&source), serial)")
        # 文本缓存只在请求后更新。
        text_commit = set_text.index("*clipboard_text = text.to_string()")
        # ownership 只在请求后更新。
        owns_commit = set_text.index("*owns_clipboard = true")
        # active source 最后发布。
        source_commit = set_text.index("self.clipboard_source = Some(source)")
        # source 必须在 set_selection 前创建。
        self.assertLess(create_source, set_selection)
        # selection 必须早于文本提交。
        self.assertLess(set_selection, text_commit)
        # 文本提交必须早于 ownership 提交。
        self.assertLess(text_commit, owns_commit)
        # ownership 提交必须早于 active source 发布。
        self.assertLess(owns_commit, source_commit)
        # 既有动态 callback 生命周期必须保持。
        self.assertIn("quick_assign_with_lifecycle", set_text)
        # Cancelled 仍消费 callback owner。
        self.assertIn("CallbackDisposition::Remove", set_text)

    # 确认本任务涉及文件满足项目规模门槛。
    def test_touched_files_stay_within_limit(self) -> None:
        # 逐一检查本任务修改的源码与测试文件。
        for path in (CLIPBOARD, Path(__file__)):
            # 计算当前文件物理行数。
            line_count = len(path.read_text(encoding="utf-8").splitlines())
            # 文件必须保持不超过 900 行。
            self.assertLessEqual(line_count, 900, path)


# 允许直接运行本聚焦测试文件。
if __name__ == "__main__":
    # 使用标准 unittest runner 输出精确用例数。
    unittest.main()

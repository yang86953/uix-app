# 使用路径对象读取仓库内的 Wayland clipboard source 生命周期契约。
from pathlib import Path
# 使用标准库测试框架保持聚焦验证无额外依赖。
import unittest

# 解析测试文件所在仓库根目录。
ROOT = Path(__file__).resolve().parents[1]
# 定位 clipboard Component。
CLIPBOARD = ROOT / "src/native/backends/linux/wayland/clipboard.rs"
# 定位 callback registry compatibility adapter。
COMPAT = ROOT / "src/native/backends/linux/wayland/compat.rs"
# 定位 WaylandBackend owner 字段。
BACKEND = ROOT / "src/native/backends/linux/wayland/mod.rs"


# 验证 data-source callback 随 selection 生命周期消费。
class WaylandClipboardSourceLifecycleTests(unittest.TestCase):
    # 截取 set_text 的 source 建立片段。
    def set_text_source(self) -> str:
        # 读取 clipboard Component。
        source = CLIPBOARD.read_text(encoding="utf-8")
        # 定位同步 set_text 入口。
        start = source.index("fn set_text")
        # 以 has_text 入口作为片段终点。
        end = source.index("fn has_text", start)
        # 返回 source replacement 与 callback 片段。
        return source[start:end]

    # 确认 backend 持有唯一 active source owner。
    def test_backend_owns_single_clipboard_source(self) -> None:
        # 读取 backend 主 Module。
        source = BACKEND.read_text(encoding="utf-8")
        # backend 必须声明单一可选 data-source handle。
        self.assertEqual(
            # 统计稳定字段声明。
            source.count("clipboard_source: Option<Main<wl_data_source::WlDataSource>>"),
            # 只允许一个 owner 槽。
            1,
        )
        # 构造期必须显式初始化为空 owner。
        self.assertIn("clipboard_source: None", source)

    # 确认新 selection 先注销旧 source 再发布新 owner。
    def test_set_text_replaces_source_before_new_publication(self) -> None:
        # 截取 set_text source lifecycle。
        source = self.set_text_source()
        # 定位旧 owner 消费。
        take_previous = source.index("self.clipboard_source.take()")
        # 定位旧 callback 注销。
        clear_previous = source.index("previous.clear_callback()")
        # 定位新协议对象创建。
        create = source.index("dm.create_data_source()")
        # 定位 selection 请求提交。
        selection = source.index("dd.set_selection(Some(&source), serial)")
        # 定位新 active owner 发布。
        publish = source.index("self.clipboard_source = Some(source)")
        # 旧 owner 必须先取出再注销。
        self.assertLess(take_previous, clear_previous)
        # 旧 callback 注销后才创建新 source。
        self.assertLess(clear_previous, create)
        # selection 请求先于本地 owner 发布。
        self.assertLess(selection, publish)

    # 确认 Send 保持 owner 而 Cancelled 消费 owner。
    def test_callback_uses_event_level_disposition(self) -> None:
        # 截取 set_text source lifecycle。
        source = self.set_text_source()
        # 必须使用显式 lifecycle callback API。
        self.assertIn("source.quick_assign_with_lifecycle", source)
        # 定位 Send 分支。
        send = source.index("wl_data_source::Event::Send")
        # 定位 Cancelled 分支。
        cancelled = source.index("wl_data_source::Event::Cancelled")
        # Send 片段必须保留 callback owner。
        send_source = source[send:cancelled]
        # 正常和早退路径都只能返回 Keep。
        self.assertGreaterEqual(send_source.count("CallbackDisposition::Keep"), 2)
        # Cancelled 片段必须失效 ownership。
        cancelled_source = source[cancelled:]
        # 健康 owner 被提交为 false。
        self.assertIn("*owns = false", cancelled_source)
        # poisoned owner 必须进入同一 failure adapter。
        self.assertIn('"owns flag on Cancelled"', cancelled_source)
        # 生命周期终点必须消费 registry owner。
        self.assertIn("CallbackDisposition::Remove", cancelled_source)

    # 确认 compat adapter 只为显式 Remove 阻断回插。
    def test_compat_remove_precedes_persistent_reinsert(self) -> None:
        # 读取 compat Module。
        source = COMPAT.read_text(encoding="utf-8")
        # 普通 quick_assign 必须默认 Keep。
        quick_start = source.index("pub(crate) fn quick_assign<F>")
        # lifecycle API 标记普通 wrapper 终点。
        quick_end = source.index("pub(crate) fn quick_assign_with_lifecycle", quick_start)
        # 保存普通 wrapper。
        quick = source[quick_start:quick_end]
        # 普通调用方不改变持久 callback 语义。
        self.assertIn("CallbackDisposition::Keep", quick)
        # 定位泛型 dispatch adapter。
        dispatch_start = source.index("fn dispatch<I>")
        # Dispatch trait 实现标记片段终点。
        dispatch_end = source.index("impl<I> Dispatch", dispatch_start)
        # 保存 callback 执行与回插片段。
        dispatch = source[dispatch_start:dispatch_end]
        # callback disposition 必须来自 registry lock 外的实际执行结果。
        execute = dispatch.index("callback(&callback_proxy, event, qh)")
        # Remove 判断必须发生在 panic 转换后。
        remove = dispatch.index("CallbackDisposition::Remove")
        # 持久 owner 回插保持在 Remove 分支之后。
        reinsert = dispatch.index('"callback reinsert"')
        # callback 先执行。
        self.assertLess(execute, remove)
        # Remove 必须阻断持久回插。
        self.assertLess(remove, reinsert)

    # 确认 shutdown 在 shared owners 前注销 active source。
    def test_shutdown_clears_source_before_shared_state(self) -> None:
        # 读取 clipboard Component。
        source = CLIPBOARD.read_text(encoding="utf-8")
        # 定位 shutdown 入口。
        start = source.index("pub(crate) fn shutdown_clipboard_io")
        # 以 impl 结束作为片段终点。
        end = source.index("impl IClipboard for WaylandBackend", start)
        # 保存 shutdown 片段。
        shutdown = source[start:end]
        # 定位 active source owner 消费。
        take_source = shutdown.index("self.clipboard_source.take()")
        # 定位 callback 注销。
        clear_source = shutdown.index("source.clear_callback()")
        # 定位首个 shared owner lock。
        first_lock = shutdown.index(".owns_clipboard")
        # source 先取出再注销。
        self.assertLess(take_source, clear_source)
        # callback 必须在 shared state 清理前停止。
        self.assertLess(clear_source, first_lock)

    # 确认本任务涉及文件满足项目规模门槛。
    def test_touched_files_stay_within_limit(self) -> None:
        # 逐一检查本任务修改的源码与测试文件。
        for path in (CLIPBOARD, COMPAT, BACKEND, Path(__file__)):
            # 计算当前文件物理行数。
            line_count = len(path.read_text(encoding="utf-8").splitlines())
            # 文件必须保持不超过 900 行。
            self.assertLessEqual(line_count, 900, path)


# 允许直接运行本聚焦测试文件。
if __name__ == "__main__":
    # 使用标准 unittest runner 输出精确用例数。
    unittest.main()

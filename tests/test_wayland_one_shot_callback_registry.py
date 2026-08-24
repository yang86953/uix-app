# 使用路径对象读取仓库内的 Wayland callback registry 契约。
from pathlib import Path
# 使用标准库测试框架保持聚焦验证无额外依赖。
import unittest

# 解析测试文件所在仓库根目录。
ROOT = Path(__file__).resolve().parents[1]
# 定位 Wayland 0.31 兼容层。
COMPAT = ROOT / "src/native/backends/linux/windowing/wayland/compat.rs"
# 定位逐窗 frame callback 注册入口。
WINDOW_OPS = ROOT / "src/native/backends/linux/windowing/wayland/window_ops.rs"
# 定位 frame request 消费 Component。
FRAME_CALLBACK = ROOT / "src/native/backends/linux/windowing/wayland/frame_callback.rs"


# 验证 one-shot 协议 callback owner 只消费一次。
class WaylandOneShotCallbackRegistryTests(unittest.TestCase):
    # 截取 compat dispatch adapter 源码。
    def dispatch_source(self) -> str:
        # 读取完整兼容层。
        source = COMPAT.read_text(encoding="utf-8")
        # 定位泛型 dispatch adapter 起点。
        start = source.index("fn dispatch<I>")
        # 以 Dispatch trait 实现作为片段终点。
        end = source.index("impl<I> Dispatch", start)
        # 返回单一 dispatch adapter。
        return source[start:end]

    # 确认 wl_callback 在执行和 panic 转换后被消费而非回插。
    def test_wl_callback_returns_before_registry_reinsert(self) -> None:
        # 截取 dispatch adapter。
        dispatch = self.dispatch_source()
        # callback 必须先在 registry 锁外执行。
        callback_call = dispatch.index(
            "(callback_owner.as_mut())(&callback_proxy, event, qh)"
        )
        # panic 必须先转换为 owner-thread failure。
        panic_report = dispatch.index("report_callback_panic::<I>")
        # one-shot 类型判断必须精确匹配 wl_callback 接口。
        one_shot = dispatch.index(
            # 使用完整类型身份表达一次性协议对象。
            "TypeId::of::<I>() == TypeId::of::<wl_callback::WlCallback>()"
        )
        # one-shot 分支必须在持久 owner 回插前结束。
        reinsert = dispatch.index('"callback reinsert"')
        # callback 执行先于 panic 观察。
        self.assertLess(callback_call, panic_report)
        # panic 观察先于生命周期消费判断。
        self.assertLess(panic_report, one_shot)
        # one-shot 分支必须阻断后续回插。
        self.assertLess(one_shot, reinsert)
        # 分支必须显式释放当前 owner。
        branch = dispatch[one_shot:reinsert]
        # 提前返回让局部 callback owner 在此作用域释放。
        self.assertIn("return;", branch)
        # one-shot 消费不得生成额外错误。
        self.assertNotIn("enqueue", branch)
        # one-shot 消费不得发起协议请求。
        self.assertNotIn(".destroy(", branch)

    # 确认持久 callbacks 仍保留原有回插路径。
    def test_persistent_callbacks_keep_registry_reinsert(self) -> None:
        # 截取 dispatch adapter。
        dispatch = self.dispatch_source()
        # 生命周期分支之后必须继续使用同一 registry owner。
        self.assertIn("self.registry.insert(", dispatch)
        # 回插必须使用 dispatch 初始解析的精确对象键。
        self.assertIn("key,", dispatch)
        # 回插必须转移 dispatch 取出的同一个 callback owner。
        self.assertIn("callback_owner,", dispatch)
        # 持久事件不得为回插重新装箱。
        self.assertNotIn("Box::new(callback", dispatch)
        # 生命周期诊断继续保留稳定回插阶段。
        self.assertIn('"callback reinsert"', dispatch)
        # wl_buffer Release 仍是实际持久 callback 使用方。
        source = COMPAT.read_text(encoding="utf-8")
        # 兼容层必须继续声明 wl_buffer 协议接口。
        self.assertIn("wl_buffer", source)

    # 确认 frame 路径仍只把 Done 交给既有 request/event Component。
    def test_frame_request_transaction_is_unchanged(self) -> None:
        # 读取逐窗 frame callback 注册入口。
        window_ops = WINDOW_OPS.read_text(encoding="utf-8")
        # 定位原生 frame 请求入口。
        start = window_ops.index("fn os_request_native_frame")
        # 以取消入口作为片段终点。
        end = window_ops.index("fn os_cancel_native_frame", start)
        # 保存 frame callback 片段。
        frame = window_ops[start:end]
        # callback 仍只处理 wl_callback Done。
        self.assertIn("wl_callback::Event::Done", frame)
        # callback 仍委托检查式 request 消费 Component。
        self.assertIn("deliver_frame_opportunity(", frame)
        # 读取 request/event 事务 Component。
        delivery = FRAME_CALLBACK.read_text(encoding="utf-8")
        # active request 仍必须在事件队列之前清空。
        request_clear = delivery.index("*active = None")
        # 事件仍绑定原窗口身份投递。
        event_commit = delivery.index("events.push_back(")
        # one-shot callback registry 修复不得逆转业务事务顺序。
        self.assertLess(request_clear, event_commit)

    # 确认本任务涉及文件满足项目规模门槛。
    def test_touched_files_stay_within_limit(self) -> None:
        # 逐一检查本任务修改的源码与测试文件。
        for path in (COMPAT, Path(__file__)):
            # 计算当前文件物理行数。
            line_count = len(path.read_text(encoding="utf-8").splitlines())
            # 文件必须保持不超过 900 行。
            self.assertLessEqual(line_count, 900, path)


# 允许直接运行本聚焦测试文件。
if __name__ == "__main__":
    # 使用标准 unittest runner 输出精确用例数。
    unittest.main()

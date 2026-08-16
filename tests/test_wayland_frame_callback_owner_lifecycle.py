# 使用路径对象读取逐窗 frame callback 生命周期契约。
from pathlib import Path
# 使用标准库测试框架保持聚焦验证无额外依赖。
import unittest

# 解析测试文件所在仓库根目录。
ROOT = Path(__file__).resolve().parents[1]
# 定位 frame callback 私有 Component。
FRAME_CALLBACK = ROOT / "src/native/backends/linux/wayland/frame_callback.rs"
# 定位 WindowBackend 编排入口。
WINDOW_OPS = ROOT / "src/native/backends/linux/wayland/window_ops.rs"
# 定位逐窗 callback teardown 编排。
WINDOW_SHUTDOWN = ROOT / "src/native/backends/linux/wayland/window_callback_shutdown.rs"


# 验证 active request 与 wl_callback 由同一 Component 管理。
class WaylandFrameCallbackOwnerLifecycleTests(unittest.TestCase):
    # 读取 frame callback Component。
    def component_source(self) -> str:
        # 返回 UTF-8 源码供聚焦契约解析。
        return FRAME_CALLBACK.read_text(encoding="utf-8")

    # 确认不同 request 替换 callback，相同 request 保持去重。
    def test_prepare_request_owns_and_replaces_protocol_callback(self) -> None:
        # 读取 Component 源码。
        source = self.component_source()
        # Component 必须同时声明业务与协议 owner。
        self.assertIn("struct FrameCallbackOwner", source)
        # active request 保持 callback 闭包共享形状。
        self.assertIn("active: Arc<Mutex<Option<NativeFrameRequest>>>", source)
        # 协议 callback 必须成为显式可选 owner。
        self.assertIn("callback: Option<Main<wl_callback::WlCallback>>", source)
        # 限定 request 登记方法。
        start = source.index("fn prepare_request")
        # active_source 标记登记片段终点。
        end = source.index("fn active_source", start)
        # 保存登记事务。
        prepare = source[start:end]
        # 相同 request 判断必须先于任何 active 写入。
        duplicate = prepare.index("active.as_ref() == Some(&request)")
        # false 明确阻止调用方创建第二个 callback。
        no_create = prepare.index("return Ok(false)", duplicate)
        # 不同 request 才发布新 active 值。
        commit = prepare.index("*active = Some(request)")
        # callback registry 操作前必须释放 request mutex。
        unlock = prepare.index("drop(active)")
        # 旧 callback owner 最后被主动注销。
        clear = prepare.index("self.clear_callback()")
        # 严格锁定去重、提交、解锁和注销顺序。
        self.assertLess(duplicate, no_create)
        # 去重早退必须位于新 request 提交之前。
        self.assertLess(no_create, commit)
        # request 提交必须先释放 mutex。
        self.assertLess(commit, unlock)
        # registry 注销不得持 request mutex。
        self.assertLess(unlock, clear)
        # checked 登记不得恢复 poisoned request owner。
        self.assertNotIn("into_inner()", prepare)

    # 确认 cancel 与显式 close 只在健康 owner 上消费 callback。
    def test_cancel_and_close_are_checked_transactions(self) -> None:
        # 读取 Component 源码。
        source = self.component_source()
        # 限定取消事务。
        cancel_start = source.index("fn cancel_checked")
        # close 方法标记取消片段终点。
        cancel_end = source.index("fn close_checked", cancel_start)
        # 保存取消事务。
        cancel = source[cancel_start:cancel_end]
        # 取消必须保留原有稳定锁失败诊断。
        self.assertIn("frame request mutex poisoned during cancellation", cancel)
        # 只允许匹配 token 进入消费路径。
        self.assertIn("request.token == token", cancel)
        # 不匹配 token 必须早退且不清 callback。
        self.assertLess(cancel.index("if !matched"), cancel.index("*active = None"))
        # 匹配路径先清 active 再注销 callback。
        self.assertLess(cancel.index("*active = None"), cancel.index("self.clear_callback()"))
        # checked 取消不得恢复 poisoned owner。
        self.assertNotIn("into_inner()", cancel)
        # 限定显式关闭事务。
        close_start = source.index("fn close_checked")
        # shutdown 方法标记关闭片段终点。
        close_end = source.index("fn shutdown", close_start)
        # 保存显式关闭事务。
        close = source[close_start:close_end]
        # close 必须保留原有稳定失败诊断。
        self.assertIn("frame request mutex poisoned during window close", close)
        # close 先清 active 再注销 callback。
        self.assertLess(close.index("*active = None"), close.index("self.clear_callback()"))
        # checked close 不得恢复 poisoned owner。
        self.assertNotIn("into_inner()", close)

    # 确认无同步接收方 teardown 仍确定性释放两份 owner。
    def test_teardown_recovers_only_to_release_callback_owner(self) -> None:
        # 读取 Component 与逐窗 teardown 编排。
        source = self.component_source()
        # 定位无返回 teardown 方法。
        shutdown_start = source.index("fn shutdown")
        # clear helper 标记 shutdown 片段终点。
        shutdown_end = source.index("fn clear_callback", shutdown_start)
        # 保存无返回 teardown。
        shutdown = source[shutdown_start:shutdown_end]
        # 无同步接收方时允许恢复 guard 完成资源清理。
        self.assertIn("unwrap_or_else(|error| error.into_inner())", shutdown)
        # teardown 必须清空 active request。
        self.assertIn("*active = None", shutdown)
        # teardown 必须主动注销协议 callback。
        self.assertIn("self.clear_callback()", shutdown)
        # clear helper 必须先 take owner 槽。
        clear = source[shutdown_end:source.index("// 消费一个已经", shutdown_end)]
        # take 保证重复 teardown 幂等。
        self.assertIn("self.callback.take()", clear)
        # handle 仍存活时注销 compat registry owner。
        self.assertIn("callback.clear_callback()", clear)
        # 读取逐窗 callback teardown 编排。
        orchestration = WINDOW_SHUTDOWN.read_text(encoding="utf-8")
        # frame owner 必须早于依赖 surface 的 xdg-shell callbacks 清理。
        self.assertLess(
            # 定位 frame Component teardown。
            orchestration.index("self.frame_callback.shutdown()"),
            # 定位 activation token 清理。
            orchestration.index("self.activation_token.take()"),
        )

    # 确认 WindowOps 只编排 Component 与协议 callback 接线。
    def test_window_ops_delegates_owner_lifecycle(self) -> None:
        # 读取 WindowBackend 编排。
        source = WINDOW_OPS.read_text(encoding="utf-8")
        # WindowOps 必须持有唯一 FrameCallbackOwner。
        self.assertIn("frame_callback: FrameCallbackOwner", source)
        # 旧的独立 frame_request 字段必须消失。
        self.assertNotIn("frame_request: Arc<Mutex", source)
        # 限定 frame request 入口。
        request_start = source.index("fn os_request_native_frame")
        # cancel 入口标记 request 片段终点。
        request_end = source.index("fn os_cancel_native_frame", request_start)
        # 保存请求编排。
        request = source[request_start:request_end]
        # request 去重与替换委托 Component。
        self.assertIn("self.frame_callback.prepare_request(request)?", request)
        # callback 闭包继续共享唯一 active source。
        self.assertIn("self.frame_callback.active_source()", request)
        # callback 接线后才发布协议 owner。
        self.assertLess(request.index("callback.quick_assign"), request.index("attach_callback(callback)"))
        # cancel 只委托 Component。
        cancel_start = source.index("fn os_cancel_native_frame")
        # 窗口状态分区标记 cancel 片段终点。
        cancel_end = source.index("// ── 窗口状态", cancel_start)
        # 保存取消编排。
        cancel = source[cancel_start:cancel_end]
        # 取消必须交给匹配 token 的 checked 事务。
        self.assertIn("self.frame_callback.cancel_checked(token)", cancel)
        # 显式 close 必须在 surface 注销前 checked 清理 frame owner。
        close_start = source.index("fn os_close")
        # 主动关闭方法标记 close 片段终点。
        close_end = source.index("fn os_request_close", close_start)
        # 保存 close 编排。
        close = source[close_start:close_end]
        # frame owner 检查先于 surface 注册表事务。
        self.assertLess(close.index("frame_callback.close_checked()?"), close.index("self.unregister_surface()?"))

    # 确认本任务涉及文件满足项目规模门槛。
    def test_touched_files_stay_within_limit(self) -> None:
        # 逐一检查源码、既有测试与本测试文件。
        for path in (FRAME_CALLBACK, WINDOW_OPS, WINDOW_SHUTDOWN, Path(__file__)):
            # 计算当前文件物理行数。
            line_count = len(path.read_text(encoding="utf-8").splitlines())
            # 文件必须保持不超过 900 行。
            self.assertLessEqual(line_count, 900, path)


# 允许直接运行本聚焦测试文件。
if __name__ == "__main__":
    # 使用标准 unittest runner 输出精确用例数。
    unittest.main()

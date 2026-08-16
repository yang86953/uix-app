# 使用路径对象读取仓库内的 Wayland window activation 契约。
from pathlib import Path
# 使用标准库测试框架保持聚焦验证无额外依赖。
import unittest

# 解析测试文件所在仓库根目录。
ROOT = Path(__file__).resolve().parents[1]
# 定位逐窗 activation Component。
ACTIVATION = ROOT / "src/native/backends/linux/wayland/window_activation.rs"
# 定位 WindowOps 同步能力入口。
WINDOW_OPS = ROOT / "src/native/backends/linux/wayland/window_ops.rs"
# 定位逐窗 callback teardown Component。
SHUTDOWN = ROOT / "src/native/backends/linux/wayland/window_callback_shutdown.rs"
# 定位兼容 callback registry adapter。
COMPAT = ROOT / "src/native/backends/linux/wayland/compat.rs"
# 定位 Wayland 子模块声明。
BACKEND = ROOT / "src/native/backends/linux/wayland/mod.rs"


# 验证 Wayland raise 使用真实异步 token 并受逐窗生命周期约束。
class WaylandWindowActivationTests(unittest.TestCase):
    # 确认 request 先注册 Done callback，再 commit token 请求。
    def test_request_registers_done_before_commit_and_activate(self) -> None:
        # 读取 activation Component。
        source = ACTIVATION.read_text(encoding="utf-8")
        # 定位 callback 注册。
        callback = source.index("token.quick_assign")
        # 定位 token request commit。
        commit = source.index("token.commit()")
        # 定位最终 activate 请求。
        activate = source.index("callback_activation.activate(token, &callback_surface)")
        # callback 必须先注册，避免丢失快速 Done。
        self.assertLess(callback, commit)
        # activate 只能出现在 Done callback 内且源码位置先于外层 commit。
        self.assertLess(activate, commit)
        # callback 必须精确匹配 compositor Done。
        self.assertIn("xdg_activation_token_v1::Event::Done { token }", source)
        # 最终激活必须使用事件返回 token。
        self.assertNotIn("activate(String::new()", source)
        # token app-id 必须与 xdg_toplevel 初始化保持一致。
        self.assertIn('token.set_app_id("uix-app".to_string())', source)

    # 确认窗口 owner 同时管理请求替换与关闭清理。
    def test_pending_token_owner_is_replaced_and_shutdown(self) -> None:
        # 读取 activation Component。
        activation = ACTIVATION.read_text(encoding="utf-8")
        # 新请求必须先取出旧 pending owner。
        previous = activation.index("self.activation_token.take()")
        # 旧 callback 必须在创建新 token 前注销。
        previous_clear = activation.index("previous.clear_callback()")
        # 新 token 必须在旧 owner 清理后创建。
        create = activation.index("activation.get_activation_token()")
        # 旧 owner 先取出再注销。
        self.assertLess(previous, previous_clear)
        # 旧 callback 注销后才允许新建请求。
        self.assertLess(previous_clear, create)
        # 新 pending handle 必须发布回窗口 owner。
        self.assertIn("self.activation_token = Some(token)", activation)
        # 读取窗口 teardown Component。
        shutdown = SHUTDOWN.read_text(encoding="utf-8")
        # 关闭必须消费 pending token owner。
        self.assertIn("self.activation_token.take()", shutdown)
        # 关闭必须注销迟到 Done callback。
        self.assertIn("activation_token.clear_callback()", shutdown)
        # activation 必须先于 surface callbacks 清理。
        self.assertLess(
            # pending token callback 清理位置。
            shutdown.index("activation_token.clear_callback()"),
            # toplevel callback 清理位置。
            shutdown.index("toplevel.clear_callback()"),
        )

    # 确认同步 WindowOps 入口只委托异步 request 建立。
    def test_raise_delegates_without_empty_token(self) -> None:
        # 读取 WindowOps Module。
        source = WINDOW_OPS.read_text(encoding="utf-8")
        # 定位 raise 入口。
        start = source.index("fn os_raise")
        # 以 lower 入口作为片段终点。
        end = source.index("fn os_lower", start)
        # 保存同步 raise 端口。
        raise_source = source[start:end]
        # 入口必须委托唯一 activation Component。
        self.assertIn("self.request_activation()", raise_source)
        # 同步入口不得自行 activate。
        self.assertNotIn(".activate(", raise_source)
        # 同步入口不得阻塞等待事件。
        self.assertNotIn("roundtrip", raise_source)
        # 旧空 token 路径必须从整个 WindowOps 消失。
        self.assertNotIn("String::new(), surface", source)

    # 确认 activation token 与 frame callback 都按 one-shot 消费。
    def test_compat_consumes_both_one_shot_interfaces(self) -> None:
        # 读取 compat dispatch adapter。
        source = COMPAT.read_text(encoding="utf-8")
        # 定位泛型 dispatch 起点。
        start = source.index("fn dispatch<I>")
        # 以 Dispatch trait 实现作为片段终点。
        end = source.index("impl<I> Dispatch", start)
        # 保存单一 dispatch adapter。
        dispatch = source[start:end]
        # frame callback 必须保持 one-shot 判定。
        self.assertIn("TypeId::of::<wl_callback::WlCallback>()", dispatch)
        # activation token 也必须按 one-shot 判定。
        self.assertIn("TypeId::of::<XdgActivationTokenV1>()", dispatch)
        # 两类 one-shot 必须在持久 owner 回插前返回。
        self.assertLess(dispatch.index("XdgActivationTokenV1"), dispatch.index('"callback reinsert"'))

    # 确认 activation Component 已接入唯一 Wayland 模块树。
    def test_component_is_declared_once(self) -> None:
        # 读取 backend 子模块声明。
        source = BACKEND.read_text(encoding="utf-8")
        # 独立 Component 必须只声明一次。
        self.assertEqual(source.count("mod window_activation;"), 1)

    # 确认本任务涉及文件满足项目规模门槛。
    def test_touched_files_stay_within_limit(self) -> None:
        # 逐一检查本任务修改的源码与测试文件。
        for path in (ACTIVATION, WINDOW_OPS, SHUTDOWN, COMPAT, BACKEND, Path(__file__)):
            # 计算当前文件物理行数。
            line_count = len(path.read_text(encoding="utf-8").splitlines())
            # 文件必须保持不超过 900 行。
            self.assertLessEqual(line_count, 900, path)


# 允许直接运行本聚焦测试文件。
if __name__ == "__main__":
    # 使用标准 unittest runner 输出精确用例数。
    unittest.main()

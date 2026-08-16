# 使用路径对象读取仓库内的 Wayland SHM buffer 生命周期契约。
from pathlib import Path
# 使用标准库测试框架保持聚焦验证无额外依赖。
import unittest

# 解析测试文件所在仓库根目录。
ROOT = Path(__file__).resolve().parents[1]
# 定位 SHM buffer RAII Component。
SHM_BUFFER = ROOT / "src/native/backends/linux/wayland/shm_buffer.rs"
# 定位 SHM buffer 构造与 Release callback 注册入口。
PRESENTER = ROOT / "src/native/backends/linux/wayland/presenter.rs"


# 验证 wl_buffer::Release callback owner 随 ShmBuffer 生命周期释放。
class WaylandShmBufferCallbackShutdownTests(unittest.TestCase):
    # 确认 ShmBuffer Drop 只注销其唯一 callback owner。
    def test_drop_clears_release_callback_without_protocol_side_effects(self) -> None:
        # 读取 SHM buffer Component。
        source = SHM_BUFFER.read_text(encoding="utf-8")
        # 定位 RAII Drop 实现。
        start = source.index("impl Drop for ShmBuffer")
        # 保存文件末尾的 Drop adapter。
        drop_impl = source[start:]
        # Drop 必须精确注销 buffer callback。
        self.assertEqual(drop_impl.count("self.buffer.clear_callback()"), 1)
        # Drop 不得伪造一次 Release。
        self.assertNotIn("self.lease.release()", drop_impl)
        # Drop 不得发送 wl_buffer destroy 请求。
        self.assertNotIn(".destroy()", drop_impl)
        # Drop 不得向 UI 或 failure queue 入队。
        self.assertNotIn("enqueue", drop_impl)
        # Drop 不得恢复或访问任何共享 mutex。
        self.assertNotIn(".lock()", drop_impl)

    # 确认 callback 注册先于 ShmBuffer owner 发布。
    def test_constructor_registers_callback_before_owner_publication(self) -> None:
        # 读取 presenter Module。
        source = PRESENTER.read_text(encoding="utf-8")
        # 定位 SHM buffer 构造入口。
        start = source.index("fn create_shm_buffer")
        # 以 impl 结束标记作为构造片段终点。
        end = source.index("// ════════════════════════════════════════════════════════════════════════════", start)
        # 保存单一构造片段。
        constructor = source[start:end]
        # 定位 wl_buffer callback 注册。
        callback = constructor.index("buf.quick_assign")
        # 定位 ShmBuffer owner 发布。
        publish = constructor.index("Ok(ShmBuffer {")
        # callback 必须在 owner 进入任何回滚或替换路径前注册。
        self.assertLess(callback, publish)
        # callback 必须只观察真实 Release 事件。
        self.assertIn("wl_buffer::Event::Release", constructor)
        # callback 必须释放构造期克隆的同一 lease。
        self.assertIn("released.release()", constructor)

    # 确认 presenter 的替换仍由 Option/ShmBuffer RAII 自动覆盖。
    def test_resize_replaces_owned_buffers_transactionally(self) -> None:
        # 读取 presenter Module。
        source = PRESENTER.read_text(encoding="utf-8")
        # 定位 resize 进入点。
        start = source.index("if needs_resize")
        # 以 surface 获取作为 resize 片段终点。
        end = source.index("let surface = self.surface.as_ref()", start)
        # 保存双缓冲替换事务。
        resize = source[start:end]
        # 新 buffer 必须先在局部数组完整构造。
        local_owner = resize.index("let mut new_bufs")
        # 旧数组只能在构造循环成功后替换。
        replace = resize.index("self.shm_buffers = new_bufs")
        # 局部 owner 先于正式替换建立。
        self.assertLess(local_owner, replace)
        # 赋值会统一 Drop 两个旧 ShmBuffer 并触发 callback 注销。
        self.assertEqual(resize.count("self.shm_buffers = new_bufs"), 1)

    # 确认本任务涉及文件满足项目规模门槛。
    def test_touched_files_stay_within_limit(self) -> None:
        # 逐一检查本任务修改的源码与测试文件。
        for path in (SHM_BUFFER, Path(__file__)):
            # 计算当前文件物理行数。
            line_count = len(path.read_text(encoding="utf-8").splitlines())
            # 文件必须保持不超过 900 行。
            self.assertLessEqual(line_count, 900, path)


# 允许直接运行本聚焦测试文件。
if __name__ == "__main__":
    # 使用标准 unittest runner 输出精确用例数。
    unittest.main()

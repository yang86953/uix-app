# 声明测试文件使用 UTF-8 编码。
# -*- coding: utf-8 -*-
# 说明本测试直接编译 Linux fontconfig 候选实现，验证多候选与 SystemInfo 接线契约。
"""Exercise Linux CJK fontconfig discovery with an isolated fc-match provider."""

# 启用延迟解析类型标注。
from __future__ import annotations

# 引入环境变量复制与 PATH 拼接能力。
import os
# 引入子进程以编译并执行生产候选实现。
import subprocess
# 引入隔离目录管理器。
import tempfile
# 引入标准库单元测试框架。
import unittest
# 引入跨平台路径类型。
from pathlib import Path


# 定位仓库根目录。
ROOT = Path(__file__).resolve().parents[1]
# 定位被测 Linux fontconfig 生产实现。
FONTS_SOURCE = ROOT / "src" / "native" / "backends" / "linux" / "system_info" / "fonts.rs"
# 定位 LinuxSystemInfo 的 trait 接线实现。
SYSTEM_INFO_SOURCE = ROOT / "src" / "native" / "backends" / "linux" / "system_info" / "mod.rs"


# 定义 Linux CJK 字体发现回归测试。
@unittest.skipUnless(os.name == "posix", "isolated fc-match fixture requires a POSIX executable")
class LinuxFontDiscoveryContractTests(unittest.TestCase):
    # 验证生产实现保留错误首项后的候选并跨模式去重。
    def test_cjk_probe_keeps_ordered_fallback_candidates(self) -> None:
        # 创建不会读取宿主 fontconfig 配置的隔离目录。
        with tempfile.TemporaryDirectory() as temp_dir:
            # 把隔离根转换为路径对象。
            fixture_root = Path(temp_dir)
            # 定位伪 fontconfig 可执行文件。
            fake_fc_match = fixture_root / "fc-match"
            # 定位模拟 OpenType/CFF 容器的后续 CJK 候选。
            cjk_font = fixture_root / "cjk.otf"
            # 写入 OTTO 头，锁定平台发现层不会按过时后端假设提前过滤候选。
            cjk_font.write_bytes(b"OTTO")
            # 把动态临时路径转义为可安全嵌入单引号 shell 文本的内容。
            cjk_font_path = cjk_font.as_posix().replace("'", "'\\''")
            # 写入每次都返回错误 Latin 首项、可用 CJK 后项与重复项的稳定 provider。
            fake_fc_match.write_text(
                # 使用 POSIX shell 执行最小伪命令。
                "#!/bin/sh\n"
                # 首项模拟 fontconfig 错把 Latin 字体排在中文查询首位。
                "printf '/fonts/latin.ttf\\n'\n"
                # 后项模拟真实包含 CJK 字形的 OpenType/CFF 字体。
                f"printf '{cjk_font_path}\\n'\n"
                # 重复项验证跨模式与单次输出都不会重复登记。
                f"printf '{cjk_font_path}\\n'\n",
                # 固定 UTF-8 编码写入脚本。
                encoding="utf-8",
            )
            # 赋予当前用户读写执行权限。
            fake_fc_match.chmod(0o700)
            # 定位调用真实生产模块的临时 Rust 入口。
            harness_source = fixture_root / "main.rs"
            # 生成绝对路径，避免 rustc 工作目录影响模块定位。
            fonts_path = FONTS_SOURCE.as_posix()
            # 写入只调用生产 CJK 多候选函数的最小 harness。
            harness_source.write_text(
                # 通过 path 属性直接编译当前仓库生产实现。
                f'#[path = "{fonts_path}"]\n'
                # 把生产实现纳入同一测试 crate，以访问 crate-private 函数。
                "mod fonts;\n"
                # 声明临时二进制入口。
                "fn main() {\n"
                # 逐行输出生产实现返回的候选顺序。
                "    for path in fonts::probe_cjk_font_paths() {\n"
                # 标准输出作为 Python 断言的稳定观察面。
                "        println!(\"{path}\");\n"
                # 结束候选遍历。
                "    }\n"
                # 结束临时二进制入口。
                "}\n",
                # 固定 UTF-8 编码写入 Rust 源码。
                encoding="utf-8",
            )
            # 定位临时编译产物。
            harness_binary = fixture_root / "font-discovery-contract"
            # 编译生产模块与最小 harness。
            compile_result = subprocess.run(
                # 直接使用当前 Rust 工具链编译隔离入口。
                ["rustc", "--edition=2024", str(harness_source), "-o", str(harness_binary)],
                # 捕获失败诊断供测试报告定位。
                capture_output=True,
                # 按 UTF-8 文本返回诊断。
                text=True,
                # 不让 rustc 失败直接绕过 unittest 断言。
                check=False,
            )
            # 生产模块必须能在隔离测试 crate 中成功编译。
            self.assertEqual(compile_result.returncode, 0, compile_result.stderr)
            # 复制当前环境，保留动态链接器与基础系统变量。
            run_env = os.environ.copy()
            # 把隔离 provider 放到 PATH 首位，同时保留其余命令查找路径。
            run_env["PATH"] = f"{fixture_root}{os.pathsep}{run_env.get('PATH', '')}"
            # 执行生产候选发现逻辑。
            run_result = subprocess.run(
                # 调用刚编译的隔离 harness。
                [str(harness_binary)],
                # 注入伪 fontconfig provider。
                env=run_env,
                # 捕获候选列表与失败诊断。
                capture_output=True,
                # 按 UTF-8 文本读取输出。
                text=True,
                # 不让执行失败绕过稳定断言。
                check=False,
            )
            # 生产候选发现不得因错误首项而失败。
            self.assertEqual(run_result.returncode, 0, run_result.stderr)
            # 错误首项与后续 CJK 候选都必须保留，重复项只能出现一次。
            self.assertEqual(run_result.stdout.splitlines(), ["/fonts/latin.ttf", cjk_font.as_posix()])

    # 验证 LinuxSystemInfo 覆盖 trait 默认单候选包装逻辑。
    def test_linux_system_info_uses_multi_candidate_probe(self) -> None:
        # 读取当前生产接线源码。
        source = SYSTEM_INFO_SOURCE.read_text(encoding="utf-8")
        # Linux 实现必须显式覆盖多候选 trait 方法。
        self.assertIn("fn probe_cjk_font_paths(&self) -> Vec<String>", source)
        # 覆盖方法必须委托给同一 fontconfig 多候选实现。
        self.assertIn("probe_cjk_font_paths()", source)


# 允许直接执行当前测试文件。
if __name__ == "__main__":
    # 使用标准 unittest 入口执行全部用例。
    unittest.main()

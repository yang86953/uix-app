# 内存剖析探针：启动 demo 采样 working set / private bytes，按场景对比
import subprocess, time, sys, ctypes, json, os
from ctypes import wintypes
# 使用 pathlib 根据探针位置解析仓库根目录，避免依赖旧机器路径。
from pathlib import Path

class PMC(ctypes.Structure):
    _fields_ = [("cb", wintypes.DWORD), ("PageFaultCount", wintypes.DWORD),
        ("PeakWorkingSetSize", ctypes.c_size_t), ("WorkingSetSize", ctypes.c_size_t),
        ("QuotaPeakPagedPoolUsage", ctypes.c_size_t), ("QuotaPagedPoolUsage", ctypes.c_size_t),
        ("QuotaPeakNonPagedPoolUsage", ctypes.c_size_t), ("QuotaNonPagedPoolUsage", ctypes.c_size_t),
        ("PagefileUsage", ctypes.c_size_t), ("PeakPagefileUsage", ctypes.c_size_t),
        ("PrivateUsage", ctypes.c_size_t)]

def sample(pid):
    psapi = ctypes.WinDLL("psapi", use_last_error=True)
    c = PMC(); c.cb = ctypes.sizeof(c)
    h = ctypes.WinDLL("kernel32").OpenProcess(0x0400|0x0010, False, pid)
    if not psapi.GetProcessMemoryInfo(h, ctypes.byref(c), c.cb):
        raise OSError(ctypes.get_last_error())
    ctypes.WinDLL("kernel32").CloseHandle(h)
    return c.WorkingSetSize, c.PrivateUsage

# 使用传入仓库根目录启动并采样指定 demo 场景。
def run(extra_args, seconds, label, repo_root):
    # 根据仓库根目录定位 release demo，保持采样目标与当前工作树一致。
    executable = repo_root / "demo" / "target" / "release" / "uix-demo.exe"
    # 将 Path 转成原生进程参数，兼容 Windows 路径中的空格。
    cmd = [str(executable)] + extra_args
    print(f"== {label}: {' '.join(extra_args) or '(默认)'} ==", flush=True)
    # 从仓库根目录启动 demo，使相对资源路径与正式运行入口一致。
    proc = subprocess.Popen(cmd, cwd=repo_root)
    time.sleep(seconds)
    ws, priv = sample(proc.pid)
    proc.terminate()
    try: proc.wait(timeout=5)
    except subprocess.TimeoutExpired: proc.kill()
    print(f"   working_set={ws/2**20:.1f} MiB  private={priv/2**20:.1f} MiB", flush=True)
    return ws, priv

if __name__ == "__main__":
    # 默认以探针脚本所在仓库为根，也允许环境变量覆盖多工作树路径。
    repo_root = Path(os.environ.get("UIX_REPO_ROOT", Path(__file__).resolve().parents[1])).resolve()
    runs = [
        ("空窗口（框架+驱动固定开销）", ["--empty"], 12),
        ("默认 GUI（首页）", [], 12),
    ]
    if len(sys.argv) > 1 and sys.argv[1] == "all":
        runs += [
            ("组件 QA", ["--component-qa"], 12),
            ("G5 场景", ["--g5-release-scenario"], 15),
        ]
    for label, args, sec in runs:
        # 将统一的仓库根目录传给每个场景，避免不同场景使用不同工作树。
        run(args, sec, label, repo_root)

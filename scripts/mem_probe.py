# 内存剖析探针：启动 demo 采样 working set / private bytes，按场景对比
import subprocess, time, sys, ctypes, json, os
from ctypes import wintypes

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

def run(extra_args, seconds, label):
    cmd = ["demo/target/release/uix-demo.exe"] + extra_args
    print(f"== {label}: {' '.join(extra_args) or '(默认)'} ==", flush=True)
    proc = subprocess.Popen(cmd, cwd="G:/code/uix-app")
    time.sleep(seconds)
    ws, priv = sample(proc.pid)
    proc.terminate()
    try: proc.wait(timeout=5)
    except subprocess.TimeoutExpired: proc.kill()
    print(f"   working_set={ws/2**20:.1f} MiB  private={priv/2**20:.1f} MiB", flush=True)
    return ws, priv

if __name__ == "__main__":
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
        run(args, sec, label)

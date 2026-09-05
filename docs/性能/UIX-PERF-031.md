# UIX-PERF-031：Vulkan 像素传输缓冲按需申请与及时回收

2026-09-06 在 Linux / Wayland / AMD Radeon 780M（RADV PHOENIX）上，用真实本地
媒体播放器的隔离空库窗口对照。1536×1024、DPR 1 下，移除 GpuNative 用不到的
CPU 像素上传缓冲预分配，稳定少申请 6 MiB；截图结束后回收读回 staging，
不再让截图及历史最大窗口尺寸抬高 GPU 缓冲常驻量。

这是图形缓冲的生命周期优化。进程 RSS 样本区间重叠，未证明稳定下降；不能把
GPU 缓冲节省量直接当作任务管理器中的进程内存降幅。

## 场景、构建与计量口径

- 框架基线：`e4fcb83cce48830a226baaba79f05aaeb6d7884d`，候选仅加本记录的
  Vulkan 缓冲生命周期修改；全程在 `main` 工作。
- 消费者源码：`local-media-player` 的 `5d14faa8db3a2ab7462e77bfb1e7c2abc893d150`。
  将源码、资源和构建文件复制到框架 `target/framework-memory-20260906/consumer/`，
  仅副本的 `uix` 依赖改为框架本地路径。原播放器仓库、依赖锁定和用户媒体库未改。
- 两侧使用相同消费者 dev profile：应用 `opt-level=0`，依赖（包括 UIX）
  `opt-level=2`；离线构建，基线和候选二进制分别保存后执行。
- 每侧 3 个独立进程，使用独立 `XDG_DATA_HOME`、`XDG_CONFIG_HOME`、
  `XDG_CACHE_HOME`，媒体目录为空、不加载音视频，以 `--agent-control` 启动。
- 窗口可呈现后每隔 3 秒读取一次 `/proc/<pid>/smaps_rollup`，共 3 次。
  再经 `scripts/agent_client.py` 的持续会话依次截图 5 次、缩至 960×640 截图
  3 次、放大至 1920×1200 截图 3 次、恢复 1536×1024 截图 3 次。
  每组截图后等待 2～3 秒再采样。所有窗口操作均先枚举并按 PID 绑定实例。
- DRM 通过 `/proc/<pid>/fdinfo/*` 读取，同一 GPU 的重复 `drm-client-id`
  去重；本次只有一个 GPU 客户端。`drm-total-gtt` 计申请量，图形驻留量取
  `drm-resident-gtt + drm-resident-vram`，其中包含共享交换链缓冲，
  不再叠加 `drm-shared-*` 或作为别名的 `drm-memory-*`，也不与 RSS 相加。
  字段含义以 [Linux DRM 统计规范](https://docs.kernel.org/gpu/drm-usage-stats.html#memory)
  为准。

## 归因、变更与 SMC 边界

变更只落在 Native System 已有 Vulkan Adapter / Context 的资源所有者中：

1. `VulkanContext::new` 原先在建立交换链后，无条件按整窗像素数乘四申请
   HOST_VISIBLE 上传缓冲，即使窗口完全走 GpuNative 也一样。移除这笔预分配，
   保留 `upload_pixels` 入口原有的等待及按需扩容，不改变 PixelUpload 行为。
2. Surface 回读原先保留最大 staging 到窗口关闭。现在沿既有同步即时命令
   等待 GPU 完成，`read_pixels` 复制到自有 `Vec<u32>` 并解除映射后，立即释放
   buffer/memory。映射或像素转换失败同样释放；提交、等待失败时仍保留资源，
   由 Context 的既有关闭或 device-lost 协议回收，避免完成状态未知时提前销毁。

没有新增 owner、公共 API、定时回收线程或上层 Vulkan 分支。交换链数量、
呈现模式、帧同步、字体和绘制缓存均保持原状。及时回收只调整已有安全释放点，
满足 [Vulkan 缓冲销毁前必须完成所有引用命令](https://docs.vulkan.org/spec/latest/chapters/resources.html#VUID-vkDestroyBuffer-buffer-00922)
的要求；下一次即时命令先重置命令池，再重新录制。
长期生命周期契约见 [Vulkan 多窗口共享设备合同](../架构/graphics/vulkan-multi-window.md#像素传输缓冲生命周期)。

## A/B 数据

以下为三个独立进程的中位数，单位 MiB；空闲使用每轮第 3 个样本。

| 场景与指标 | 基线 | 修改后 | 变化 |
|---|---:|---:|---:|
| 空闲 GTT 缓冲申请量 | 9.219 | 3.219 | −6.000 |
| 1536×1024 连续截图后 GTT 缓冲申请量 | 15.219 | 3.219 | −12.000 |
| 放大截图并恢复原尺寸后 GTT 缓冲申请量 | 18.008 | 3.219 | −14.789 |
| 空闲图形驻留量 | 41.426 | 35.426 | 约 −14.5% |
| 放大截图并恢复原尺寸后图形驻留量 | 50.285 | 35.477 | 约 −29.4% |
| 空闲进程 RSS | 63.258 | 62.730 | 区间重叠，未证明稳定下降 |

GTT 申请量在两侧各自三轮中一致。最后一项 GTT 差额可对应到
6 MiB 上传缓冲与 1920×1200×4 字节（8.789 MiB）最大读回缓冲。
图形驻留量还包含驱动和绘制资源的小幅变化，不用它逐字节归因。

空闲 RSS 基线为 62.914～63.551 MiB，修改后为 62.719～63.215 MiB；匿名内存
分别为 7.875～8.129 和 7.750～8.137 MiB。截图后进程 RSS 仍高于截图前，
本改动没有消除 CPU 截图编码等路径的内存高水位，不宣称修复了所有缓存或泄漏。

截图耗时包含 Agent 请求到完整响应 JSON 解析，未包含后续 PNG 写盘：

| 1536×1024 截图 | 基线中位数（范围） | 修改后中位数（范围） |
|---|---:|---:|
| 首次，N=3 | 52.69（51.50～52.82）ms | 53.68（52.95～56.83）ms |
| 后续连续请求，N=12 | 41.87（39.64～43.53）ms | 41.24（39.14～44.45）ms |

本机样本未出现明显持续截图变慢；首次截图约增加 1 ms，样本量不足以判断稳定
差异。每次回读重新申请原生缓冲的代价仍存在，不能据此保证其它驱动或高频采集吞吐。
基线后两轮与候选编译曾有时间重叠，耗时仅作为端到端运行观察，非隔离吞吐基准。

## 复核入口与证据

本次保留的临时工作目录为 `target/framework-memory-20260906/`。其中 `probe.py`
只通过仓库 Agent 客户端操作真实应用，不访问私有 RHI，也不属于项目测试。
若该目录已清理，下面的测量入口和原始 PNG 不再可用，应重新准备相同消费者副本，
在同一构建配置下分别保留修改前后二进制；不要拿播放器旧锁定版本作本次基线。

```bash
# 在框架仓库中，用隔离消费者构建当前待测版本。
rtk cargo build --offline \
  --manifest-path target/framework-memory-20260906/consumer/Cargo.toml \
  --target-dir target --bin local-media-player

# baseline/bin 与 candidate/bin 是分别构建后保存的二进制，不能被下一轮构建覆盖。
# 每次使用新的 tag，脚本拒绝覆盖已有结果。
rtk proxy python3 target/framework-memory-20260906/probe.py \
  --binary target/framework-memory-20260906/baseline/bin/local-media-player \
  --tag baseline-recheck --count 3
rtk proxy python3 target/framework-memory-20260906/probe.py \
  --binary target/framework-memory-20260906/candidate/bin/local-media-player \
  --tag candidate-recheck --count 3
```

基线二进制 SHA-256：
`e3bd43d79b717013c9c8b1e5c8700db857b6d9021b353fc731feddbb0f25c054`。
候选二进制 SHA-256：
`4209787ce1eb8830a05d2b4db66e3fd561831b24f8ec5934ddee0bd692582cd5`。

可长期复核的逐次样本随本记录保存：
[内存样本 CSV](evidence/UIX-PERF-031-samples.csv)、
[截图耗时与 SHA-256 CSV](evidence/UIX-PERF-031-captures.csv)。
临时目录另保留每轮 `report.json`、原始 `smaps_rollup`、DRM fdinfo、应用日志、
语义快照、PNG、消费者构建日志和跨目标检查日志；其中无用户媒体或凭据。

## 正确性验证与停止边界

- 6 个真实 Linux 进程均呈现成功，经 Agent 关闭后退出码为 0。
- 42 对 PNG 的 SHA-256 完全一致，覆盖初始、缩小、放大和恢复尺寸；
  另实际查看初始和缩小后的候选截图，内容和布局正常。
- 消费者 Linux 构建通过；`python3 scripts/diagnostics_audit.py --verbose`
  扫描 969 个源文件通过，没有绕过诊断系统的错误报告点。
- 库在 `x86_64-pc-windows-msvc`、`aarch64-apple-darwin` 和
  `x86_64-apple-darwin` 上执行以下定向检查均通过：
  `cargo check --offline --locked --package uix --lib --target <目标> --features agent-control`。
  这些仅证明目标代码可编译，不证明 Windows/macOS 真机运行或节省量。
- 依照公开 API 测试边界，不新增或运行私有 Vulkan / RHI 项目测试。
  没有实际命中 PixelUpload 消费者、设备丢失或 OOM 故障场景，也未声称对应运行验收。
- 停止于已量化的两处缓冲生命周期开销；不据短时样本排除长期泄漏，
  不更改播放器依赖版本，不把本地优化视为版本发布。

# Vulkan 多窗口共享设备合同

Vulkan Adapter 属于 Graphics System 的原生实现边界。它不通过 EventBus 或上层
Vulkan 分支协调恢复，而是让共享逻辑设备本身持有健康状态，让每个窗口的既有
`RecoveryDriver` 在自己的帧边界消费同一个 typed loss 事实。

## 所有权与关闭顺序

```text
ThreadBound VulkanRuntime（线程本地唯一 Rc）
├─ VkInstance / Surface loader
└─ (PhysicalDevice, QueueFamily) -> Weak<VulkanDevice>
   └─ VulkanDevice（同一健康选择共享 Rc）
      ├─ VkDevice（唯一 Drop）
      ├─ VkQueue + QueueSerial（所有 submit/present 外部同步）
      └─ DeviceLossState（首次 DEVICE_LOST 后不可逆）
         ├─ Window A VulkanContext
         │  └─ Surface -> Swapchain -> image views -> 每窗口 semaphore/fence/command pool
         └─ Window B VulkanContext
            └─ Surface -> Swapchain -> image views -> 每窗口 semaphore/fence/command pool
```

Runtime 只保存 device 弱引用；窗口 Context 的强 lease 保证旧 device 在自己的 child
销毁完成前存活。逐窗口关闭按 Drawing 资源、image views、buffer/memory、fence、
semaphore、command pool、swapchain/present retirement、surface、device lease、runtime
lease 的逆依赖顺序执行。最后一个旧 lease 释放时才由 `VulkanDevice::Drop` 唯一销毁
旧 `VkDevice`，新 device 不接管旧 child，也不能提前或重复销毁旧 device。

## 像素传输缓冲生命周期

`GpuNative` 窗口初始化不预分配整窗 CPU 像素上传缓冲。只有显式调用
`PixelUpload` 上传入口时，才在等待上一笔上传完成后按实际像素尺寸申请或扩容；
窗口关闭仍由所属 Context 回收该缓冲。

Surface 回读缓冲只服务当前显式截图事务：按请求区域申请，沿共享 queue 的
串行即时命令提交，成功等待 GPU 完成后映射、复制到自有 `Vec<u32>` 并解除映射，
随即释放原生 buffer/memory。回读成功或映射、像素转换失败均走该释放点，
不把最大截图尺寸变成窗口常驻缓存。连续截图需要重新申请临时缓冲。

如果提交或等待失败，则不在 GPU 完成情况未知时提前释放；残留资源仍由 Context
按既有 shutdown / device-lost 协议回收。实例、共享设备、交换链和正常绘制资源
的所有权保持原契约。一次 Linux 真窗测量见
[UIX-PERF-031](../../性能/UIX-PERF-031.md)，数据不代表其它驱动或平台的收益。

## 丢失与重建状态

```text
Healthy(device N)
  ├─ 同 key acquire ───────────────> 复用 device N
  └─ 任一原生调用返回 DEVICE_LOST -> Lost(device N, first typed error)

Lost(device N)
  ├─ 任一旧 peer 业务操作 --------> GraphicsDeviceLost（原生调用前拒绝）
  ├─ 旧 Context shutdown ----------> 跳过 wait，只逆序销毁自己的 child
  └─ Runtime 下一次同 key acquire -> Healthy(device N+1)
                                       （旧 N 仍由旧 Context lease 保活）
```

同一个 `VulkanDevice` 的 queue 操作必须取得不可重入 `QueueSerial` 租约；不同窗口的
提交因此串行。acquire semaphore、render-finished semaphore、frame/present fence、
command pool 与 submission tracker 均只存在于各自 `VulkanContext`，不得跨窗口移交。

每个窗口唯一拥有一个 `RecoveryDriver` 和恢复游标。共享 device 丢失后，各窗口首次
触达旧 lease 时各得到一次 `GraphicsDeviceLost`；各自只登记首个 pending failure，并在
下一帧边界执行一次受控 backend 重建。健康 replacement 成功后后续帧不再重建，因而
不会形成 peer 重试风暴。

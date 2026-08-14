// 导入 discovery、管道和临时目录所需文件系统能力。
use std::fs::{self, File, OpenOptions};
// 导入子进程输出读取能力。
use std::io::Read;
// 导入 Windows 文件共享模式扩展以独占 Agent 管道客户端。
use std::os::windows::fs::OpenOptionsExt;
// 导入稳定路径值。
use std::path::PathBuf;
// 导入主演示子进程与管道配置。
use std::process::{Child, Command, Stdio};
// 导入输出读取线程句柄。
use std::thread::{self, JoinHandle};
// 导入有界 discovery 与连接等待时间。
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

// 导入 JSON descriptor 值。
use serde_json::Value;

// 保存主演示发布 Agent descriptor 的最长等待时间。
const START_TIMEOUT: Duration = Duration::from_secs(45);

// 保存 discovery 对外提供的最小连接信息。
pub(crate) struct AgentEndpoint {
    // 保存同用户本机管道端点。
    pub(crate) endpoint: String,
    // 保存首条 hello 使用的高熵 token。
    pub(crate) token: String,
}

// 唯一拥有本测试启动的主演示进程与临时 discovery 目录。
pub(crate) struct DemoProcess {
    // 保存仍存活或待回收的子进程。
    child: Child,
    // 保存隔离 LOCALAPPDATA 根目录。
    discovery_root: PathBuf,
    // 保存按子进程 ID 命名的 descriptor 路径。
    discovery_path: PathBuf,
    // 保存 stdout/stderr 的并行读取线程，避免管道背压阻塞窗口。
    output_readers: Vec<JoinHandle<String>>,
}

// 实现主演示 fixture 的确定启动、连接与回收。
impl DemoProcess {
    // 定位与当前测试 profile 对齐的主演示二进制。
    fn binary_path() -> PathBuf {
        // 测试 profile 与 demo workspace 的构建 profile 必须一致。
        let profile = if cfg!(debug_assertions) {
            // 普通 cargo test 使用 debug 产物。
            "debug"
        } else {
            // release 测试使用 release 产物。
            "release"
        };
        // 从根 crate 清单目录进入独立 demo workspace 产物目录。
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            // 进入 demo workspace。
            .join("demo")
            // 进入共享 target。
            .join("target")
            // 选择当前 profile。
            .join(profile)
            // 选择 Windows 主演示可执行文件。
            .join("uix-lang-demo.exe");
        // 缺失产物时提供可直接执行的精确构建命令。
        assert!(
            path.is_file(),
            "主演示二进制不存在：{}；请先执行：\ncargo build --manifest-path demo/Cargo.toml --features \"agent-control,test-harness\" --bin uix-lang-demo",
            path.display()
        );
        // 返回已经验证存在的精确路径。
        path
    }

    // 启动强制 D3D11 的专用图形恢复验收进程。
    pub(crate) fn spawn() -> Self {
        // 为并发或重复测试构造不冲突的目录后缀。
        let unique = SystemTime::now()
            // 计算自 Unix epoch 起的稳定递增时长。
            .duration_since(UNIX_EPOCH)
            // 系统时钟早于 epoch 时无法建立唯一目录。
            .expect("system clock after Unix epoch")
            // 使用纳秒降低同进程碰撞概率。
            .as_nanos();
        // 在系统临时目录内创建本次测试唯一 LOCALAPPDATA 根。
        let discovery_root = std::env::temp_dir().join(format!(
            // 同时包含测试进程 ID 与高精度时间。
            "uix-lang-graphics-recovery-{}-{unique}",
            // 读取当前测试进程 ID。
            std::process::id()
        ));
        // 创建隔离目录。
        fs::create_dir(&discovery_root).expect("create isolated LOCALAPPDATA");
        // 预建 Agent discovery 子目录。
        fs::create_dir(discovery_root.join("uix-agent"))
            // 目录创建失败必须阻止启动未隔离的应用。
            .expect("create isolated Agent discovery directory");

        // 从已构建主演示创建子进程命令。
        let mut command = Command::new(Self::binary_path());
        // 配置双运行时门禁、隔离环境和输出管道。
        command
            // 显式启用本机 Agent Bridge。
            .arg("--agent-control")
            // 显式选择专用图形恢复页面。
            .arg("--test-graphics-recovery")
            // 将 descriptor 限定到本测试唯一目录。
            .env("LOCALAPPDATA", &discovery_root)
            // 强制使用本任务验证的 Windows D3D11 recipe。
            .env("UIX_GRAPHICS_BACKEND", "d3d11")
            // 保留图形恢复与 native lower boundary 的 info 证据。
            .env("RUST_LOG", "info")
            // 测试不向主演示提供交互式标准输入。
            .stdin(Stdio::null())
            // 独立捕获标准输出。
            .stdout(Stdio::piped())
            // 独立捕获标准错误。
            .stderr(Stdio::piped());
        // 启动唯一主演示子进程。
        let mut child = command.spawn().expect("launch uix-lang-demo process");
        // 取得 stdout 管道所有权。
        let stdout = child.stdout.take().expect("capture demo stdout");
        // 取得 stderr 管道所有权。
        let stderr = child.stderr.take().expect("capture demo stderr");
        // 并行排空两个输出通道，避免子进程写满缓冲区。
        let output_readers = vec![read_output(stdout), read_output(stderr)];
        // descriptor 名称由 Agent Bridge 使用主演示进程 ID 构造。
        let discovery_path = discovery_root
            // 进入 Agent discovery 子目录。
            .join("uix-agent")
            // 拼接当前主演示进程 ID。
            .join(format!("uix-{}.json", child.id()));
        // 返回完整 fixture 所有权。
        Self {
            // 保存子进程。
            child,
            // 保存临时根目录。
            discovery_root,
            // 保存 descriptor 路径。
            discovery_path,
            // 保存输出读取线程。
            output_readers,
        }
    }

    // 等待 Agent Bridge 发布 ready descriptor。
    pub(crate) fn wait_for_endpoint(&mut self) -> AgentEndpoint {
        // 计算绝对超时点，避免无界轮询。
        let deadline = Instant::now() + START_TIMEOUT;
        // 有界检查 descriptor 或进程提前退出。
        loop {
            // 尝试读取可能仍在原子替换中的 descriptor。
            if let Ok(bytes) = fs::read(&self.discovery_path) {
                // 只接受完整合法 JSON。
                if let Ok(descriptor) = serde_json::from_slice::<Value>(&bytes) {
                    // ready 且进程 ID 匹配时才接管端点。
                    if descriptor["state"] == "ready"
                        && descriptor["process_id"].as_u64() == Some(self.child.id() as u64)
                    {
                        // endpoint 必须是 Unicode 字符串。
                        let endpoint = descriptor["endpoint"]
                            // 读取管道端点字符串。
                            .as_str()
                            // 缺失端点属于无效 descriptor。
                            .expect("descriptor endpoint")
                            // 复制到 fixture 自有值。
                            .to_string();
                        // token 必须是 Unicode 字符串且不会被输出。
                        let token = descriptor["token"]
                            // 读取 hello token。
                            .as_str()
                            // 缺失 token 属于无效 descriptor。
                            .expect("descriptor token")
                            // 复制到 fixture 自有值。
                            .to_string();
                        // 返回最小公开连接信息。
                        return AgentEndpoint { endpoint, token };
                    }
                }
            }
            // 子进程提前退出时立即收集输出并失败。
            if let Some(status) = self.child.try_wait().expect("query demo process") {
                // 排空当前已有输出用于诊断。
                let output = self.take_output();
                // 报告进程状态但不输出 descriptor token。
                panic!(
                    "uix-lang-demo exited before discovery publication: {status}; output={output}"
                );
            }
            // 超过启动预算时停止等待。
            assert!(
                Instant::now() < deadline,
                "timed out waiting for Agent discovery descriptor"
            );
            // 使用短睡眠避免 busy loop。
            thread::sleep(Duration::from_millis(25));
        }
    }

    // 连接 descriptor 指定的同用户本机 Agent 管道。
    pub(crate) fn connect(&mut self, endpoint: &str) -> File {
        // 计算绝对连接超时点。
        let deadline = Instant::now() + START_TIMEOUT;
        // 等待 listener 接受当前唯一客户端。
        loop {
            // 以读写和独占共享模式打开 Windows named pipe。
            match OpenOptions::new()
                // 允许读取响应。
                .read(true)
                // 允许写入请求。
                .write(true)
                // 不允许其他文件句柄共享当前客户端。
                .share_mode(0)
                // 打开 descriptor 给出的精确端点。
                .open(endpoint)
            {
                // 成功时返回已连接管道。
                Ok(stream) => return stream,
                // 连接尚未就绪时检查进程和预算。
                Err(error) => {
                    // 进程退出后重试没有意义。
                    if let Some(status) = self.child.try_wait().expect("query demo process") {
                        // 报告连接失败与进程状态，不输出 token。
                        panic!(
                            "uix-lang-demo exited before Agent connection: {status}; error={error}"
                        );
                    }
                    // 超过预算时报告最后一次安全错误。
                    assert!(
                        Instant::now() < deadline,
                        "timed out connecting to Agent endpoint: {error}"
                    );
                    // 使用短睡眠避免 busy loop。
                    thread::sleep(Duration::from_millis(25));
                }
            }
        }
    }

    // 终止本 fixture 创建的进程并收集完整输出证据。
    pub(crate) fn stop_and_collect(&mut self) -> String {
        // 只在子进程仍存活时发送终止请求。
        if self
            .child
            .try_wait()
            .expect("query demo shutdown")
            .is_none()
        {
            // 测试完成后不把主演示窗口留在用户桌面。
            self.child.kill().expect("terminate spawned uix-lang-demo");
            // 等待内核确认子进程已经退出并关闭输出句柄。
            let _ = self.child.wait().expect("wait for uix-lang-demo exit");
        }
        // 返回两个通道的全部已写出内容。
        self.take_output()
    }

    // 合并并消费当前输出读取线程。
    fn take_output(&mut self) -> String {
        // 取走线程集合确保每个句柄只 join 一次。
        std::mem::take(&mut self.output_readers)
            // 转成拥有所有权的迭代器。
            .into_iter()
            // 等待每个读取线程完成并取得字符串。
            .map(|reader| reader.join().expect("join demo output reader"))
            // 保留 stdout 与 stderr 的完整内容。
            .collect::<Vec<_>>()
            // 合并为单一断言证据。
            .join("")
    }
}

// 在测试失败或提前返回时仍回收唯一子进程与临时目录。
impl Drop for DemoProcess {
    // 执行幂等 fixture 清理。
    fn drop(&mut self) {
        // 只终止尚未由 stop_and_collect 回收的子进程。
        if self.child.try_wait().ok().flatten().is_none() {
            // 忽略 Drop 期间的终止错误以保留原始 panic。
            let _ = self.child.kill();
            // 等待句柄关闭，避免遗留测试进程。
            let _ = self.child.wait();
        }
        // 等待读取线程退出，避免它们持有已删除目录之外的进程管道。
        for reader in std::mem::take(&mut self.output_readers) {
            // Drop 期间不覆盖原始失败。
            let _ = reader.join();
        }
        // 仅删除本 fixture 创建的精确临时根目录。
        let _ = fs::remove_dir_all(&self.discovery_root);
    }
}

// 在独立线程中持续排空一个子进程输出通道。
fn read_output(mut stream: impl Read + Send + 'static) -> JoinHandle<String> {
    // 启动只拥有当前管道的读取线程。
    thread::spawn(move || {
        // 创建拥有所有权的 UTF-8 输出缓冲。
        let mut output = String::new();
        // 读取到子进程关闭管道。
        stream
            // 持续追加到同一字符串。
            .read_to_string(&mut output)
            // 无法读取输出时测试证据不完整。
            .expect("read uix-lang-demo output");
        // 返回完整通道文本。
        output
    })
}

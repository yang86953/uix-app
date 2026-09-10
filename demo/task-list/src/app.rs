use crate::domain::{Statistics, TaskList, calculate_statistics};
use std::path::{Path, PathBuf};
use std::sync::{
    Arc, Mutex, Weak,
    atomic::{AtomicBool, AtomicU64, Ordering},
};
use std::thread::JoinHandle;
use uix_app::data::SettingsService;
use uix_app::prelude::*;

// 本示例只有一个原生窗口。领域/服务与外部 worker 由组合根持有，UIX 只消费投影。
#[derive(Clone)]
pub struct TaskApp(Arc<Inner>);

struct JobOwner {
    generation: u64,
    active: bool,
    cancel: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl Default for JobOwner {
    fn default() -> Self {
        Self {
            generation: 0,
            active: false,
            cancel: Arc::new(AtomicBool::new(false)),
            thread: None,
        }
    }
}

struct Inner {
    path: PathBuf,
    settings: SettingsService,
    data: State<TaskList>,
    draft: State<String>,
    ids: State<Vec<String>>,
    status: State<String>,
    stats_result: State<String>,
    show_stats: State<bool>,
    busy: State<bool>,
    dirty: State<bool>,
    writable: bool,
    revision: AtomicU64,
    closed: AtomicBool,
    handle: Mutex<Option<AppHandle>>,
    job: Mutex<JobOwner>,
}

impl TaskApp {
    pub fn new(path: PathBuf) -> Result<Self, String> {
        if !path.is_absolute() {
            return Err("保存路径必须是调用方指定的绝对路径".into());
        }
        let path_text = path.to_str().ok_or("保存路径不是有效 UTF-8")?;
        let settings = SettingsService::new();
        let loaded = settings
            .load(path_text)
            .map_err(|e| e.to_string())
            .and_then(|()| {
                settings
                    .get_struct::<TaskList>("task_list")
                    .map_err(|e| e.to_string())
            })
            .map(|list| list.unwrap_or_default())
            .and_then(|list| list.validate().map(|()| list));
        let (data, status, writable) = match loaded {
            Ok(list) => {
                let status = format!("已加载 {} 项；未保存编辑将在关闭时丢弃", list.tasks.len());
                (list, status, true)
            }
            Err(error) => (
                TaskList::default(),
                format!("加载失败（禁止编辑/保存，原文件保留）：{error}"),
                false,
            ),
        };
        let ids = data.tasks.iter().map(|t| t.id.to_string()).collect();
        Ok(Self(Arc::new(Inner {
            path,
            settings,
            data: State::new(data),
            draft: State::new(String::new()),
            ids: State::new(ids),
            status: State::new(status),
            stats_result: State::new("点击“开始统计”在本地后台计算".into()),
            show_stats: State::new(false),
            busy: State::new(false),
            dirty: State::new(false),
            writable,
            revision: AtomicU64::new(0),
            closed: AtomicBool::new(false),
            handle: Mutex::new(None),
            job: Mutex::new(JobOwner::default()),
        })))
    }

    pub fn path(&self) -> &Path {
        &self.0.path
    }
    pub fn data(&self) -> TaskList {
        self.0.data.get()
    }
    pub fn status(&self) -> String {
        self.0.status.get()
    }
    pub fn statistics_text(&self) -> String {
        self.0.stats_result.get()
    }
    pub fn is_dirty(&self) -> bool {
        self.0.dirty.get()
    }
    pub fn is_busy(&self) -> bool {
        self.0.busy.get()
    }

    pub fn attach(&self, handle: AppHandle) {
        *self.0.handle.lock().unwrap() = Some(handle);
    }

    pub fn application(&self) -> App {
        let root = self.clone();
        let startup = self.clone();
        App::new()
            .title("UIX 任务清单")
            .size(720, 680)
            .on_start(move |handle| startup.attach(handle))
            .root(move || root.view())
    }

    fn editable(&self) -> bool {
        self.0.writable && !self.0.closed.load(Ordering::Acquire)
    }

    fn edit(&self, operation: impl FnOnce(&mut TaskList) -> Result<(), String>) {
        if !self.editable() {
            return;
        }
        let mut list = self.0.data.get();
        match operation(&mut list) {
            Ok(()) => {
                self.cancel_statistics("任务已变更，请重新统计");
                self.0.revision.fetch_add(1, Ordering::AcqRel);
                self.0
                    .ids
                    .set(list.tasks.iter().map(|t| t.id.to_string()).collect());
                self.0.data.set(list);
                self.0.dirty.set(true);
                self.0.status.set("更改尚未保存".into());
            }
            Err(error) => self.0.status.set(format!("校验失败：{error}")),
        }
    }

    pub fn add(&self, title: String) {
        let mut added = false;
        self.edit(|list| {
            list.add(&title)?;
            added = true;
            Ok(())
        });
        if added {
            self.0.draft.set(String::new());
        }
    }

    pub fn toggle(&self, id: String) {
        self.edit(|list| list.toggle(id.parse().map_err(|_| "任务身份无效")?));
    }

    pub fn remove(&self, id: String) {
        self.edit(|list| list.remove(id.parse().map_err(|_| "任务身份无效")?));
    }

    pub fn save(&self) {
        if !self.editable() {
            return;
        }
        let list = self.0.data.get();
        // set_struct 仅改内存；仅 save 成功后才能更新“已保存”和业务脏标记。
        match self
            .0
            .settings
            .set_struct("task_list", &list)
            .and_then(|()| self.0.settings.save())
        {
            Ok(()) => {
                self.0.dirty.set(false);
                self.0.status.set(format!("已保存 {} 项", list.tasks.len()));
            }
            Err(error) => self
                .0
                .status
                .set(format!("保存失败（更改仍未落盘）：{error}")),
        }
    }

    pub fn navigate(&self, statistics: bool) {
        if self.0.closed.load(Ordering::Acquire) {
            return;
        }
        if !statistics && self.0.show_stats.get() {
            self.cancel_statistics("已离开统计页；旧请求不会覆盖结果");
        }
        self.0.show_stats.set(statistics);
    }

    pub fn cancel_statistics(&self, message: &str) {
        let mut job = self.0.job.lock().unwrap();
        job.cancel.store(true, Ordering::Release);
        job.generation += 1;
        job.active = false;
        drop(job);
        self.0.busy.set(false);
        self.0.stats_result.set(message.into());
    }

    pub fn start_statistics(&self) {
        if !self.editable() || !self.0.show_stats.get() {
            return;
        }
        let Some(handle) = self.0.handle.lock().unwrap().clone() else {
            self.0.stats_result.set("统计失败：原生窗口尚未就绪".into());
            return;
        };
        let mut job = self.0.job.lock().unwrap();
        if job.active || job.thread.as_ref().is_some_and(|t| !t.is_finished()) {
            drop(job);
            self.0
                .stats_result
                .set("统计仍在运行或取消回收中，请稍后再试".into());
            return;
        }
        if let Some(thread) = job.thread.take() {
            // 只在 is_finished 后 join；UI 事件路径不等待运行中的 worker。
            if thread.join().is_err() {
                drop(job);
                self.0
                    .stats_result
                    .set("统计失败：上次后台线程异常退出".into());
                return;
            }
        }
        job.generation += 1;
        let generation = job.generation;
        job.active = true;
        let cancel = Arc::new(AtomicBool::new(false));
        job.cancel = cancel.clone();
        let revision = self.0.revision.load(Ordering::Acquire);
        let list = self.0.data.get();
        let owner: Weak<Inner> = Arc::downgrade(&self.0);
        self.0.busy.set(true);
        self.0.stats_result.set("正在本地后台统计…".into());
        let spawned = std::thread::Builder::new()
            .name("task-list-statistics".into())
            .spawn(move || {
                let result = calculate_statistics(&list, &cancel);
                // () 不是回执；实际结果状态由以下 UI 闭包提交。
                handle.post_to_ui(move || {
                    if let Some(inner) = owner.upgrade() {
                        TaskApp(inner).accept_statistics(generation, revision, result);
                    }
                });
            });
        match spawned {
            Ok(thread) => job.thread = Some(thread),
            Err(error) => {
                job.active = false;
                drop(job);
                self.0.busy.set(false);
                self.0
                    .stats_result
                    .set(format!("统计失败：无法启动线程：{error}"));
            }
        }
    }

    fn accept_statistics(
        &self,
        generation: u64,
        revision: u64,
        result: Result<Statistics, String>,
    ) {
        let mut job = self.0.job.lock().unwrap();
        if self.0.closed.load(Ordering::Acquire) || !job.active || job.generation != generation {
            return;
        }
        job.active = false;
        let valid = self.0.show_stats.get() && self.0.revision.load(Ordering::Acquire) == revision;
        drop(job);
        self.0.busy.set(false);
        if !valid {
            return;
        }
        self.0.stats_result.set(match result {
            Ok(s) => format!(
                "统计完成：共 {} 项，已完成 {} 项，标题 {} 字符",
                s.total, s.completed, s.title_characters
            ),
            Err(error) => format!("统计失败：{error}"),
        });
    }

    // App::run 返回后由组合根调用；框架关窗清队列，但外部线程的取消/join 属于本应用。
    pub fn shutdown(&self) -> Result<(), String> {
        self.0.closed.store(true, Ordering::Release);
        let mut job = self.0.job.lock().unwrap();
        job.cancel.store(true, Ordering::Release);
        job.generation += 1;
        job.active = false;
        let thread = job.thread.take();
        drop(job);
        *self.0.handle.lock().unwrap() = None;
        self.0.busy.set(false);
        if let Some(thread) = thread {
            thread.join().map_err(|_| "后台统计线程异常退出")?;
        }
        Ok(())
    }

    pub fn view(&self) -> ViewNode {
        let draft = self.0.draft.clone();
        let task_ids = self.0.ids.clone();
        let message = self.0.status.clone();
        let stats_result = self.0.stats_result.clone();
        let show_stats = self.0.show_stats.clone();
        let busy = self.0.busy.clone();
        let dirty = self.0.dirty.clone();
        let writable = self.0.writable;
        let file_path = self.0.path.display().to_string();
        let on_add = {
            let app = self.clone();
            move |text: String| app.add(text)
        };
        let on_toggle = {
            let app = self.clone();
            move |id: String| app.toggle(id)
        };
        let on_remove = {
            let app = self.clone();
            move |id: String| app.remove(id)
        };
        let title_for = {
            let app = self.clone();
            move |id: String| {
                app.data()
                    .tasks
                    .iter()
                    .find(|t| t.id.to_string() == id)
                    .map(|t| format!("#{}  {}  {}", t.id, if t.done { "✓" } else { "○" }, t.title))
                    .unwrap_or_else(|| "任务已移除".into())
            }
        };
        let on_stats = {
            let app = self.clone();
            move || app.navigate(true)
        };
        let on_tasks = {
            let app = self.clone();
            move || app.navigate(false)
        };
        let on_save = {
            let app = self.clone();
            move || app.save()
        };
        let on_start_stats = {
            let app = self.clone();
            move || app.start_statistics()
        };
        let on_cancel_stats = {
            let app = self.clone();
            move || app.cancel_statistics("统计已取消")
        };
        uix!("src/main.uix")
    }
}

impl Drop for Inner {
    fn drop(&mut self) {
        let job = self.job.get_mut().unwrap_or_else(|e| e.into_inner());
        job.cancel.store(true, Ordering::Release);
        if let Some(thread) = job.thread.take() {
            let _ = thread.join();
        }
    }
}

// 只把窗口关闭交互内核接入 UIX 声明的按钮正文，不以 Rust 替代页面。
fn close_control(content: ViewNode) -> ViewNode {
    window_control(WindowControl::Close, content)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_current_page_request_and_revision_can_commit_statistics() {
        // 不运行原生线程：只检验本应用收到迟到结果时的业务提交门禁。
        let directory = std::env::temp_dir().join(format!(
            "uix-task-list-gate-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&directory).unwrap();
        let app = TaskApp::new(directory.join("tasks.json")).unwrap();
        app.add("第一项".into());
        app.navigate(true);
        let revision = app.0.revision.load(Ordering::Acquire);
        {
            let mut job = app.0.job.lock().unwrap();
            job.generation = 40;
            job.active = true;
        }
        let result = Statistics {
            total: 1,
            completed: 0,
            title_characters: 3,
        };
        app.0.busy.set(true);
        let before = app.statistics_text();
        app.accept_statistics(39, revision, Ok(result.clone()));
        assert_eq!(app.statistics_text(), before);
        assert!(app.is_busy()); // 旧请求不得清除新请求的忙碌状态。
        app.accept_statistics(40, revision, Ok(result.clone()));
        assert!(app.statistics_text().starts_with("统计完成"));
        assert!(!app.is_busy());
        app.navigate(false);
        let cancelled = app.statistics_text();
        app.accept_statistics(40, revision, Ok(result.clone()));
        assert_eq!(app.statistics_text(), cancelled);
        app.navigate(true);
        {
            let mut job = app.0.job.lock().unwrap();
            job.generation = 50;
            job.active = true;
        }
        app.0.revision.fetch_add(1, Ordering::AcqRel);
        app.accept_statistics(50, revision, Ok(result));
        assert_eq!(app.statistics_text(), cancelled);
        app.shutdown().unwrap();
        std::fs::remove_dir_all(directory).unwrap();
    }
}

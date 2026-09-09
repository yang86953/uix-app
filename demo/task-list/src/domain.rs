use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, Ordering};

pub const MAX_TASKS: usize = 128;
pub const MAX_TITLE_CHARS: usize = 80;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Task {
    pub id: u64,
    pub title: String,
    pub done: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TaskList {
    pub schema: u32,
    pub next_id: u64,
    pub tasks: Vec<Task>,
}

impl Default for TaskList {
    fn default() -> Self {
        Self {
            schema: 1,
            next_id: 1,
            tasks: Vec::new(),
        }
    }
}

impl TaskList {
    // 文件内容同样经过领域校验；损坏数据不能静默变为空清单再被保存覆盖。
    pub fn validate(&self) -> Result<(), String> {
        if self.schema != 1 {
            return Err("不支持的任务文件版本".into());
        }
        if self.tasks.len() > MAX_TASKS {
            return Err(format!("最多支持 {MAX_TASKS} 项任务"));
        }
        let mut ids = HashSet::new();
        for task in &self.tasks {
            if task.id == 0 || !ids.insert(task.id) || task.id >= self.next_id {
                return Err("任务身份重复或 next_id 无效".into());
            }
            if validate_title(&task.title)? != task.title {
                return Err("保存的标题含首尾空白".into());
            }
        }
        if self.next_id == 0 {
            return Err("next_id 无效".into());
        }
        Ok(())
    }

    pub fn add(&mut self, title: &str) -> Result<u64, String> {
        let title = validate_title(title)?;
        if self.tasks.len() >= MAX_TASKS {
            return Err(format!("最多支持 {MAX_TASKS} 项任务"));
        }
        let id = self.next_id;
        let next = id.checked_add(1).ok_or("任务身份已耗尽")?;
        self.tasks.push(Task {
            id,
            title,
            done: false,
        });
        self.next_id = next;
        Ok(id)
    }

    pub fn toggle(&mut self, id: u64) -> Result<(), String> {
        let task = self
            .tasks
            .iter_mut()
            .find(|t| t.id == id)
            .ok_or("该任务已不存在")?;
        task.done = !task.done;
        Ok(())
    }

    pub fn remove(&mut self, id: u64) -> Result<(), String> {
        let index = self
            .tasks
            .iter()
            .position(|t| t.id == id)
            .ok_or("该任务已不存在")?;
        self.tasks.remove(index);
        Ok(())
    }
}

fn validate_title(title: &str) -> Result<String, String> {
    let title = title.trim();
    let length = title.chars().count();
    if length == 0 || length > MAX_TITLE_CHARS {
        return Err(format!("标题需要 1–{MAX_TITLE_CHARS} 个字符"));
    }
    if title.chars().any(char::is_control) {
        return Err("标题不能包含换行或控制字符".into());
    }
    Ok(title.to_owned())
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Statistics {
    pub total: usize,
    pub completed: usize,
    pub title_characters: usize,
}

// 真正的本地计算，不用 sleep 模拟网络；输入规模由领域合同限定。
pub fn calculate_statistics(list: &TaskList, cancel: &AtomicBool) -> Result<Statistics, String> {
    if list.tasks.is_empty() {
        return Err("清单为空，请先添加任务再统计".into());
    }
    let mut result = Statistics {
        total: list.tasks.len(),
        completed: 0,
        title_characters: 0,
    };
    for task in &list.tasks {
        if cancel.load(Ordering::Acquire) {
            return Err("统计已取消".into());
        }
        result.completed += usize::from(task.done);
        result.title_characters += task.title.chars().count();
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validation_and_stable_ids_survive_removal() {
        let mut list = TaskList::default();
        assert!(list.add("  ").is_err());
        assert!(list.add(&"中".repeat(81)).is_err());
        assert!(list.add("中\n文").is_err());
        let first = list.add(" 学习 UIX ").unwrap();
        let second = list.add("保存数据").unwrap();
        list.remove(first).unwrap();
        let third = list.add("再次打开").unwrap();
        assert_eq!((first, second, third), (1, 2, 3));
        assert!(list.toggle(first).is_err());
        list.toggle(second).unwrap();
        assert!(list.tasks[0].done);
        list.validate().unwrap();
        list.tasks[1].id = second;
        assert!(list.validate().is_err());
    }

    #[test]
    fn bounded_statistics_success_empty_and_cancelled() {
        let mut list = TaskList::default();
        let cancel = AtomicBool::new(false);
        assert!(calculate_statistics(&list, &cancel).is_err());
        list.add("中文").unwrap();
        list.toggle(1).unwrap();
        assert_eq!(
            calculate_statistics(&list, &cancel).unwrap(),
            Statistics {
                total: 1,
                completed: 1,
                title_characters: 2
            }
        );
        cancel.store(true, Ordering::Release);
        assert_eq!(
            calculate_statistics(&list, &cancel).unwrap_err(),
            "统计已取消"
        );
    }
}

//! `ui/widgets/input/select/search.rs` 的 cfg(test) 单元测试体；模块层级不变，经 #[path] 引用，不进发布包。

use super::*;
use crate::ui::EventResult;
use crate::ui::widget_runtime::traits::EventHandler;
use crate::ui::widgets::input::select::SelectOption;

fn searchable_select(option_count: usize) -> Select {
    let mut select = Select::searchable();
    select.options = (0..option_count)
        .map(|index| SelectOption::new(format!("Option {index}"), format!("{index}")))
        .collect();
    select.open();
    select
}

fn type_query(select: &mut Select, text: &str) {
    assert_eq!(
        select.on_event(&crate::ui::SystemEvent::TextInput {
            text: text.to_owned()
        }),
        EventResult::Handled
    );
}

// 查询保持稳定时重复读取必须返回同一计数。
#[test]
fn repeated_count_reads_stay_stable() {
    let mut select = searchable_select(8);
    type_query(&mut select, "OPTION 3");
    assert_eq!(select.visible_row_count(), 1);
    assert_eq!(select.visible_row_count(), 1);
}

// 事件路径修改搜索词后，计数必须反映新查询而不是旧快照。
#[test]
fn query_edits_invalidate_cached_count() {
    let mut select = searchable_select(8);
    type_query(&mut select, "OPTION 3");
    assert_eq!(select.visible_row_count(), 1);
    // 回格删除尾部数字后查询回到词级前缀，可见集合重新扩大到全体。
    select.search_query.pop();
    assert_eq!(select.visible_row_count(), 8);
}

// sync_from 整体替换行集合输入后必须重新计数。
#[test]
fn sync_replacement_invalidate_cached_count() {
    let mut mounted = searchable_select(4);
    assert_eq!(mounted.visible_row_count(), 4);
    let replacement = searchable_select(9);
    mounted.sync_from(replacement);
    assert_eq!(mounted.visible_row_count(), 9);
}

// loading 切换属于行集合输入变化，缓存不得继续提供旧计数。
#[test]
fn loading_transition_invalidate_cached_count() {
    let mut mounted = searchable_select(6);
    assert_eq!(mounted.visible_row_count(), 6);
    let mut loading_next = searchable_select(6);
    loading_next.loading = true;
    mounted.sync_from(loading_next);
    assert_eq!(mounted.visible_row_count(), 0);
}

// 尺寸观测只服务于性能记录登记，忽略入口不参与常规验证。
#[test]
#[ignore = "手工尺寸观测"]
fn probe_select_struct_size() {
    eprintln!("SELECT_SIZE={}", std::mem::size_of::<Select>());
}

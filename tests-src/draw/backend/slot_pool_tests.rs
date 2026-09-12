//! `draw/backend/slot_pool.rs` 的 cfg(test) 单元测试体；模块层级不变，经 #[path] 引用，不进发布包。

use super::SlotPool;

#[test]
fn reuses_freed_ids_and_compacts_tail() {
    let mut pool = SlotPool::new();
    let a = pool.insert('a');
    let b = pool.insert('b');
    let c = pool.insert('c');
    assert_eq!((a, b, c), (0, 1, 2));

    pool.remove(b);
    assert_eq!(pool.insert('d'), 1);
    pool.remove(a);
    pool.remove(c);
    pool.remove(1);
    pool.compact();

    assert_eq!(pool.len(), 0);
    assert_eq!(pool.insert('e'), 0);
    assert_eq!(pool.occupied_ids().collect::<Vec<_>>(), vec![0]);
}

#[test]
fn remove_of_empty_or_unknown_id_yields_none_without_free_id() {
    let mut pool = SlotPool::new();
    let a = pool.insert(7);
    assert_eq!(pool.remove(a), Some(7));
    assert_eq!(pool.remove(a), None);
    assert_eq!(pool.remove(999), None);
    // 重复释放与未知释放都不得制造第二个空闲 id。
    let reused = pool.insert(8);
    pool.remove(reused);
    assert_eq!(pool.insert(9), reused);
}

#[test]
fn two_mut_borrows_disjoint_slots() {
    let mut pool = SlotPool::new();
    let a = pool.insert(1);
    let b = pool.insert(2);
    let (left, right) = pool.get_two_mut(a, b);
    *left.unwrap() += 10;
    *right.unwrap() += 20;
    assert_eq!(pool.get(a), Some(&11));
    assert_eq!(pool.get(b), Some(&22));

    // 相同 id 请求前者为 None，不产生别名借用。
    let (same, other) = pool.get_two_mut(a, a);
    assert!(same.is_none());
    assert_eq!(other.map(|v| *v), Some(11));
}

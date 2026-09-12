//! 离屏 Picture 槽位池 — CPU 像素池与 GPU RHI 池共用的唯一簿记。
//!
//! 只拥有 id 分配、槽位填充、释放回收与尾部压缩四件与资源类型无关的
//! 簿记语义；资源本体（像素表面 / RHI 纹理）的创建、销毁顺序与失败
//! 语义仍由各 backend 持有。

#[derive(Debug)]
pub(crate) struct SlotPool<T> {
    slots: Vec<Option<T>>,
    free_ids: Vec<u32>,
    next_id: u32,
}

impl<T> Default for SlotPool<T> {
    fn default() -> Self {
        Self {
            slots: Vec::new(),
            free_ids: Vec::new(),
            next_id: 0,
        }
    }
}

impl<T> SlotPool<T> {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    /// 释放全部槽位与簿记状态；资源本体的检查式销毁由调用方先行完成。
    pub(crate) fn clear(&mut self) {
        self.slots.clear();
        self.free_ids.clear();
        self.next_id = 0;
    }

    /// 分配下一个 id（优先复用空闲 id）并填充槽位，返回新 id。
    pub(crate) fn insert(&mut self, value: T) -> u32 {
        let id = if let Some(id) = self.free_ids.pop() {
            id
        } else {
            let id = self.next_id;
            self.next_id = self.next_id.saturating_add(1);
            id
        };
        let idx = id as usize;
        while self.slots.len() <= idx {
            self.slots.push(None);
        }
        self.slots[idx] = Some(value);
        id
    }

    /// 取出槽位值并把 id 归还空闲表；空槽或越界 id 返回 `None` 且不产生
    /// 空闲 id，调用方不得对同一资源重复释放。
    pub(crate) fn remove(&mut self, id: u32) -> Option<T> {
        let value = self.slots.get_mut(id as usize)?.take()?;
        self.free_ids.push(id);
        Some(value)
    }

    pub(crate) fn get(&self, id: u32) -> Option<&T> {
        self.slots.get(id as usize)?.as_ref()
    }

    pub(crate) fn get_mut(&mut self, id: u32) -> Option<&mut T> {
        self.slots.get_mut(id as usize)?.as_mut()
    }

    /// 同时可变借用两个槽位；相同 id 时前者为 `None`，由调用方先行拒绝。
    pub(crate) fn get_two_mut(
        &mut self,
        first: u32,
        second: u32,
    ) -> (Option<&mut T>, Option<&mut T>) {
        let (low, high) = if first <= second {
            (first, second)
        } else {
            (second, first)
        };
        let (before, after) = self.slots.split_at_mut(high as usize);
        let high_slot = after.first_mut().and_then(Option::as_mut);
        let low_slot = before.get_mut(low as usize).and_then(Option::as_mut);
        if first <= second {
            (low_slot, high_slot)
        } else {
            (high_slot, low_slot)
        }
    }

    /// 返回当前仍占用资源的全部 id，升序。
    pub(crate) fn occupied_ids(&self) -> impl Iterator<Item = u32> + '_ {
        self.slots
            .iter()
            .enumerate()
            .filter_map(|(id, slot)| slot.as_ref().map(|_| id as u32))
    }

    pub(crate) fn iter_values(&self) -> impl Iterator<Item = &T> {
        self.slots.iter().flatten()
    }

    pub(crate) fn iter_values_mut(&mut self) -> impl Iterator<Item = &mut T> {
        self.slots.iter_mut().flatten()
    }

    /// 槽位数组长度（含尾部之前的空洞），仅用于诊断与测试观测。
    pub(crate) fn len(&self) -> usize {
        self.slots.len()
    }

    /// 截掉尾部连续空洞，丢弃越界空闲 id，并把下一新 id 收敛为槽位长度。
    pub(crate) fn compact(&mut self) {
        while self.slots.last().is_some_and(Option::is_none) {
            self.slots.pop();
        }
        self.free_ids
            .retain(|id| (*id as usize) < self.slots.len());
        self.next_id = self.slots.len() as u32;
    }
}

#[cfg(test)]
#[path = "../../../tests-src/draw/backend/slot_pool_tests.rs"]
mod tests;


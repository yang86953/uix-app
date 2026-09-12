use super::WidgetSnapshotFields;

/// 允许组件直接导出其类型化配置与运行状态快照。
pub trait SnapshotSource {
    /// 捕获组件当前的类型专属快照字段。
    fn snapshot_fields(&self) -> WidgetSnapshotFields;
}

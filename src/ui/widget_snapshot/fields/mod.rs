use std::sync::Arc;

use crate::draw::Color;
use crate::draw::geometry::spatial::PhysicalUnit;
// 反馈 capability 启用时才需要警告提示状态级别。
#[cfg(feature = "feedback")]
use crate::platform::capabilities::StatusLevel;
use crate::platform::windowing::{ControlSize, ScrollDirection};
use crate::ui::form::FormLayout;
use crate::ui::layout::{AlignItems, FlexDirection, JustifyContent};
// 引入主题感知颜色值与通用样式快照类型。
use crate::ui::theme::style::{ColorValue, Style, StyleSet};
use crate::ui::widgets::window_chrome::WindowControl;
use crate::ui::widgets::*;
// FloatButton 与可选反馈组件共同使用公开浮层方位。
use crate::ui::Placement;
// Modal 与 Drawer 快照保存统一 overlay backdrop 请求。
#[cfg(feature = "feedback")]
use crate::ui::OverlayBackdropBlur;

use super::{SnapshotCollapsePanel, SnapshotField, SnapshotTransferItem};
// 反馈 capability 启用时才引入专属气泡确认框快照模型。
#[cfg(feature = "feedback")]
// 该类型只服务同步门控的 Popconfirm 枚举变体。
use super::SnapshotPopconfirm;
// 树组件 capability 启用时才引入树节点快照模型。
#[cfg(feature = "tree-widgets")]
// 该类型只服务同步门控的 Tree 与 TreeSelect 枚举变体。
use super::SnapshotTreeNode;
// 表格 capability 启用时才引入专属快照列模型。
#[cfg(feature = "table")]
// 两个类型只服务同步门控的 Table 枚举变体。
use super::{SnapshotTableColumn, SnapshotTableColumnGroup};

mod accessibility;

// 两段私有宏源共同描述同一个公开枚举，避免单个源码文件突破行数上限。
include!("schema_part_one.rs");
// 第二段接续第一段并由根模块统一完成类型定义。
include!("schema_part_two.rs");

// 根模块唯一拥有公开类型定义及其派生契约。
macro_rules! finish_snapshot_fields {
    // 接收两个分段累积出的全部枚举变体。
    ($($variants:tt)*) => {
        // 保持原有调试、克隆与相等比较能力。
        #[derive(Debug, Clone, PartialEq)]
        /// 内置与自定义组件用于比较、语义派生和声明协调的类型化字段快照。
        pub enum SnapshotFields {
            // 分段宏只提供变体，不改变公开类型路径。
            $($variants)*
        }
    };
}

// 从第一段开始累积，并在第二段结束后生成公开枚举。
snapshot_fields_part_one!(finish_snapshot_fields);

//! `TerminalScreen` 的构造、布局、reconcile 与快照方法。

use crate::core::{Rect, Size};
use crate::ui::SnapshotFields;

use super::TERMINAL_SCREEN_VISUAL_REF;
use super::screen_widget::TerminalScreen;
use super::session::{TerminalSession, TerminalSessionStatus};

impl TerminalScreen {
    /// 用一个已显式启动的会话创建真实终端屏幕。
    ///
    /// 组件不创建进程：`session` 必须来自 [`TerminalSession::spawn`]；
    /// clone 共享同一会话。默认内在尺寸由 UIX 视觉声明提供。
    pub fn new(session: &TerminalSession) -> Self {
        Self {
            session: session.clone(),
            focused: std::cell::Cell::new(false),
            suppress_next_text: std::cell::Cell::new(false),
            last_frame: std::cell::Cell::new(None),
            synced_grid: std::cell::Cell::new((0, 0)),
            view_style: crate::ui::theme::style::Style::default(),
            visual: TERMINAL_SCREEN_VISUAL_REF,
        }
    }

    /// 返回组件绑定的会话句柄（clone 共享同一会话）。
    pub fn session(&self) -> &TerminalSession {
        &self.session
    }

    // 屏幕是固定网格视口：内在尺寸由声明默认值给出。
    pub(crate) fn intrinsic_size(&self) -> Size {
        Size::new(
            self.view_style
                .width
                .unwrap_or(self.visual.geometry.default_width),
            self.view_style
                .height
                .unwrap_or(self.visual.geometry.default_height),
        )
    }

    // 公共布局声明通过同一内核测量，显式零 Flex 权重同样生效。
    pub(crate) fn apply_view_layout_style(
        &mut self,
        style: &crate::ui::theme::style::Style,
        flex_grow: Option<f32>,
        flex_shrink: Option<f32>,
    ) {
        self.view_style = crate::ui::theme::style::Style {
            width: style.width,
            height: style.height,
            flex_grow: style.flex_grow,
            flex_shrink: style.flex_shrink,
            margin: style.margin,
            align_self: style.align_self,
            ..Default::default()
        };
        if let Some(value) = flex_grow {
            self.view_style.flex_grow = value;
        }
        if let Some(value) = flex_shrink {
            self.view_style.flex_shrink = value;
        }
    }

    // 帧尺寸变化即布局声明变化；会话与网格运行态不属于声明。
    pub(crate) fn reconcile_layout_changed(&self, next: &Self) -> bool {
        self.view_style != next.view_style || self.visual.geometry != next.visual.geometry
    }

    pub(crate) fn local_frame(&self) -> Rect {
        self.last_frame
            .get()
            .map(|frame| Self::normalized_frame(Rect::new(0.0, 0.0, frame.w, frame.h)))
            .unwrap_or_else(|| {
                Rect::new(
                    0.0,
                    0.0,
                    self.visual.geometry.default_width,
                    self.visual.geometry.default_height,
                )
            })
    }

    pub(crate) fn normalized_frame(frame: Rect) -> Rect {
        Rect::new(
            frame.x,
            frame.y,
            if frame.w.is_finite() {
                frame.w.max(0.0)
            } else {
                0.0
            },
            if frame.h.is_finite() {
                frame.h.max(0.0)
            } else {
                0.0
            },
        )
    }

    // 网格原点（相对帧顶）与单列宽度；退出状态带占据网格上方高度。
    pub(crate) fn grid_origin(&self, frame: Rect) -> (f32, f32) {
        let geometry = self.visual.geometry;
        let status_band = matches!(self.session.status(), TerminalSessionStatus::Exited { .. })
            .then_some(geometry.status_height)
            .unwrap_or(0.0);
        (
            frame.y + geometry.padding_y + status_band,
            geometry.cell_width,
        )
    }

    // 按键字节写入会话；失败按边界观察处置（写入端无恢复所有者）。
    pub(crate) fn write_to_session(&self, bytes: &[u8]) {
        if let Err(error) = self.session.write(bytes) {
            // 会话关闭后的残留按键是预期事实，其余写入失败保留观察。
            if error.code() != crate::core::Errc::InvalidOperation {
                crate::diagnostics::observe_boundary_error("ui::terminal-screen", &error);
            }
        }
    }

    pub(crate) fn sync_from(&mut self, next: Self) {
        self.visual = next.visual;
        self.session = next.session;
        self.view_style = next.view_style;
        // 焦点、网格同步与文本抑制属于组件运行态，跨帧保留。
    }

    pub(crate) fn snapshot_fields(&self) -> SnapshotFields {
        let rows: Vec<String> = self
            .session
            .rows()
            .iter()
            .map(super::vt::TerminalRow::plain)
            .collect();
        let (cursor_row, cursor_col) = self.session.cursor();
        let (cols, screen_rows) = self.session.screen_size();
        let (running, exit_code) = match self.session.status() {
            TerminalSessionStatus::Running => (true, None),
            TerminalSessionStatus::Exited { code } => (false, code),
        };
        SnapshotFields::TerminalScreen {
            rows,
            cursor: (cursor_row, cursor_col),
            size: (cols, screen_rows),
            running,
            exit_code,
        }
    }

    // 语义快照比较组件声明配置；会话运行态由 pump 驱动。
    pub(crate) fn reconcile_config_changed(&self, next: &Self) -> bool {
        !self.session.same_session(&next.session)
            || self.visual != next.visual
            || self.view_style != next.view_style
    }
}

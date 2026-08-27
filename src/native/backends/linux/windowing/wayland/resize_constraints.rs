// Wayland 窗口 resize 约束状态机与 xdg_toplevel 协议 Adapter。

use wayland_protocols::xdg::shell::client::xdg_toplevel;

use crate::core::error::{Errc, Error, Result};

use super::compat::Main;

type LogicalExtent = (i32, i32);

// 状态机输出的唯一 xdg_toplevel resize 约束命令。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum WaylandResizeConstraintRequest {
    // 当前转换不触碰 compositor 已持有的约束。
    Unchanged,
    // 可调整状态下只更新用户最小尺寸。
    SetMinimum(LogicalExtent),
    // 可调整状态下只更新用户最大尺寸。
    SetMaximum(LogicalExtent),
    // 锁定或解锁时完整替换两侧约束；None 按协议清除该侧。
    Replace {
        minimum: Option<LogicalExtent>,
        maximum: Option<LogicalExtent>,
    },
}

// 尚未提交的纯状态转换；协议成功前不得发布 next。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct WaylandResizeConstraintTransition {
    next: WaylandResizeConstraintState,
    request: WaylandResizeConstraintRequest,
}

impl WaylandResizeConstraintTransition {
    pub(super) const fn request(self) -> WaylandResizeConstraintRequest {
        self.request
    }
}

// 单窗口用户约束、有效尺寸与 resizable 决策的唯一权威。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct WaylandResizeConstraintState {
    user_minimum: Option<LogicalExtent>,
    user_maximum: Option<LogicalExtent>,
    effective_extent: Option<LogicalExtent>,
    resizable: bool,
}

impl WaylandResizeConstraintState {
    // 初始尺寸只有严格为正时才可成为后续锁定依据。
    pub(super) fn new(width: i32, height: i32) -> Self {
        Self {
            user_minimum: None,
            user_maximum: None,
            effective_extent: positive_extent(width, height),
            resizable: true,
        }
    }

    // 规划 resizable 边沿；锁定必须使用最新有效 logical 客户区尺寸。
    pub(super) fn plan_set_resizable(
        self,
        operation: &str,
        resizable: bool,
    ) -> Result<WaylandResizeConstraintTransition> {
        if self.resizable == resizable {
            return Ok(self.transition(self, WaylandResizeConstraintRequest::Unchanged));
        }

        let mut next = self;
        next.resizable = resizable;
        let request = if resizable {
            WaylandResizeConstraintRequest::Replace {
                minimum: self.user_minimum,
                maximum: self.user_maximum,
            }
        } else {
            let effective_extent = self.effective_extent.ok_or_else(|| {
                Error::new(
                    Errc::InvalidState,
                    format!("{operation}: Wayland has no current positive logical client extent"),
                )
            })?;
            WaylandResizeConstraintRequest::Replace {
                minimum: Some(effective_extent),
                maximum: Some(effective_extent),
            }
        };
        Ok(self.transition(next, request))
    }

    // 用户最小尺寸始终更新权威；锁定期间不触碰当前固定协议约束。
    pub(super) fn plan_set_user_minimum(
        self,
        width: i32,
        height: i32,
    ) -> WaylandResizeConstraintTransition {
        let extent = (width, height);
        let mut next = self;
        next.user_minimum = Some(extent);
        let request = if self.resizable {
            WaylandResizeConstraintRequest::SetMinimum(extent)
        } else {
            WaylandResizeConstraintRequest::Unchanged
        };
        self.transition(next, request)
    }

    // 用户最大尺寸始终更新权威；锁定期间不触碰当前固定协议约束。
    pub(super) fn plan_set_user_maximum(
        self,
        width: i32,
        height: i32,
    ) -> WaylandResizeConstraintTransition {
        let extent = (width, height);
        let mut next = self;
        next.user_maximum = Some(extent);
        let request = if self.resizable {
            WaylandResizeConstraintRequest::SetMaximum(extent)
        } else {
            WaylandResizeConstraintRequest::Unchanged
        };
        self.transition(next, request)
    }

    // 接收程序化 resize 或 compositor configure 确认的有效 logical 尺寸。
    pub(super) fn plan_effective_extent(
        self,
        width: i32,
        height: i32,
    ) -> Option<WaylandResizeConstraintTransition> {
        let extent = positive_extent(width, height)?;
        let mut next = self;
        next.effective_extent = Some(extent);
        let request = if !self.resizable && self.effective_extent != Some(extent) {
            WaylandResizeConstraintRequest::Replace {
                minimum: Some(extent),
                maximum: Some(extent),
            }
        } else {
            WaylandResizeConstraintRequest::Unchanged
        };
        Some(self.transition(next, request))
    }

    // 只有协议或测试 Adapter 成功后才原子发布规划状态。
    pub(super) fn commit_after(
        &mut self,
        transition: WaylandResizeConstraintTransition,
        apply: impl FnOnce(WaylandResizeConstraintRequest) -> Result<()>,
    ) -> Result<()> {
        apply(transition.request)?;
        *self = transition.next;
        Ok(())
    }

    const fn transition(
        self,
        next: Self,
        request: WaylandResizeConstraintRequest,
    ) -> WaylandResizeConstraintTransition {
        WaylandResizeConstraintTransition { next, request }
    }
}

const fn positive_extent(width: i32, height: i32) -> Option<LogicalExtent> {
    if width > 0 && height > 0 {
        Some((width, height))
    } else {
        None
    }
}

// 所有 xdg_toplevel min/max 请求的唯一协议 Adapter。
pub(super) struct WaylandResizeConstraintAdapter<'a> {
    toplevel: &'a Main<xdg_toplevel::XdgToplevel>,
}

impl<'a> WaylandResizeConstraintAdapter<'a> {
    // 同步入口必须先取得活动 proxy，失败时状态机尚未提交。
    pub(super) fn require(
        toplevel: Option<&'a Main<xdg_toplevel::XdgToplevel>>,
        operation: &str,
    ) -> Result<Self> {
        let toplevel = toplevel.ok_or_else(|| {
            Error::new(
                Errc::InvalidState,
                format!("{operation}: Wayland xdg_toplevel is unavailable"),
            )
        })?;
        Ok(Self { toplevel })
    }

    // configure callback 自身已提供仍存活的 xdg_toplevel proxy。
    pub(super) const fn from_proxy(toplevel: &'a Main<xdg_toplevel::XdgToplevel>) -> Self {
        Self { toplevel }
    }

    // 把状态机命令机械编码为协议请求，不持有任何策略或权威状态。
    pub(super) fn apply(&self, request: WaylandResizeConstraintRequest) -> Result<()> {
        match request {
            WaylandResizeConstraintRequest::Unchanged => {}
            WaylandResizeConstraintRequest::SetMinimum((width, height)) => {
                self.toplevel.set_min_size(width, height);
            }
            WaylandResizeConstraintRequest::SetMaximum((width, height)) => {
                self.toplevel.set_max_size(width, height);
            }
            WaylandResizeConstraintRequest::Replace { minimum, maximum } => {
                // 同一 surface commit 前先清除旧约束，避免锁定边沿产生瞬时交叉。
                self.toplevel.set_min_size(0, 0);
                self.toplevel.set_max_size(0, 0);
                if let Some((width, height)) = minimum {
                    self.toplevel.set_min_size(width, height);
                }
                if let Some((width, height)) = maximum {
                    self.toplevel.set_max_size(width, height);
                }
            }
        }
        Ok(())
    }
}

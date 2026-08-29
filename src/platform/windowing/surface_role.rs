//! 平台中立的窗口 surface role 与桌面 layer 配置值。

/// 窗口在合成器中的公开角色。
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum WindowSurfaceRole {
    /// 普通应用窗口；Wayland 使用 `xdg_toplevel`。
    #[default]
    Toplevel,
    /// 桌面外壳 surface；Wayland 使用 wlr layer-shell。
    DesktopLayer(DesktopLayerConfig),
}

/// 桌面 surface 所在的合成层。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DesktopLayer {
    Background,
    Bottom,
    #[default]
    Top,
    Overlay,
}

/// 桌面 surface 可锚定的输出边缘。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DesktopAnchor {
    Top,
    Bottom,
    Left,
    Right,
}

/// 桌面 surface 的键盘交互方式。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DesktopKeyboardInteractivity {
    /// 不接收键盘焦点，适用于壁纸和只读面板。
    #[default]
    None,
    /// 独占键盘焦点，适用于需要阻止其他窗口输入的覆盖层。
    Exclusive,
    /// 由合成器按需授予键盘焦点，适用于启动器等临时界面。
    OnDemand,
}

/// Wayland layer-shell 桌面 surface 的平台中立配置。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DesktopLayerConfig {
    layer: DesktopLayer,
    anchors: u8,
    exclusive_zone: i32,
    keyboard_interactivity: DesktopKeyboardInteractivity,
    namespace: String,
}

impl DesktopLayerConfig {
    const TOP: u8 = 1 << 0;
    const BOTTOM: u8 = 1 << 1;
    const LEFT: u8 = 1 << 2;
    const RIGHT: u8 = 1 << 3;

    /// 创建指定合成层的桌面 surface 配置。
    pub fn new(layer: DesktopLayer) -> Self {
        Self {
            layer,
            anchors: 0,
            exclusive_zone: 0,
            keyboard_interactivity: DesktopKeyboardInteractivity::None,
            namespace: "uix-app".to_owned(),
        }
    }

    /// 设置或清除一个输出边缘锚点。
    pub fn anchor(mut self, anchor: DesktopAnchor, enabled: bool) -> Self {
        let bit = Self::anchor_bit(anchor);
        if enabled {
            self.anchors |= bit;
        } else {
            self.anchors &= !bit;
        }
        self
    }

    /// 设置独占区；`-1` 表示不受其他 surface 独占区影响，非负值保留对应逻辑像素。
    pub fn exclusive_zone(mut self, zone: i32) -> Self {
        self.exclusive_zone = zone;
        self
    }

    /// 设置键盘交互方式。
    pub fn keyboard_interactivity(mut self, interactivity: DesktopKeyboardInteractivity) -> Self {
        self.keyboard_interactivity = interactivity;
        self
    }

    /// 设置 layer-shell namespace，供合成器识别桌面组件类型。
    pub fn namespace(mut self, namespace: impl Into<String>) -> Self {
        self.namespace = namespace.into();
        self
    }

    pub fn layer(&self) -> DesktopLayer {
        self.layer
    }

    pub fn is_anchored(&self, anchor: DesktopAnchor) -> bool {
        self.anchors & Self::anchor_bit(anchor) != 0
    }

    pub fn exclusive_zone_value(&self) -> i32 {
        self.exclusive_zone
    }

    pub fn keyboard_interactivity_value(&self) -> DesktopKeyboardInteractivity {
        self.keyboard_interactivity
    }

    pub fn namespace_value(&self) -> &str {
        &self.namespace
    }

    /// 在进入原生协议前检查公开配置，避免产生协议级断开。
    pub fn validate(&self) -> Result<(), &'static str> {
        if self.exclusive_zone < -1 {
            return Err("exclusive_zone 只允许 -1 或非负值");
        }
        if self.namespace.is_empty() || self.namespace.len() > 128 || self.namespace.contains('\0')
        {
            return Err("namespace 必须为 1..=128 字节且不能包含 NUL");
        }
        Ok(())
    }

    const fn anchor_bit(anchor: DesktopAnchor) -> u8 {
        match anchor {
            DesktopAnchor::Top => Self::TOP,
            DesktopAnchor::Bottom => Self::BOTTOM,
            DesktopAnchor::Left => Self::LEFT,
            DesktopAnchor::Right => Self::RIGHT,
        }
    }
}

impl Default for DesktopLayerConfig {
    fn default() -> Self {
        Self::new(DesktopLayer::Top)
    }
}

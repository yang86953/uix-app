//! Ant Design 5 预设色板与类型化访问入口。

use crate::draw::Color;

/// 每条主色色板的色阶数量。
pub const COLOR_SCALE_LEN: usize = 10;
/// Ant Design 预设主色数量。
pub const PRIMARY_HUE_COUNT: usize = 12;
/// 主色在从浅到深色阶中的零基索引。
pub const PRIMARY_SHADE_INDEX: usize = 5;

/// 一条从浅到深排列的 10 阶色板。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ColorScale([Color; COLOR_SCALE_LEN]);

impl ColorScale {
    /// 从固定长度颜色数组构造色板。
    pub const fn new(colors: [Color; COLOR_SCALE_LEN]) -> Self {
        Self(colors)
    }

    /// 返回完整色阶；索引 0 最浅，索引 9 最深。
    pub const fn colors(&self) -> &[Color; COLOR_SCALE_LEN] {
        &self.0
    }

    /// 返回指定色阶，越界时返回 `None`。
    pub const fn shade(&self, index: usize) -> Option<Color> {
        if index < COLOR_SCALE_LEN {
            Some(self.0[index])
        } else {
            None
        }
    }

    /// 返回第 6 格主色。
    pub const fn primary(&self) -> Color {
        self.0[PRIMARY_SHADE_INDEX]
    }
}

/// Ant Design 12 个预设主色色相。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum PrimaryHue {
    Red,
    Volcano,
    Orange,
    Gold,
    Yellow,
    Lime,
    Green,
    Cyan,
    Blue,
    Geekblue,
    Purple,
    Magenta,
}

impl PrimaryHue {
    /// 以 `from_primaries` 接受的稳定顺序返回全部色相。
    pub const ALL: [Self; PRIMARY_HUE_COUNT] = [
        Self::Red,
        Self::Volcano,
        Self::Orange,
        Self::Gold,
        Self::Yellow,
        Self::Lime,
        Self::Green,
        Self::Cyan,
        Self::Blue,
        Self::Geekblue,
        Self::Purple,
        Self::Magenta,
    ];

    /// 返回色相在稳定顺序中的零基索引。
    pub const fn index(self) -> usize {
        self as usize
    }

    /// 返回用于令牌名的稳定小写名称。
    pub const fn name(self) -> &'static str {
        match self {
            Self::Red => "red",
            Self::Volcano => "volcano",
            Self::Orange => "orange",
            Self::Gold => "gold",
            Self::Yellow => "yellow",
            Self::Lime => "lime",
            Self::Green => "green",
            Self::Cyan => "cyan",
            Self::Blue => "blue",
            Self::Geekblue => "geekblue",
            Self::Purple => "purple",
            Self::Magenta => "magenta",
        }
    }

    /// 返回该色相的预设 10 阶色板。
    pub const fn palette(self) -> &'static ColorScale {
        match self {
            Self::Red => &RED_PALETTE,
            Self::Volcano => &VOLCANO_PALETTE,
            Self::Orange => &ORANGE_PALETTE,
            Self::Gold => &GOLD_PALETTE,
            Self::Yellow => &YELLOW_PALETTE,
            Self::Lime => &LIME_PALETTE,
            Self::Green => &GREEN_PALETTE,
            Self::Cyan => &CYAN_PALETTE,
            Self::Blue => &BLUE_PALETTE,
            Self::Geekblue => &GEEKBLUE_PALETTE,
            Self::Purple => &PURPLE_PALETTE,
            Self::Magenta => &MAGENTA_PALETTE,
        }
    }

    /// 返回该色相第 6 格的主色。
    pub const fn primary(self) -> Color {
        self.palette().primary()
    }
}

const fn rgb(hex: u32) -> Color {
    Color::from_rgb(
        ((hex >> 16) & 0xff) as u8,
        ((hex >> 8) & 0xff) as u8,
        (hex & 0xff) as u8,
    )
}

macro_rules! color_scale {
    ($($hex:expr),+ $(,)?) => {
        ColorScale::new([$(rgb($hex)),+])
    };
}

/// Red 预设色板。
pub const RED_PALETTE: ColorScale = color_scale![
    0xfff1f0, 0xffccc7, 0xffa39e, 0xff7875, 0xff4d4f, 0xf5222d, 0xcf1322, 0xa8071a, 0x820014,
    0x5c0011,
];

/// Volcano 预设色板。
pub const VOLCANO_PALETTE: ColorScale = color_scale![
    0xfff2e8, 0xffd8bf, 0xffbb96, 0xff9c6e, 0xff7a45, 0xfa541c, 0xd4380d, 0xad2102, 0x871400,
    0x610b00,
];

/// Orange 预设色板。
pub const ORANGE_PALETTE: ColorScale = color_scale![
    0xfff7e6, 0xffe7ba, 0xffd591, 0xffc069, 0xffa940, 0xfa8c16, 0xd46b08, 0xad4e00, 0x873800,
    0x612500,
];

/// Gold 预设色板。
pub const GOLD_PALETTE: ColorScale = color_scale![
    0xfffbe6, 0xfff1b8, 0xffe58f, 0xffd666, 0xffc53d, 0xfaad14, 0xd48806, 0xad6800, 0x874d00,
    0x613400,
];

/// Yellow 预设色板。
pub const YELLOW_PALETTE: ColorScale = color_scale![
    0xfeffe6, 0xffffb8, 0xfffb8f, 0xfff566, 0xffec3d, 0xfadb14, 0xd4b106, 0xad8b00, 0x876800,
    0x614700,
];

/// Lime 预设色板。
pub const LIME_PALETTE: ColorScale = color_scale![
    0xfcffe6, 0xf4ffb8, 0xeaff8f, 0xd3f261, 0xbae637, 0xa0d911, 0x7cb305, 0x5b8c00, 0x3f6600,
    0x254000,
];

/// Green 预设色板。
pub const GREEN_PALETTE: ColorScale = color_scale![
    0xf6ffed, 0xd9f7be, 0xb7eb8f, 0x95de64, 0x73d13d, 0x52c41a, 0x389e0d, 0x237804, 0x135200,
    0x092b00,
];

/// Cyan 预设色板。
pub const CYAN_PALETTE: ColorScale = color_scale![
    0xe6fffb, 0xb5f5ec, 0x87e8de, 0x5cdbd3, 0x36cfc9, 0x13c2c2, 0x08979c, 0x006d75, 0x00474f,
    0x002329,
];

/// Blue 预设色板。
pub const BLUE_PALETTE: ColorScale = color_scale![
    0xe6f4ff, 0xbae0ff, 0x91caff, 0x69b1ff, 0x4096ff, 0x1677ff, 0x0958d9, 0x003eb3, 0x002c8c,
    0x001d66,
];

/// Geekblue 预设色板。
pub const GEEKBLUE_PALETTE: ColorScale = color_scale![
    0xf0f5ff, 0xd6e4ff, 0xadc6ff, 0x85a5ff, 0x597ef7, 0x2f54eb, 0x1d39c4, 0x10239e, 0x061178,
    0x030852,
];

/// Purple 预设色板。
pub const PURPLE_PALETTE: ColorScale = color_scale![
    0xf9f0ff, 0xefdbff, 0xd3adf7, 0xb37feb, 0x9254de, 0x722ed1, 0x531dab, 0x391085, 0x22075e,
    0x120338,
];

/// Magenta 预设色板。
pub const MAGENTA_PALETTE: ColorScale = color_scale![
    0xfff0f6, 0xffd6e7, 0xffadd2, 0xff85c0, 0xf759ab, 0xeb2f96, 0xc41d7f, 0x9e1068, 0x780650,
    0x520339,
];

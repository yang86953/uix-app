use std::fmt;
use std::time::Duration;

use crate::native::graphics::vulkan::platform::adapter::VulkanAdapterInfo;

const GFX_R5_EXPECT_VENDOR_ENV: &str = "UIX_GFX_R5_EXPECT_VENDOR";
const VULKAN_EXPECT_DEVICE_FAULT_ENV: &str = "UIX_VULKAN_EXPECT_DEVICE_FAULT";
const VULKAN_DEVICE_LOST_TIMEOUT_ENV: &str = "UIX_VULKAN_DEVICE_LOST_TIMEOUT_SECONDS";
const DEFAULT_DEVICE_LOST_TIMEOUT_SECONDS: u64 = 60;
const MIN_DEVICE_LOST_TIMEOUT_SECONDS: u64 = 5;
const MAX_DEVICE_LOST_TIMEOUT_SECONDS: u64 = 600;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ExpectedVulkanVendor {
    Nvidia,
    Amd,
    Intel,
}

impl ExpectedVulkanVendor {
    pub(crate) const fn vendor_id(self) -> u32 {
        match self {
            Self::Nvidia => 0x10DE,
            Self::Amd => 0x1002,
            Self::Intel => 0x8086,
        }
    }

    pub(crate) const fn label(self) -> &'static str {
        match self {
            Self::Nvidia => "nvidia",
            Self::Amd => "amd",
            Self::Intel => "intel",
        }
    }

    pub(crate) fn assert_adapter(self, adapter: &VulkanAdapterInfo) {
        assert_eq!(
            adapter.vendor_id,
            self.vendor_id(),
            "expected {} ({:#06X}), actual {}",
            self.label(),
            self.vendor_id(),
            adapter.diagnostic_summary()
        );
    }

    fn parse(value: &str) -> Result<Self, GfxR5ConfigError> {
        match value.trim().to_ascii_lowercase().as_str() {
            "nvidia" => Ok(Self::Nvidia),
            "amd" => Ok(Self::Amd),
            "intel" => Ok(Self::Intel),
            _ => Err(GfxR5ConfigError::Invalid {
                name: GFX_R5_EXPECT_VENDOR_ENV,
                value: value.to_owned(),
                expected: "nvidia, amd, or intel",
            }),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum GfxR5ConfigError {
    Missing {
        name: &'static str,
        expected: &'static str,
    },
    Invalid {
        name: &'static str,
        value: String,
        expected: &'static str,
    },
    OutOfRange {
        name: &'static str,
        value: u64,
        min: u64,
        max: u64,
    },
}

impl fmt::Display for GfxR5ConfigError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Missing { name, expected } => {
                write!(formatter, "{name} is required; expected {expected}")
            }
            Self::Invalid {
                name,
                value,
                expected,
            } => write!(formatter, "invalid {name}={value:?}; expected {expected}"),
            Self::OutOfRange {
                name,
                value,
                min,
                max,
            } => write!(formatter, "invalid {name}={value}; expected {min}..={max}"),
        }
    }
}

pub(crate) fn expected_gfx_r5_vendor() -> Result<ExpectedVulkanVendor, GfxR5ConfigError> {
    let value = required_env(GFX_R5_EXPECT_VENDOR_ENV, "nvidia, amd, or intel")?;
    ExpectedVulkanVendor::parse(&value)
}

pub(crate) fn expected_device_fault_capability() -> Result<bool, GfxR5ConfigError> {
    let value = required_env(VULKAN_EXPECT_DEVICE_FAULT_ENV, "true, false, 1, or 0")?;
    parse_expected_bool(&value)
}

pub(crate) fn requested_external_device_loss_timeout() -> Result<Duration, GfxR5ConfigError> {
    let value = match std::env::var(VULKAN_DEVICE_LOST_TIMEOUT_ENV) {
        Ok(value) => Some(value),
        Err(std::env::VarError::NotPresent) => None,
        Err(std::env::VarError::NotUnicode(value)) => {
            return Err(GfxR5ConfigError::Invalid {
                name: VULKAN_DEVICE_LOST_TIMEOUT_ENV,
                value: value.to_string_lossy().into_owned(),
                expected: "an integer number of seconds from 5 through 600",
            });
        }
    };
    parse_device_loss_timeout(value.as_deref())
}

fn required_env(name: &'static str, expected: &'static str) -> Result<String, GfxR5ConfigError> {
    match std::env::var(name) {
        Ok(value) => Ok(value),
        Err(std::env::VarError::NotPresent) => Err(GfxR5ConfigError::Missing { name, expected }),
        Err(std::env::VarError::NotUnicode(value)) => Err(GfxR5ConfigError::Invalid {
            name,
            value: value.to_string_lossy().into_owned(),
            expected,
        }),
    }
}

fn parse_expected_bool(value: &str) -> Result<bool, GfxR5ConfigError> {
    match value.trim().to_ascii_lowercase().as_str() {
        "true" | "1" => Ok(true),
        "false" | "0" => Ok(false),
        _ => Err(GfxR5ConfigError::Invalid {
            name: VULKAN_EXPECT_DEVICE_FAULT_ENV,
            value: value.to_owned(),
            expected: "true, false, 1, or 0",
        }),
    }
}

fn parse_device_loss_timeout(value: Option<&str>) -> Result<Duration, GfxR5ConfigError> {
    let Some(value) = value else {
        return Ok(Duration::from_secs(DEFAULT_DEVICE_LOST_TIMEOUT_SECONDS));
    };
    let seconds = value
        .trim()
        .parse::<u64>()
        .map_err(|_| GfxR5ConfigError::Invalid {
            name: VULKAN_DEVICE_LOST_TIMEOUT_ENV,
            value: value.to_owned(),
            expected: "an integer number of seconds from 5 through 600",
        })?;
    if !(MIN_DEVICE_LOST_TIMEOUT_SECONDS..=MAX_DEVICE_LOST_TIMEOUT_SECONDS).contains(&seconds) {
        return Err(GfxR5ConfigError::OutOfRange {
            name: VULKAN_DEVICE_LOST_TIMEOUT_ENV,
            value: seconds,
            min: MIN_DEVICE_LOST_TIMEOUT_SECONDS,
            max: MAX_DEVICE_LOST_TIMEOUT_SECONDS,
        });
    }
    Ok(Duration::from_secs(seconds))
}

#[test]
fn gfx_r5_vendor_expectation_accepts_named_matrix_vendors() {
    assert_eq!(
        ExpectedVulkanVendor::parse("NVIDIA"),
        Ok(ExpectedVulkanVendor::Nvidia)
    );
    assert_eq!(
        ExpectedVulkanVendor::parse(" amd "),
        Ok(ExpectedVulkanVendor::Amd)
    );
    assert_eq!(
        ExpectedVulkanVendor::parse("intel"),
        Ok(ExpectedVulkanVendor::Intel)
    );
    assert_eq!(ExpectedVulkanVendor::Nvidia.vendor_id(), 0x10DE);
    assert_eq!(ExpectedVulkanVendor::Amd.vendor_id(), 0x1002);
    assert_eq!(ExpectedVulkanVendor::Intel.vendor_id(), 0x8086);
}

#[test]
fn gfx_r5_vendor_expectation_rejects_implicit_or_unknown_vendors() {
    assert_eq!(
        ExpectedVulkanVendor::parse(""),
        Err(GfxR5ConfigError::Invalid {
            name: GFX_R5_EXPECT_VENDOR_ENV,
            value: String::new(),
            expected: "nvidia, amd, or intel",
        })
    );
    assert_eq!(
        ExpectedVulkanVendor::parse("virtual"),
        Err(GfxR5ConfigError::Invalid {
            name: GFX_R5_EXPECT_VENDOR_ENV,
            value: "virtual".to_owned(),
            expected: "nvidia, amd, or intel",
        })
    );
}

#[test]
fn gfx_r5_device_fault_expectation_is_explicit() {
    assert_eq!(parse_expected_bool(" TRUE "), Ok(true));
    assert_eq!(parse_expected_bool("1"), Ok(true));
    assert_eq!(parse_expected_bool("false"), Ok(false));
    assert_eq!(parse_expected_bool("0"), Ok(false));
    assert!(matches!(
        parse_expected_bool("enabled"),
        Err(GfxR5ConfigError::Invalid {
            name: VULKAN_EXPECT_DEVICE_FAULT_ENV,
            ..
        })
    ));
}

#[test]
fn gfx_r5_device_loss_timeout_rejects_invalid_or_clamped_evidence() {
    assert_eq!(
        parse_device_loss_timeout(None),
        Ok(Duration::from_secs(DEFAULT_DEVICE_LOST_TIMEOUT_SECONDS))
    );
    assert_eq!(
        parse_device_loss_timeout(Some(" 5 ")),
        Ok(Duration::from_secs(5))
    );
    assert_eq!(
        parse_device_loss_timeout(Some("600")),
        Ok(Duration::from_secs(600))
    );
    assert!(matches!(
        parse_device_loss_timeout(Some("fast")),
        Err(GfxR5ConfigError::Invalid {
            name: VULKAN_DEVICE_LOST_TIMEOUT_ENV,
            ..
        })
    ));
    assert!(matches!(
        parse_device_loss_timeout(Some("4")),
        Err(GfxR5ConfigError::OutOfRange { value: 4, .. })
    ));
    assert!(matches!(
        parse_device_loss_timeout(Some("601")),
        Err(GfxR5ConfigError::OutOfRange { value: 601, .. })
    ));
}

use std::fmt;

const GFX_R5_EXPECT_VENDOR_ENV: &str = "UIX_GFX_R5_EXPECT_VENDOR";

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

    fn parse(value: &str) -> Result<Self, GfxR5VendorExpectationError> {
        match value.trim().to_ascii_lowercase().as_str() {
            "nvidia" => Ok(Self::Nvidia),
            "amd" => Ok(Self::Amd),
            "intel" => Ok(Self::Intel),
            _ => Err(GfxR5VendorExpectationError::Unsupported(value.to_owned())),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum GfxR5VendorExpectationError {
    Missing,
    Unsupported(String),
}

impl fmt::Display for GfxR5VendorExpectationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Missing => write!(
                formatter,
                "{GFX_R5_EXPECT_VENDOR_ENV} must be nvidia, amd, or intel"
            ),
            Self::Unsupported(value) => write!(
                formatter,
                "unsupported {GFX_R5_EXPECT_VENDOR_ENV}={value:?}; expected nvidia, amd, or intel"
            ),
        }
    }
}

pub(crate) fn expected_gfx_r5_vendor() -> Result<ExpectedVulkanVendor, GfxR5VendorExpectationError>
{
    let value = std::env::var(GFX_R5_EXPECT_VENDOR_ENV)
        .map_err(|_| GfxR5VendorExpectationError::Missing)?;
    ExpectedVulkanVendor::parse(&value)
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
        Err(GfxR5VendorExpectationError::Unsupported(String::new()))
    );
    assert_eq!(
        ExpectedVulkanVendor::parse("virtual"),
        Err(GfxR5VendorExpectationError::Unsupported(
            "virtual".to_owned()
        ))
    );
}

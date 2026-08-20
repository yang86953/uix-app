    // 引入当前私有分类 Component。
    use super::classify_file_drop_enable;
    // 构造 typed 结果所需的错误码与错误值。
    use crate::core::{Errc, Error};

    // 成功结果必须报告真实启用。
    #[test]
    fn reports_enabled_capability() {
        // Ok 只能映射为 true。
        assert_eq!(classify_file_drop_enable(Ok(())).unwrap(), true);
    }

    // 稳定 NotImplemented 不得阻止应用窗口创建。
    #[test]
    fn tolerates_only_not_implemented() {
        // 构造平台稳定能力缺失。
        let unsupported = Error::new(Errc::NotImplemented, "file drop unavailable");
        // 能力缺失映射为 false 而非错误。
        assert_eq!(
            // 调用纯分类 Component。
            classify_file_drop_enable(Err(unsupported)).unwrap(),
            // false 表示窗口可用但 FileDrop 未启用。
            false
        );
    }

    // 真实执行失败必须保留原始错误码。
    #[test]
    fn propagates_real_enable_failure() {
        // 构造不可吞掉的状态错误。
        let failure = Error::new(Errc::InvalidState, "file drop state poisoned");
        // 分类器必须返回错误。
        let returned = classify_file_drop_enable(Err(failure)).unwrap_err();
        // typed 错误码不得被重写。
        assert_eq!(returned.code(), Errc::InvalidState);
    }

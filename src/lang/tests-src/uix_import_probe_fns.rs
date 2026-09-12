// 各项自带 cfg(test)，在源模块作用域 include! 展开。

impl SourceStageCache {

    // 返回真正执行单文件解析的次数，供增量缓存契约测试观测。
    #[cfg(test)]
    pub(crate) const fn parse_runs(&self) -> usize {
        self.parse_runs
    }
}

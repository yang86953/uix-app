//! Module 的普通方法复用框架数据执行内核。
use crate::lang::runtime::*;

impl Frame<'_, '_> {
    pub fn method(&mut self, value: Value, name: &str, args: Vec<Value>) -> RuntimeResult<Value> {
        super::data_methods::method_data(value, name, args, self.value_limits())
    }
}

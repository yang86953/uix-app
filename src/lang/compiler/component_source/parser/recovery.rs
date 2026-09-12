//! 编辑器有界结构恢复。只消费原文，不追加源码，不将恢复结果交给生成入口。
use super::*;

impl Parser<'_> {
    pub(super) fn recover(&mut self, error: Box<CompilerDiagnostic>) -> Result<()> {
        let Some(diagnostics) = &mut self.recovery else {
            return Err(error);
        };
        if diagnostics.len() >= 64 {
            return Err(self.error("component-recovery-limit", "单文件编辑恢复诊断超过 64 项"));
        }
        diagnostics.push(*error);
        Ok(())
    }
    /// 循环只在实际闭合符或编辑模式 EOF 停止；不把虚构 token 当作已消费。
    pub(super) fn at_end(&mut self, token: &str) -> Result<bool> {
        Ok(self.at(token)? || (self.recovery.is_some() && self.eof()?))
    }
    pub(super) fn missing_token(&mut self, token: &str) -> Result<()> {
        let error = self.error("component-token", format!("此处需要 {token}"));
        let next = self.source[self.pos..].chars().next();
        let synchronized = matches!(
            (token, next),
            (";", Some('}')) | (")" | "]", Some(';' | ',' | '}')) | (")", Some('{'))
        );
        if self.recovery.is_some() && (self.eof()? || synchronized) {
            self.recover(error)
        } else {
            Err(error)
        }
    }
    pub(super) fn hole_boundary(&mut self) -> Result<bool> {
        if self.recovery.is_none() {
            return Ok(false);
        }
        self.trivia()?;
        Ok(self.pos == self.source.len()
            || self.source[self.pos..].starts_with([';', ')', ']', '}', ',']))
    }
    pub(super) fn missing_expression(&mut self) -> Result<Expr> {
        let start = self.start()?;
        self.node()?;
        self.recover(self.error("component-expression", "此处缺失表达式"))?;
        Ok(Expr {
            kind: ExprKind::Missing,
            span: self.span(start),
        })
    }
    pub(super) fn missing_type(&mut self) -> Result<TypeNode> {
        let start = self.start()?;
        self.recover(self.error("component-type", "此处缺失类型"))?;
        Ok(TypeNode {
            kind: TypeKind::Missing,
            span: self.span(start),
        })
    }
    /// 只用于已经进入的成员或标签名称位置，不制造声明/参数绑定。
    pub(super) fn edit_name(&mut self) -> Result<Name> {
        let start = self.start()?;
        if self.hole_boundary()? || (self.recovery.is_some() && self.at(">")?) {
            self.recover(self.error("component-name", "此处缺失标识符"))?;
            return Ok(Name {
                text: String::new(),
                span: self.span(start),
            });
        }
        self.name()
    }
    pub(super) fn edit_path(&mut self) -> Result<Vec<Name>> {
        let mut path = vec![self.edit_name()?];
        while self.eat(".")? {
            path.push(self.edit_name()?);
        }
        Ok(path)
    }
    pub(super) fn recover_closing_tag(&mut self) -> Result<bool> {
        if self.recovery.is_some() && self.pos == self.source.len() {
            self.recover(self.error("component-closing-tag", "标签缺少结束标签"))?;
            Ok(true)
        } else {
            Ok(false)
        }
    }
}

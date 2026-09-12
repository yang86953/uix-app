// 引入父解析器与共享跨度合并函数。
use super::{ExpressionParser, merge_span};
// 引入对象解析所需的表达式 AST、标记与诊断。
use crate::lang::compiler::uix_lang::{
    Diagnostic, Expression, ExpressionKind, ExpressionToken, ExpressionTokenKind, ObjectField,
    SourceSpan,
};

// 为受限表达式解析器实现对象字面量边界。
impl ExpressionParser {
    // 解析只含标识符键和受限表达式值的对象字面量。
    pub(super) fn parse_object(
        &mut self,
        // 接收左花括号标记。
        open: ExpressionToken,
    ) -> Result<Expression, Diagnostic> {
        // 按源码顺序保存字段，供结构属性生成器确定性消费。
        let mut fields = Vec::new();
        // 空对象直接闭合。
        if self.take(&ExpressionTokenKind::RightBrace) {
            // 保存闭合花括号标记。
            let close = self.previous().clone();
            // 返回空对象节点。
            return Ok(Expression {
                // 保存空字段序列。
                kind: ExpressionKind::Object(fields),
                // 保存完整对象跨度。
                span: merge_span(open.span, close.span),
            });
        }
        // 解析逗号分隔的确定字段。
        loop {
            // 对象键只能是普通标识符。
            let key = self.take_identifier(
                "对象字段键必须是标识符",
                "使用如 { count: 7, dot: false } 的确定字段",
            )?;
            // 提取已验证键文本。
            let name = match &key.kind {
                // 复制标识符文本。
                ExpressionTokenKind::Identifier(value) => value.clone(),
                // 标识符入口保证其他分支不可达。
                _ => unreachable!(),
            };
            // 同一对象不能重复声明字段。
            if fields
                // 遍历已经保存的字段。
                .iter()
                // 比较字段名称。
                .any(|field: &ObjectField| field.name == name)
            {
                // 返回重复字段诊断。
                return Err(Diagnostic::new(
                    // 精确指向重复键。
                    key.span,
                    // 陈述重复字段名称。
                    format!("对象字段 {name} 重复声明"),
                    // 给出去重动作。
                    "每个对象字段只保留一次",
                ));
            }
            // 字段键后必须包含冒号。
            self.expect(
                &ExpressionTokenKind::Colon,
                "对象字段缺少 :",
                "在字段名和值之间添加冒号",
            )?;
            // 字段值复用现有受限表达式语法。
            let value = self.parse_ternary()?;
            // 保存字段完整跨度。
            let field_span = merge_span(key.span, value.span);
            // 追加当前字段。
            fields.push(ObjectField {
                // 保存字段名。
                name,
                // 保存字段值。
                value,
                // 保存字段跨度。
                span: field_span,
            });
            // 右花括号结束对象。
            if self.take(&ExpressionTokenKind::RightBrace) {
                // 保存闭合花括号标记。
                let close = self.previous().clone();
                // 返回完整对象节点。
                return Ok(Expression {
                    // 保存有序字段。
                    kind: ExpressionKind::Object(fields),
                    // 保存完整对象跨度。
                    span: merge_span(open.span, close.span),
                });
            }
            // 输入结束表示对象缺少右花括号。
            if self.check(&ExpressionTokenKind::End) {
                // 返回未闭合对象诊断。
                return Err(missing_close(self.current().span));
            }
            // 字段之间必须使用逗号。
            self.expect(
                &ExpressionTokenKind::Comma,
                "对象字段之间缺少逗号",
                "在相邻字段之间添加逗号",
            )?;
            // 当前语法不接受尾随逗号。
            if self.check(&ExpressionTokenKind::RightBrace) {
                // 返回尾随逗号诊断。
                return Err(Diagnostic::new(
                    // 指向右花括号。
                    self.current().span,
                    // 陈述尾随逗号限制。
                    "对象字面量不允许尾随逗号",
                    // 给出删除动作。
                    "删除最后一个逗号",
                ));
            }
            // 逗号后结束同样表示对象缺少右花括号。
            if self.check(&ExpressionTokenKind::End) {
                // 返回未闭合对象诊断。
                return Err(missing_close(self.current().span));
            }
        }
    }
}
// 构造缺少对象右花括号的统一诊断。
fn missing_close(span: SourceSpan) -> Diagnostic {
    // 返回未闭合对象诊断。
    Diagnostic::new(
        // 指向输入末尾。
        span,
        // 陈述闭合符缺失。
        "对象字面量缺少 }",
        // 给出补全动作。
        "在对象字段末尾添加 }",
    )
}

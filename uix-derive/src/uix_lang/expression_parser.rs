// 把对象字面量解析隔离到解析器子模块。
mod object;
// 把调用形状与闭包位置验证隔离到解析器子模块。
mod call_validation;

// 引入调用与闭包位置验证入口。
use call_validation::{validate_call, validate_closure_positions};

// 引入表达式 AST、词法标记、诊断和跨度。
use super::{
    BinaryOperator, CallArgument, Diagnostic, Expression, ExpressionKind, ExpressionToken,
    ExpressionTokenKind, SourceSpan, UnaryOperator, lex_expression,
};

// 解析并验证一段受限表达式源码。
pub(crate) fn parse_expression(
    // 接收不含外围定界符的源码。
    source: &str,
    // 接收源码首字符的绝对位置。
    origin: SourceSpan,
) -> Result<Expression, Diagnostic> {
    // 先建立带跨度标记流。
    let tokens = lex_expression(source, origin)?;
    // 创建优先级解析器。
    let mut parser = ExpressionParser::new(tokens);
    // 解析完整表达式。
    parser.parse()
}

// 提供确定性递归下降表达式解析器。
struct ExpressionParser {
    // 保存包含结束哨兵的标记流。
    tokens: Vec<ExpressionToken>,
    // 保存当前标记索引。
    index: usize,
}

// 实现受限表达式优先级和调用规则。
impl ExpressionParser {
    // 从标记流起点创建解析器。
    fn new(tokens: Vec<ExpressionToken>) -> Self {
        // 初始化零索引。
        Self { tokens, index: 0 }
    }

    // 解析完整输入并拒绝尾随结构。
    fn parse(&mut self) -> Result<Expression, Diagnostic> {
        // 空输入缺少表达式。
        if self.check(&ExpressionTokenKind::End) {
            // 返回空表达式诊断。
            return Err(Diagnostic::new(
                // 指向输入末尾。
                self.current().span,
                // 陈述失败原因。
                "表达式不能为空",
                // 给出修复建议。
                "填写文档允许的受限表达式",
            ));
        }
        // 从最低优先级三元表达式开始。
        let expression = self.parse_ternary()?;
        // 输入必须完全消费。
        if !self.check(&ExpressionTokenKind::End) {
            // 返回针对尾随标记的专用诊断。
            return Err(self.trailing_error());
        }
        // 验证受限闭包只出现在开放的数组操作参数位置。
        validate_closure_positions(&expression)?;
        // 返回已验证表达式。
        Ok(expression)
    }

    // 解析右结合三元条件表达式。
    fn parse_ternary(&mut self) -> Result<Expression, Diagnostic> {
        // 先解析逻辑或条件。
        let condition = self.parse_or()?;
        // 没有问号时直接返回条件。
        if !self.take(&ExpressionTokenKind::Question) {
            // 返回非三元表达式。
            return Ok(condition);
        }
        // 解析真分支并允许嵌套三元。
        let then_branch = self.parse_ternary()?;
        // 三元必须包含冒号。
        self.expect(
            &ExpressionTokenKind::Colon,
            "三元表达式缺少 :",
            "使用 condition ? trueValue : falseValue",
        )?;
        // 解析假分支并保持右结合。
        let else_branch = self.parse_ternary()?;
        // 合并完整三元跨度。
        let span = merge_span(condition.span, else_branch.span);
        // 返回三元 AST。
        Ok(Expression {
            // 保存三元结构。
            kind: ExpressionKind::Ternary {
                // 保存条件。
                condition: Box::new(condition),
                // 保存真分支。
                then_branch: Box::new(then_branch),
                // 保存假分支。
                else_branch: Box::new(else_branch),
            },
            // 保存完整跨度。
            span,
        })
    }

    // 解析逻辑或层。
    fn parse_or(&mut self) -> Result<Expression, Diagnostic> {
        // 使用逻辑与作为更高优先级操作数。
        self.parse_binary(
            // 传入下一优先级解析器。
            Self::parse_and,
            // 声明本层运算符。
            &[(ExpressionTokenKind::OrOr, BinaryOperator::Or)],
        )
    }

    // 解析逻辑与层。
    fn parse_and(&mut self) -> Result<Expression, Diagnostic> {
        // 使用相等比较作为更高优先级操作数。
        self.parse_binary(
            // 传入下一优先级解析器。
            Self::parse_equality,
            // 声明本层运算符。
            &[(ExpressionTokenKind::AndAnd, BinaryOperator::And)],
        )
    }

    // 解析相等与不等层。
    fn parse_equality(&mut self) -> Result<Expression, Diagnostic> {
        // 使用顺序比较作为更高优先级操作数。
        self.parse_binary(
            // 传入下一优先级解析器。
            Self::parse_comparison,
            // 声明相等类运算符。
            &[
                // 映射相等比较。
                (ExpressionTokenKind::EqualEqual, BinaryOperator::Equal),
                // 映射不等比较。
                (ExpressionTokenKind::BangEqual, BinaryOperator::NotEqual),
            ],
        )
    }

    // 解析大小比较层。
    fn parse_comparison(&mut self) -> Result<Expression, Diagnostic> {
        // 使用加减层作为更高优先级操作数。
        self.parse_binary(
            // 传入下一优先级解析器。
            Self::parse_additive,
            // 声明顺序比较运算符。
            &[
                // 映射小于。
                (ExpressionTokenKind::Less, BinaryOperator::Less),
                // 映射小于等于。
                (ExpressionTokenKind::LessEqual, BinaryOperator::LessEqual),
                // 映射大于。
                (ExpressionTokenKind::Greater, BinaryOperator::Greater),
                // 映射大于等于。
                (
                    ExpressionTokenKind::GreaterEqual,
                    BinaryOperator::GreaterEqual,
                ),
            ],
        )
    }

    // 解析加减层。
    fn parse_additive(&mut self) -> Result<Expression, Diagnostic> {
        // 使用乘除层作为更高优先级操作数。
        self.parse_binary(
            // 传入下一优先级解析器。
            Self::parse_multiplicative,
            // 声明加减运算符。
            &[
                // 映射加法。
                (ExpressionTokenKind::Plus, BinaryOperator::Add),
                // 映射减法。
                (ExpressionTokenKind::Minus, BinaryOperator::Subtract),
            ],
        )
    }

    // 解析乘除取余层。
    fn parse_multiplicative(&mut self) -> Result<Expression, Diagnostic> {
        // 使用一元层作为更高优先级操作数。
        self.parse_binary(
            // 传入下一优先级解析器。
            Self::parse_unary,
            // 声明乘除取余运算符。
            &[
                // 映射乘法。
                (ExpressionTokenKind::Star, BinaryOperator::Multiply),
                // 映射除法。
                (ExpressionTokenKind::Slash, BinaryOperator::Divide),
                // 映射取余。
                (ExpressionTokenKind::Percent, BinaryOperator::Remainder),
            ],
        )
    }

    // 解析任一左结合二元优先级层。
    fn parse_binary(
        &mut self,
        // 接收更高优先级操作数解析器。
        operand: fn(&mut Self) -> Result<Expression, Diagnostic>,
        // 接收本层标记到 AST 运算符映射。
        operators: &[(ExpressionTokenKind, BinaryOperator)],
    ) -> Result<Expression, Diagnostic> {
        // 解析首个左操作数。
        let mut left = operand(self)?;
        // 连续折叠同优先级运算符。
        loop {
            // 查找当前标记对应运算符。
            let Some((_, operator)) = operators
                // 遍历本层运算符。
                .iter()
                // 按变体匹配当前标记。
                .find(|(kind, _)| same_kind(kind, &self.current().kind))
            else {
                // 没有本层运算符时完成折叠。
                break;
            };
            // 保存具体 AST 运算符。
            let operator = *operator;
            // 消费运算符标记。
            self.advance();
            // 解析右操作数。
            let right = operand(self)?;
            // 合并左右跨度。
            let span = merge_span(left.span, right.span);
            // 构造新的左结合节点。
            left = Expression {
                // 保存二元结构。
                kind: ExpressionKind::Binary {
                    // 保存此前左树。
                    left: Box::new(left),
                    // 保存本层运算符。
                    operator,
                    // 保存新右操作数。
                    right: Box::new(right),
                },
                // 保存组合跨度。
                span,
            };
        }
        // 返回本层结果。
        Ok(left)
    }

    // 解析逻辑非或数值取负。
    fn parse_unary(&mut self) -> Result<Expression, Diagnostic> {
        // 映射可用一元运算符。
        let operator = if self.take(&ExpressionTokenKind::Bang) {
            // 选择逻辑非。
            Some(UnaryOperator::Not)
        } else if self.take(&ExpressionTokenKind::Minus) {
            // 选择数值取负。
            Some(UnaryOperator::Negate)
        } else {
            // 当前不是一元运算符。
            None
        };
        // 有一元运算符时递归解析操作数。
        if let Some(operator) = operator {
            // 运算符是刚刚消费的标记。
            let operator_span = self.previous().span;
            // 递归解析右侧一元表达式。
            let operand = self.parse_unary()?;
            // 合并运算符和操作数跨度。
            let span = merge_span(operator_span, operand.span);
            // 返回一元 AST。
            return Ok(Expression {
                // 保存一元结构。
                kind: ExpressionKind::Unary {
                    // 保存运算符。
                    operator,
                    // 保存操作数。
                    operand: Box::new(operand),
                },
                // 保存完整跨度。
                span,
            });
        }
        // 无一元前缀时解析后缀链。
        self.parse_postfix()
    }

    // 解析成员、索引与调用后缀链。
    fn parse_postfix(&mut self) -> Result<Expression, Diagnostic> {
        // 解析基础原子。
        let mut expression = self.parse_primary()?;
        // 连续消费后缀。
        loop {
            // 点号开始成员访问。
            if self.take(&ExpressionTokenKind::Dot) {
                // 要求成员标识符。
                let member = self
                    .take_identifier("成员访问缺少名称", "在点号后填写成员名，例如 state.value")?;
                // 保存成员名称。
                let member_name = match &member.kind {
                    // 提取标识符文本。
                    ExpressionTokenKind::Identifier(value) => value.clone(),
                    // 入口保证该分支不可达。
                    _ => unreachable!(),
                };
                // 合并对象与成员跨度。
                let span = merge_span(expression.span, member.span);
                // 构造成员访问节点。
                expression = Expression {
                    // 保存成员结构。
                    kind: ExpressionKind::Member {
                        // 保存对象。
                        object: Box::new(expression),
                        // 保存成员名。
                        member: member_name,
                    },
                    // 保存完整跨度。
                    span,
                };
                // 继续解析后续链。
                continue;
            }
            // 左方括号开始索引访问。
            if self.take(&ExpressionTokenKind::LeftBracket) {
                // 解析下标表达式。
                let index = self.parse_ternary()?;
                // 要求右方括号。
                let close = self.expect(
                    &ExpressionTokenKind::RightBracket,
                    "索引表达式缺少 ]",
                    "在下标表达式末尾添加 ]",
                )?;
                // 合并对象与闭合括号跨度。
                let span = merge_span(expression.span, close.span);
                // 构造索引节点。
                expression = Expression {
                    // 保存索引结构。
                    kind: ExpressionKind::Index {
                        // 保存对象。
                        object: Box::new(expression),
                        // 保存下标。
                        index: Box::new(index),
                    },
                    // 保存完整跨度。
                    span,
                };
                // 继续解析后续链。
                continue;
            }
            // 左圆括号开始调用。
            if self.take(&ExpressionTokenKind::LeftParen) {
                // 保存调用开始前的目标跨度。
                let callee_span = expression.span;
                // 解析调用参数和闭合括号。
                let (arguments, close) = self.parse_arguments()?;
                // 验证调用目标与内置操作约束。
                validate_call(&expression, &arguments, close.span)?;
                // 合并调用目标与闭合括号跨度。
                let span = merge_span(callee_span, close.span);
                // 构造调用节点。
                expression = Expression {
                    // 保存调用结构。
                    kind: ExpressionKind::Call {
                        // 保存调用目标。
                        callee: Box::new(expression),
                        // 保存参数。
                        arguments,
                    },
                    // 保存完整跨度。
                    span,
                };
                // 继续解析后续链。
                continue;
            }
            // 没有后缀时完成表达式。
            break;
        }
        // 返回后缀表达式。
        Ok(expression)
    }

    // 解析调用参数列表。
    fn parse_arguments(&mut self) -> Result<(Vec<CallArgument>, ExpressionToken), Diagnostic> {
        // 保存参数顺序。
        let mut arguments = Vec::new();
        // 空参数列表直接结束。
        if self.check(&ExpressionTokenKind::RightParen) {
            // 消费右圆括号。
            let close = self.advance().clone();
            // 返回空列表。
            return Ok((arguments, close));
        }
        // 解析逗号分隔参数。
        loop {
            // 保存参数起点。
            let start = self.current().span;
            // 识别标识符加冒号的命名参数。
            let name = if matches!(self.current().kind, ExpressionTokenKind::Identifier(_))
                // 检查下一标记为冒号。
                && self.check_next(&ExpressionTokenKind::Colon)
            {
                // 消费参数名。
                let token = self.advance().clone();
                // 消费冒号。
                self.advance();
                // 提取参数名称。
                match token.kind {
                    // 返回标识符文本。
                    ExpressionTokenKind::Identifier(value) => Some(value),
                    // 入口保证该分支不可达。
                    _ => unreachable!(),
                }
            } else {
                // 普通参数没有名称。
                None
            };
            // 解析参数值。
            let value = self.parse_ternary()?;
            // 合并参数跨度。
            let span = merge_span(start, value.span);
            // 保存参数。
            arguments.push(CallArgument { name, value, span });
            // 没有逗号时结束参数读取。
            if !self.take(&ExpressionTokenKind::Comma) {
                // 退出参数循环。
                break;
            }
            // 尾随逗号不属于规范调用语法。
            if self.check(&ExpressionTokenKind::RightParen) {
                // 返回尾随逗号诊断。
                return Err(Diagnostic::new(
                    // 指向右圆括号。
                    self.current().span,
                    // 陈述失败原因。
                    "调用参数不允许尾随逗号",
                    // 给出修复建议。
                    "删除最后一个逗号",
                ));
            }
        }
        // 要求闭合右圆括号。
        let close = self.expect(
            &ExpressionTokenKind::RightParen,
            "调用缺少 )",
            "在参数列表末尾添加 )",
        )?;
        // 返回参数与闭合标记。
        Ok((arguments, close))
    }

    // 解析位于原子位置的方括号数组字面量。
    fn parse_array(&mut self, open: ExpressionToken) -> Result<Expression, Diagnostic> {
        // 保存按源码顺序解析的元素。
        let mut items = Vec::new();
        // 空数组直接闭合。
        if self.take(&ExpressionTokenKind::RightBracket) {
            // 返回空数组节点。
            return Ok(Expression {
                // 保存空元素集合。
                kind: ExpressionKind::Array(Vec::new()),
                // 覆盖完整方括号跨度。
                span: merge_span(open.span, self.previous().span),
            });
        }
        // 解析逗号分隔的元素。
        loop {
            // 每个元素都是完整受限表达式。
            let item = self.parse_ternary()?;
            // 保存当前元素。
            items.push(item);
            // 没有逗号时结束元素读取。
            if !self.take(&ExpressionTokenKind::Comma) {
                // 退出元素循环。
                break;
            }
            // 尾随逗号不属于规范数组语法。
            if self.check(&ExpressionTokenKind::RightBracket) {
                // 返回尾随逗号诊断。
                return Err(Diagnostic::new(
                    // 指向右方括号。
                    self.current().span,
                    // 陈述失败原因。
                    "数组字面量不允许尾随逗号",
                    // 给出修复建议。
                    "删除最后一个逗号",
                ));
            }
        }
        // 要求闭合右方括号。
        let close = self.expect(
            &ExpressionTokenKind::RightBracket,
            "数组字面量缺少 ]",
            "在数组元素末尾添加 ]",
        )?;
        // 返回完整数组节点。
        Ok(Expression {
            // 保存有序元素。
            kind: ExpressionKind::Array(items),
            // 覆盖完整方括号跨度。
            span: merge_span(open.span, close.span),
        })
    }

    // 解析原子表达式。
    fn parse_primary(&mut self) -> Result<Expression, Diagnostic> {
        // 消费当前原子标记。
        let token = self.advance().clone();
        // 按标记构造 AST 或专用诊断。
        match token.kind {
            // 普通标识符映射引用。
            ExpressionTokenKind::Identifier(value)
                if value != "match" && (!value.starts_with('$') || value == "$event") =>
            {
                // 返回普通或保留事件标识符。
                Ok(Expression {
                    // 保存标识符。
                    kind: ExpressionKind::Identifier(value),
                    // 保存标记跨度。
                    span: token.span,
                })
            }
            // 只有保留事件参数允许美元符号前缀。
            ExpressionTokenKind::Identifier(value) if value.starts_with('$') => {
                // 返回保留标识符诊断。
                Err(Diagnostic::new(
                    // 指向非法美元标识符。
                    token.span,
                    // 陈述失败原因。
                    format!("表达式只允许保留事件参数 $event，不能使用 {value}"),
                    // 给出修复建议。
                    "使用 $event 或移除美元符号前缀",
                ))
            }
            // match 明确不属于界面表达式。
            ExpressionTokenKind::Identifier(_) => Err(Diagnostic::new(
                // 指向 match。
                token.span,
                // 陈述失败原因。
                "表达式不支持 match",
                // 给出修复建议。
                "把复杂分支逻辑移到 Rust 侧，或使用三元表达式",
            )),
            // 数字标记映射数字节点。
            ExpressionTokenKind::Number(value) => Ok(Expression {
                // 保存数字源码。
                kind: ExpressionKind::Number(value),
                // 保存标记跨度。
                span: token.span,
            }),
            // 字符串标记映射字符串节点。
            ExpressionTokenKind::String(value) => Ok(Expression {
                // 保存解码字符串。
                kind: ExpressionKind::String(value),
                // 保存标记跨度。
                span: token.span,
            }),
            // true 映射布尔真。
            ExpressionTokenKind::True => Ok(Expression {
                // 保存布尔真。
                kind: ExpressionKind::Boolean(true),
                // 保存标记跨度。
                span: token.span,
            }),
            // false 映射布尔假。
            ExpressionTokenKind::False => Ok(Expression {
                // 保存布尔假。
                kind: ExpressionKind::Boolean(false),
                // 保存标记跨度。
                span: token.span,
            }),
            // 左圆括号开始分组。
            ExpressionTokenKind::LeftParen => {
                // 解析括号内部表达式。
                let mut expression = self.parse_ternary()?;
                // 要求右圆括号。
                let close = self.expect(
                    &ExpressionTokenKind::RightParen,
                    "分组表达式缺少 )",
                    "在分组表达式末尾添加 )",
                )?;
                // 把分组括号计入跨度。
                expression.span = merge_span(token.span, close.span);
                // 返回内部确定性结构。
                Ok(expression)
            }
            // 左方括号位于原子位置表示数组字面量。
            ExpressionTokenKind::LeftBracket => self.parse_array(token),
            // 左花括号位于原子位置表示受限对象字面量。
            ExpressionTokenKind::LeftBrace => self.parse_object(token),
            // 单竖线开始受限单参数闭包。
            ExpressionTokenKind::Pipe => self.parse_closure(token),
            // 双竖线位于原子位置表示零参数闭包。
            ExpressionTokenKind::OrOr => Err(Diagnostic::new(
                // 指向闭包标记。
                token.span,
                // 陈述失败原因。
                "表达式不支持闭包",
                // 给出修复建议。
                "在 Rust 侧定义回调并通过 props 引用",
            )),
            // 分号表示语句结构。
            ExpressionTokenKind::Semicolon => Err(Diagnostic::new(
                // 指向分号。
                token.span,
                // 陈述失败原因。
                "表达式不支持语句",
                // 给出修复建议。
                "只保留单个表达式并把语句移到 Rust 侧",
            )),
            // 其余标记不能开始原子。
            _ => Err(Diagnostic::new(
                // 指向非法起始标记。
                token.span,
                // 陈述失败原因。
                "此处需要表达式值",
                // 给出修复建议。
                "填写标识符、字面量、分组或允许的调用",
            )),
        }
    }

    // 解析单参数单表达式受限闭包。
    fn parse_closure(&mut self, open: ExpressionToken) -> Result<Expression, Diagnostic> {
        // 闭包参数必须是普通标识符。
        let parameter = self.take_identifier(
            // 说明缺失参数名。
            "受限闭包缺少参数名",
            // 给出规范闭包形状。
            "使用 |item| expression",
        )?;
        // 提取参数文本。
        let parameter = match parameter.kind {
            // 保存普通标识符。
            ExpressionTokenKind::Identifier(value) if !value.starts_with('$') => value,
            // 美元保留名称不能成为闭包参数。
            _ => {
                // 返回闭包参数诊断。
                return Err(Diagnostic::new(
                    // 指向参数标记。
                    parameter.span,
                    // 说明参数名限制。
                    "受限闭包参数必须是普通标识符",
                    // 给出规范参数示例。
                    "使用 |item| expression",
                ));
            }
        };
        // 闭包参数后必须有闭合竖线。
        self.expect(
            // 要求第二个单竖线。
            &ExpressionTokenKind::Pipe,
            // 说明缺少闭合定界符。
            "受限闭包参数后缺少 |",
            // 给出完整形状。
            "使用 |item| expression",
        )?;
        // 闭包体是单个完整受限表达式。
        let body = self.parse_ternary()?;
        // 合并起始竖线与闭包体跨度。
        let span = merge_span(open.span, body.span);
        // 返回闭包 AST。
        Ok(Expression {
            // 保存参数与唯一表达式体。
            kind: ExpressionKind::Closure {
                // 保存参数名称。
                parameter,
                // 保存闭包体。
                body: Box::new(body),
            },
            // 保存完整闭包跨度。
            span,
        })
    }

    // 返回尾随非法结构的专用诊断。
    fn trailing_error(&self) -> Diagnostic {
        // 读取未消费标记。
        let token = self.current();
        // 按高价值非法结构选择原因和建议。
        let (message, suggestion) = match token.kind {
            // 冒号在三元和命名参数外表示类型标注。
            ExpressionTokenKind::Colon => (
                // 陈述类型标注禁用。
                "表达式不支持类型标注",
                // 建议把类型放在 Rust 侧。
                "删除类型标注并在 Rust 绑定声明中定义类型",
            ),
            // 分号表示语句。
            ExpressionTokenKind::Semicolon => (
                // 陈述语句禁用。
                "表达式不支持语句",
                // 建议保留单表达式。
                "只保留单个表达式并把语句移到 Rust 侧",
            ),
            // 左方括号可能是尾随数组结构。
            ExpressionTokenKind::LeftBrace => (
                // 陈述对象禁用。
                "表达式不支持对象字面量",
                // 建议外部绑定。
                "在 Rust 侧创建结构化数据并通过绑定引用传入",
            ),
            // 单竖线表示闭包。
            ExpressionTokenKind::Pipe => (
                // 陈述闭包禁用。
                "表达式不支持闭包",
                // 建议回调引用。
                "在 Rust 侧定义回调并通过 props 引用",
            ),
            // 其他尾随内容使用统一诊断。
            _ => (
                // 陈述存在多余内容。
                "表达式包含无法解析的尾随内容",
                // 建议限制语法。
                "只使用文档列出的受限表达式结构",
            ),
        };
        // 返回结构化诊断。
        Diagnostic::new(token.span, message, suggestion)
    }

    // 消费并返回标识符标记。
    fn take_identifier(
        &mut self,
        message: &'static str,
        suggestion: &'static str,
    ) -> Result<ExpressionToken, Diagnostic> {
        // 当前必须是标识符。
        if matches!(self.current().kind, ExpressionTokenKind::Identifier(_)) {
            // 返回消费的标识符。
            return Ok(self.advance().clone());
        }
        // 返回缺失标识符诊断。
        Err(Diagnostic::new(self.current().span, message, suggestion))
    }

    // 要求并消费指定标记。
    fn expect(
        &mut self,
        kind: &ExpressionTokenKind,
        message: &'static str,
        suggestion: &'static str,
    ) -> Result<ExpressionToken, Diagnostic> {
        // 匹配时返回消费标记。
        if self.check(kind) {
            // 克隆稳定标记所有权。
            return Ok(self.advance().clone());
        }
        // 返回缺失标记诊断。
        Err(Diagnostic::new(self.current().span, message, suggestion))
    }

    // 匹配并可选消费指定标记。
    fn take(&mut self, kind: &ExpressionTokenKind) -> bool {
        // 不匹配时保持索引。
        if !self.check(kind) {
            // 报告未消费。
            return false;
        }
        // 消费匹配标记。
        self.advance();
        // 报告消费成功。
        true
    }

    // 判断当前标记变体。
    fn check(&self, kind: &ExpressionTokenKind) -> bool {
        // 比较忽略载荷的标记种类。
        same_kind(kind, &self.current().kind)
    }

    // 判断下一标记变体。
    fn check_next(&self, kind: &ExpressionTokenKind) -> bool {
        // 安全取得下一标记并比较种类。
        self.tokens
            // 读取下一索引。
            .get(self.index + 1)
            // 比较忽略载荷的种类。
            .is_some_and(|token| same_kind(kind, &token.kind))
    }

    // 返回当前标记。
    fn current(&self) -> &ExpressionToken {
        // 结束哨兵保证索引始终有效。
        &self.tokens[self.index]
    }

    // 返回刚刚消费的标记。
    fn previous(&self) -> &ExpressionToken {
        // 调用方保证至少消费一个标记。
        &self.tokens[self.index - 1]
    }

    // 消费并返回当前标记。
    fn advance(&mut self) -> &ExpressionToken {
        // 保存消费前索引。
        let current = self.index;
        // 结束标记不再推进。
        if !self.check(&ExpressionTokenKind::End) {
            // 增加标记索引。
            self.index += 1;
        }
        // 返回消费前标记，包括结束哨兵本身。
        &self.tokens[current]
    }
}

// 比较忽略载荷的标记变体。
fn same_kind(left: &ExpressionTokenKind, right: &ExpressionTokenKind) -> bool {
    // 使用枚举判别值比较。
    std::mem::discriminant(left) == std::mem::discriminant(right)
}

// 合并两个按源码顺序排列的跨度。
fn merge_span(start: SourceSpan, end: SourceSpan) -> SourceSpan {
    // 返回从左起点到右终点的跨度。
    SourceSpan {
        // 保留左侧绝对起点。
        start: start.start,
        // 使用右侧绝对终点。
        end: end.end,
        // 保留左侧行号。
        line: start.line,
        // 保留左侧列号。
        column: start.column,
    }
}

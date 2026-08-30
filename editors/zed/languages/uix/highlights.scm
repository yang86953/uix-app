; UIX Lang 语法高亮查询：捕获名与 Zed 主题标准色映射对齐。

; 注释
(line_comment) @comment
(block_comment) @comment

; 标签与元素定界
(tag_name) @type
["<" ">" "</" "/>"] @punctuation.delimiter

; 属性与事件
(attribute_name) @attribute
(event_name) @attribute

; 字面量
(string) @string
(number) @number

; 插值与表达式定界
(interpolation ["{" "}"]) @punctuation.special
(expression ["{" "}"]) @punctuation.special

; 顶层指令
(directive_name) @keyword.directive

; 样式块
(style_property) @property
(pseudo_class) @attribute
(theme_token) @constant

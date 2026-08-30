/**
 * UIX Lang tree-sitter grammar。
 *
 * 覆盖：注释、元素（标签/属性/事件/字符串/插值）、@ 指令
 * （@import/@export/@theme/@keyframes）、样式类块（属性/伪类/#token）。
 * 文本节点排除 <、{、}、// 注释起始，避免与结构规则冲突。
 */

module.exports = grammar({
  name: 'uix',

  extras: $ => [/\s+/],

  word: $ => $.identifier,

  rules: {
    source_file: $ => repeat($._item),

    _item: $ => choice(
      $.element,
      $.directive,
      $.style_block,
      $.pseudo_style_block,
      $.line_comment,
      $.block_comment,
      $.interpolation,
      $.text,
    ),

    // 注释：行内注释优先级高于文本，保证任意位置的 // 都能高亮。
    line_comment: _ => token(prec(1, seq('//', /[^\n]*/))),
    block_comment: _ => token(prec(1, seq('/*', /[^*]*\*+([^/*][^*]*\*+)*/, '/'))),

    identifier: _ => token(prec(1, /[A-Za-z_][A-Za-z0-9_]*/)),

    // 元素：自闭合与带子节点两种形态。
    element: $ => choice(
      seq(
        '<',
        field('name', $.tag_name),
        repeat($.attribute),
        '/>',
      ),
      seq(
        '<',
        field('name', $.tag_name),
        repeat($.attribute),
        '>',
        repeat($._child),
        '</',
        field('closing_name', $.tag_name),
        '>',
      ),
    ),

    tag_name: _ => /[A-Za-z_][A-Za-z0-9_.-]*/,

    _child: $ => choice(
      $.element,
      $.interpolation,
      $.line_comment,
      $.block_comment,
      $.text,
    ),

    // 属性：具名属性（可选值）、@ 事件属性、无名表达式属性（如 <If {cond}>）。
    attribute: $ => choice(
      seq(
        field('name', choice($.attribute_name, $.event_name)),
        optional(seq('=', field('value', $.attribute_value))),
      ),
      field('value', $.expression),
    ),

    attribute_name: _ => /[A-Za-z_][A-Za-z0-9_-]*/,
    event_name: _ => /@[A-Za-z_][A-Za-z0-9_]*/,

    attribute_value: $ => choice($.string, $.expression),

    string: _ => token(choice(
      seq('"', repeat(choice(/[^"\\]/, /\\./)), '"'),
      seq("'", repeat(choice(/[^'\\]/, /\\./)), "'"),
    )),

    // 花括号包裹的表达式值；内部不细分，允许嵌套花括号与字符串。
    expression: $ => seq('{', $._expression_content, '}'),

    interpolation: $ => seq('{', $._expression_content, '}'),

    _expression_content: $ => repeat1(choice(
      $.string,
      token(prec(-1, /[^\{\}"]+/)),
      $.expression,
    )),

    // 文本：不含结构字符；含 / 的段落单独成词，// 与 /* 始终归注释。
    text: _ => token(prec(-2, repeat1(choice(
      /[^<>{}\/\n]+/,
      /\n/,
      /\/[^\/\*][^<>{}\/]*/,
      /\//,
    )))),

    // 顶层指令。
    directive: $ => choice(
      $.import_directive,
      $.export_directive,
      $.theme_directive,
      $.keyframes_directive,
    ),

    import_directive: $ => seq(
      alias(token(prec(1, '@import')), $.directive_name),
      '(',
      field('path', $.string),
      optional(seq(',', field('export', $.string))),
      ')',
    ),

    export_directive: $ => seq(
      alias(token(prec(1, '@export')), $.directive_name),
      '(',
      field('name', $.string),
      ')',
    ),

    theme_directive: $ => seq(
      alias(token(prec(1, '@theme')), $.directive_name),
      field('name', $.identifier),
      $.style_body,
    ),

    keyframes_directive: $ => seq(
      alias(token(prec(1, '@keyframes')), $.directive_name),
      field('name', $.identifier),
      $.style_body,
    ),

    // 样式类块：顶层为 `name { }`；带伪类的扩展为 `name:pseudo { }`。
    style_block: $ => seq(
      field('name', $.identifier),
      $.style_body,
    ),

    pseudo_style_block: $ => seq(
      field('name', $.identifier),
      field('pseudo', $.pseudo_class),
      $.style_body,
    ),

    style_body: $ => seq(
      '{',
      repeat(choice(
        $.style_declaration,
        $.pseudo_class_block,
        $.line_comment,
        $.block_comment,
      )),
      '}',
    ),

    pseudo_class_block: $ => seq(
      field('pseudo', $.pseudo_class),
      $.style_body,
    ),

    pseudo_class: _ => token(prec(1, /:[A-Za-z][A-Za-z0-9_-]*/)),

    style_declaration: $ => seq(
      field('property', choice($.style_property, $.theme_token)),
      ':',
      field('value', $.style_value),
      optional(';'),
    ),

    style_property: _ => /[A-Za-z_-][A-Za-z0-9_-]*/,

    // 样式值：#token 引用、数字（含单位）、字符串与任意非结构字符序列。
    style_value: $ => prec.right(repeat1(choice(
      $.theme_token,
      $.number,
      $.string,
      token(prec(-1, /[^;{}#\n]+/)),
    ))),

    theme_token: _ => /#[A-Za-z0-9_][A-Za-z0-9_.-]*/,
    number: _ => /-?[0-9]+(\.[0-9]+)?([a-zA-Z%]+)?/,
  },
});

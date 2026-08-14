# UIX 样式属性参考

[← 返回 UIX Lang 索引](README.md)

> **接口**：声明 UIX Lang 样式系统中全部属性的语法、取值范围、默认值、适用元素、映射状态与应用示例。依赖：[标签语法](标签语法.md)。导出：布局组件 → [内置组件/布局组件](内置组件/布局组件.md)。
>
> **状态声明**：公开 `uix!` 已在编译期转换标签级内联 `style`、具名样式类、多 class 叠加与无环 `extends` 继承。本页标记为 ✅ 的属性进入确定映射；标记为 🕒 或当前 Rust 类型无等价值的取值会在编译期明确拒绝。主题引用仍按目标设计推进，未登记能力不会静默忽略（见 [README](README.md)）。

## 1. 简介与使用方法

### 1.1 映射状态图例

每个属性条目标注其与 Rust API 的映射状态：

| 标记 | 含义 |
|---|---|
| ✅ 已映射 | Rust `Style` 已有对应字段（[src/ui/theme/style](../../src/ui/theme/style/mod.rs)），语义一致 |
| 🕒 规划中 | 目标设计；Rust 暂无等价字段，实现差距由 [Vikunja 项目 4](https://yang-server.tail9d5559.ts.net:3456/projects/4) 跟踪 |

### 1.2 使用方法

样式属性可通过以下四种方式使用：

1. **基础样式类**：

   ```uix
   styleName {
     propertyName: propertyValue;
   }
   ```

2. **标签级内联样式**（CSS 字符串形式，语法与样式类块一致）：

   ```uix
   <Text style="propertyName: propertyValue;" />
   ```

3. **主题样式定义**：

   ```uix
   @theme themeName {
     propertyName: propertyValue;
   }
   ```

4. **引用主题属性**（值中以 `#属性名` 引用）：

   ```uix
   styleName {
     propertyName: #themeProperty;
   }
   ```

**优先级**：内联样式 > 样式类 > 主题默认值。

## 2. 布局属性

### 2.1 display

**映射**：✅ 已映射（`Style.display: DisplayMode`）

**语法**：`display: value`

**取值范围**：

- `flex`：弹性布局（默认，UIX 布局引擎主模式）
- `grid`：网格布局
- `none`：隐藏元素（不参与布局与渲染）

**默认值**：`flex`

**适用元素**：所有元素

**示例**：

```uix
flexContainer {
  display: flex;
  justifyContent: center;
}

<Container class="flexContainer">
  <Text>Flex 容器</Text>
</Container>
```

> 说明：UIX 引擎没有 `block` / `inline` / `inline-block` 模式（元素一律以弹性盒子参与布局）；块级/内联观感由 `direction`、`alignItems` 等控制。

### 2.2 flexDirection

**映射**：✅ 已映射（`Style.flex_direction: FlexDirection`）

**语法**：`flexDirection: value`

**取值范围**：

- `row`：主轴水平，从左到右
- `column`：主轴垂直，从上到下（默认）

**默认值**：`column`

**适用元素**：弹性容器

**示例**：

```uix
toolbar {
  flexDirection: row;
  gap: 8px;
}
```

### 2.3 justifyContent / alignItems

**映射**：✅ 已映射（`Style.justify_content` / `Style.align_items`）

**语法**：`justifyContent: value`、`alignItems: value`

**取值范围**：

- `justifyContent`（主轴）：`flex-start`、`flex-end`、`center`、`space-between`、`space-around`
- `alignItems`（交叉轴）：`flex-start`、`flex-end`、`center`、`stretch`（默认）

**默认值**：`justifyContent: flex-start`、`alignItems: stretch`

**适用元素**：弹性容器

**示例**：

```uix
centeredRow {
  flexDirection: row;
  justifyContent: center;
  alignItems: center;
}
```

### 2.4 gap

**映射**：✅ 已映射（`Style.gap`）

**语法**：`gap: value`

**取值范围**：长度值（如 `8px`、`12px`）

**默认值**：`0`

**适用元素**：弹性容器与网格容器

**示例**：

```uix
spacedList {
  gap: 8px;
}
```

### 2.5 flexGrow / flexShrink

**映射**：✅ 已映射（`Style.flex_grow` / `Style.flex_shrink`）

**语法**：`flexGrow: number`、`flexShrink: number`

**取值范围**：非负数字（如 `0`、`1`、`2`）

**默认值**：`flexGrow: 0`、`flexShrink: 1`

**适用元素**：弹性容器中的子元素

**示例**：

```uix
growPane {
  flexGrow: 1;
}
```

### 2.6 flexWrap

**映射**：✅ 已映射（`Style.flex_wrap: bool`）

**语法**：`flexWrap: true | false`

**默认值**：`false`

**适用元素**：弹性容器

### 2.7 alignSelf

**映射**：✅ 已映射（`Style.align_self: Option<AlignItems>`）

**语法**：`alignSelf: value`

**取值范围**：同 `alignItems`（`flex-start`、`flex-end`、`center`、`stretch`）

**默认值**：无（跟随容器 `alignItems`）

**适用元素**：弹性容器中的子元素

### 2.8 gridTemplateColumns / gridTemplateRows

**映射**：✅ 已映射（`Style.grid_template_columns` / `Style.grid_template_rows: Vec<GridTrack>`）

**语法**：`gridTemplateColumns: value`

**取值范围**：轨道列表，每个轨道为固定长度（如 `100px`）或弹性比例（如 `1fr`）

**默认值**：无（Grid 布局需显式声明轨道）

**适用元素**：`display: grid` 容器

**示例**：

```uix
twoColumnGrid {
  display: grid;
  gridTemplateColumns: 1fr 1fr;
  gap: 16px;
}
```

### 2.9 gridColumnGap / gridRowGap

**映射**：✅ 已映射（`Style.grid_column_gap` / `Style.grid_row_gap`）

**语法**：`gridColumnGap: value`、`gridRowGap: value`

**取值范围**：长度值

**默认值**：`0`

**适用元素**：网格容器

### 2.10 gridColumnSpan / gridRowSpan

**映射**：✅ 已映射（`Style.grid_column_span` / `Style.grid_row_span`）

**语法**：`gridColumnSpan: number`、`gridRowSpan: number`

**取值范围**：正整数

**默认值**：`1`

**适用元素**：网格容器中的子元素

### 2.11 position 与 top / right / bottom / left

**映射**：🕒 规划中（UIX 引擎以 flex/grid 为主，定位能力为目标设计）

**语法**：`position: value`

**取值范围**：`static`、`relative`、`absolute`、`fixed`、`sticky`

**默认值**：`static`

**适用元素**：所有元素

**示例**：

```uix
absoluteElement {
  position: absolute;
  top: 10px;
  right: 10px;
}
```

> 当前等效做法：浮动/悬浮区域用 [浮层系统](../使用/事件.md) 与定位容器实现；`position` 系列属性落地后由 Vikunja 跟踪。

### 2.12 z-index

**映射**：🕒 规划中

**语法**：`z-index: value`

**取值范围**：整数

**默认值**：`auto`

**适用元素**：所有元素

**示例**：

```uix
modal {
  position: fixed;
  z-index: 1000;
}
```

### 2.13 float / clear

**映射**：🕒 规划中

**语法**：`float: left | right | none`、`clear: left | right | both | none`

**默认值**：`none`

**适用元素**：块级元素

> 说明：UIX 布局引擎无浮动模型；等效效果优先用 `flexDirection: row` + `justifyContent` / `flexGrow` 表达。本组属性仅作目标设计保留。

## 3. 盒模型属性

### 3.1 width / height

**映射**：✅ 已映射（`Style.width` / `Style.height: Option<f32>`）

**语法**：`width: value`、`height: value`

**取值范围**：长度值（如 `100px`、`50%`）或 `auto`

**默认值**：`auto`

**适用元素**：所有元素

**示例**：

```uix
fullWidth {
  width: 100%;
}
```

### 3.2 margin

**映射**：✅ 已映射（`Style.margin: EdgeInsets`）

**语法**：`margin: value` 或 `marginTop: value`、`marginRight: value`、`marginBottom: value`、`marginLeft: value`

**取值范围**：长度值或百分比

**默认值**：`0`

**适用元素**：所有元素

**示例**：

```uix
marginExample {
  margin: 10px 20px;
}
```

### 3.3 padding

**映射**：✅ 已映射（`Style.padding: EdgeInsets`）

**语法**：`padding: value` 或 `paddingTop: value`、`paddingRight: value`、`paddingBottom: value`、`paddingLeft: value`

**取值范围**：长度值或百分比

**默认值**：`0`

**适用元素**：所有元素

**示例**：

```uix
paddingExample {
  padding: 15px;
}
```

### 3.4 borderColor / borderWidth

**映射**：✅ 已映射（`Style.border_color` / `Style.border_width: EdgeInsets`）

**语法**：`borderColor: value`、`borderWidth: value`（四边）或 `borderTopWidth: value` 等单边形式

**取值范围**：颜色值；长度值

**默认值**：`borderColor: none`、`borderWidth: 0`

**适用元素**：所有元素

**示例**：

```uix
borderExample {
  borderColor: #ccc;
  borderWidth: 1px;
}
```

### 3.5 borderRadius

**映射**：✅ 已映射（`Style.border_radius: f32`）

**语法**：`borderRadius: value`

**取值范围**：长度值（如 `4px`、`8px`）

**默认值**：`0`

**适用元素**：所有元素

**示例**：

```uix
roundedCorners {
  borderRadius: 8px;
}
```

### 3.6 borderStyle

**映射**：🕒 规划中（Rust `Style` 暂无线型字段，实线为默认线型）

**语法**：`borderStyle: none | solid | dashed | dotted | double`

**默认值**：`solid`

**适用元素**：所有元素

## 4. 文本属性

### 4.1 color

**映射**：✅ 已映射（`Style.color: ColorValue`）

**语法**：`color: value`

**取值范围**：颜色值（如 `#000`、`rgb(0,0,0)`、`blue`）

**默认值**：主题默认文本色

**适用元素**：文本元素

**示例**：

```uix
redText {
  color: #ff0000;
}
```

### 4.2 fontSize

**映射**：✅ 已映射（`Style.font_size: TypographyToken`，支持 token 名或像素值）

**语法**：`fontSize: value`

**取值范围**：字号 token（`small`、`body`、`large`、`xlarge`、`heading1`~`heading5`）或长度值（如 `14px`）

**默认值**：`body`

**适用元素**：文本元素

**示例**：

```uix
largeText {
  fontSize: 24px;
}

titleText {
  fontSize: heading3;
}
```

### 4.3 fontFamily

**映射**：🕒 规划中

**语法**：`fontFamily: value`

**取值范围**：字体名称列表（如 `Arial, sans-serif`）

**默认值**：系统默认字体

**适用元素**：文本元素

### 4.4 fontWeight

**映射**：🕒 规划中

**语法**：`fontWeight: value`

**取值范围**：`normal`、`bold` 或数字 `100`~`900`

**默认值**：`normal`

**适用元素**：文本元素

### 4.5 textAlign

**映射**：🕒 规划中

**语法**：`textAlign: value`

**取值范围**：`left`、`right`、`center`、`justify`

**默认值**：`left`

**适用元素**：块级文本元素

### 4.6 textDecoration

**映射**：🕒 规划中

**语法**：`textDecoration: value`

**取值范围**：`none`、`underline`、`overline`、`line-through`

**默认值**：`none`

**适用元素**：文本元素

### 4.7 lineHeight

**映射**：🕒 规划中

**语法**：`lineHeight: value`

**取值范围**：数字（如 `1.5`）或长度值（如 `24px`）

**默认值**：`normal`

**适用元素**：文本元素

## 5. 背景属性

### 5.1 backgroundColor

**映射**：✅ 已映射（`Style.background`，含 hover / focus / active 状态色）

**语法**：`backgroundColor: value`

**取值范围**：颜色值（如 `#fff`、`rgb(255,255,255)`、`transparent`）

**默认值**：`transparent`

**适用元素**：所有元素

**示例**：

```uix
whiteBackground {
  backgroundColor: #ffffff;
}
```

> 状态色：`backgroundColor:hover` / `backgroundColor:focus` / `backgroundColor:active` 形式声明交互状态背景（对应 `Style.background_hover` 等字段）。

### 5.2 backgroundImage

**映射**：🕒 规划中

**语法**：`backgroundImage: url('image.png')` 或渐变函数

**默认值**：`none`

**适用元素**：所有元素

### 5.3 backgroundPosition

**映射**：🕒 规划中

**语法**：`backgroundPosition: value`

**取值范围**：长度值或关键字（如 `center`、`top left`）

**默认值**：`0% 0%`

**适用元素**：所有元素

### 5.4 backgroundRepeat

**映射**：🕒 规划中

**语法**：`backgroundRepeat: value`

**取值范围**：`repeat`、`repeat-x`、`repeat-y`、`no-repeat`

**默认值**：`repeat`

**适用元素**：所有元素

## 6. 动画与过渡

> 当前 UIX 动画能力通过动画 API 声明（见[动画](../使用/动画.md)）；`Style` 中的动画字段为目标设计。

### 6.1 animation

**映射**：🕒 规划中（目标设计）

**语法**：`animation: name duration timing-function delay iteration-count direction fill-mode`

**取值范围**：

- `name`：动画名称
- `duration`：持续时间（如 `1s`）
- `timing-function`：缓动函数（如 `ease`、`linear`）
- `delay`：延迟（如 `0s`）
- `iteration-count`：重复次数（如 `infinite`）
- `direction`：方向（如 `normal`、`reverse`）
- `fill-mode`：填充模式（如 `forwards`）

**默认值**：`none 0s ease 0s 1 normal none`

**适用元素**：所有元素

**示例**：

```uix
fadeIn {
  animation: fade 1s ease-in-out;
}

@keyframes fade {
  from {
    opacity: 0;
  }
  to {
    opacity: 1;
  }
}
```

### 6.2 transition

**映射**：🕒 规划中（目标设计）

**语法**：`transition: property duration timing-function delay`

**取值范围**：过渡属性（如 `all`、`backgroundColor`）、时长、缓动、延迟

**默认值**：`all 0s ease 0s`

**适用元素**：所有元素

**示例**：

```uix
normalAction {
  padding: 6px 12px;
  backgroundColor: #ffffff;
}

hoverAction {
  padding: 8px 16px;
  backgroundColor: #e6f4ff;
}

<Component name="HoverAction">
  <Button class="normalAction"
          @mouseEnter="setStyle('hoverAction')"
          @mouseLeave="setStyle('')">
    悬停并恢复原始 class
  </Button>
</Component>

<HoverAction />
```

> `transition` 仍为规划中字段；上述示例使用已映射字段展示 `setStyle` 的组件内动态 class。内联 `style` 会继续覆盖每个动态 class 分支中的同名字段。

## 7. 变换属性

### 7.1 transform

**映射**：🕒 规划中

**语法**：`transform: value`

**取值范围**：变换函数（`translate()`、`rotate()`、`scale()`、`skew()`）

**默认值**：`none`

**适用元素**：所有元素

**示例**：

```uix
rotatedElement {
  transform: rotate(45deg);
}
```

### 7.2 transformOrigin

**映射**：🕒 规划中

**语法**：`transformOrigin: value`

**取值范围**：长度值或关键字（如 `center`、`top left`）

**默认值**：`50% 50% 0`

**适用元素**：所有元素

## 8. 交互属性

### 8.1 cursor

**映射**：🕒 规划中

**语法**：`cursor: value`

**取值范围**：`default`、`pointer`、`text`、`move`、`not-allowed`

**默认值**：`default`

**适用元素**：所有元素

**示例**：

```uix
clickable {
  cursor: pointer;
}
```

### 8.2 userSelect

**映射**：🕒 规划中（当前文本选择能力见 [Label](../使用/数据展示.md#label)）

**语法**：`userSelect: value`

**取值范围**：`auto`、`none`、`text`、`all`

**默认值**：`auto`

**适用元素**：所有元素

## 9. 其他属性

### 9.1 opacity

**映射**：✅ 已映射（`Style.opacity: f32`）

**语法**：`opacity: value`

**取值范围**：`0`~`1` 之间的数字

**默认值**：`1`

**适用元素**：所有元素

**示例**：

```uix
semiTransparent {
  opacity: 0.5;
}
```

### 9.2 overflow

**映射**：✅ 已映射（`Style.overflow_content: bool`；滚动容器见 [ScrollView](内置组件/布局组件.md)）

**语法**：`overflow: value`

**取值范围**：`visible`、`hidden`（`scroll` / `auto` 通过滚动容器组件声明）

**默认值**：`visible`

**适用元素**：块级元素

**示例**：

```uix
clipContent {
  overflow: hidden;
}
```

### 9.3 boxShadow

**映射**：✅ 已映射（`Style.box_shadow: BoxShadowDef`：color / blur / offset_x / offset_y；spread 为目标设计）

**语法**：`boxShadow: h-offset v-offset blur color`

**取值范围**：

- `h-offset`：水平偏移（如 `2px`）
- `v-offset`：垂直偏移（如 `2px`）
- `blur`：模糊半径（如 `4px`）
- `color`：阴影颜色（如 `rgba(0,0,0,0.2)`）

**默认值**：`none`

**适用元素**：所有元素

**示例**：

```uix
shadowEffect {
  boxShadow: 0 2px 4px rgba(0,0,0,0.1);
}
```

### 9.4 visible

**映射**：✅ 已映射（`Style.visible: bool`）

**语法**：`visible: true | false`

**默认值**：`true`

**适用元素**：所有元素

**示例**：

```uix
hiddenBanner {
  visible: false;
}
```

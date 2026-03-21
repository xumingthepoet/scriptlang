# Macro Enhancement Plan

本文档描述下一阶段宏系统增强的目标、边界和实施顺序。它不是当前实现说明；仓库当前真实状态仍以 [`IMPLEMENTATION.md`](/Users/xuming/work/scriptlang-new/IMPLEMENTATION.md) 为准。

Last updated: 2026-03-21

## Goal

在充分考虑 `scriptlang` 自身特殊性的前提下，把现有 MVP 宏系统增强到更接近 Elixir 的能力模型：

1. 宏展开继续以 env-driven expand 为中心
2. builtin form 和 macro 继续共享统一分派入口
3. 提供 `quote / unquote`
4. 提供显式的编译期环境、局部变量和编译期逻辑
5. 为 `unless` / `if-else` 这类标准库宏铺平道路
6. 为后续在 `kernel` 或普通 module 中持续扩语法提供稳定基线

## Non-Goals

本阶段明确不做：

- 不让 macro 深度改写 Rhai 表达式内部控制流
- 不把 expr 变成完整通用 AST 编译器
- 不让 macro 直接生成 runtime IR
- 不把 runtime temp 和 macro 局部变量混成同一种语法
- 不在这一阶段实现 Elixir 级别完整 hygiene、module lifecycle 或 user macro surface

## Transitional Syntax To Be Removed

当前仓库里已经存在一套 MVP 宏表层语法：

- `scope="statement"` / `scope="module"`
- `{{attr_name}}` 属性替换
- `<yield/>` children 拼接

这套能力只用于验证 env-driven expand、module/import macro visibility 和最小 macro dispatch 链路，不应被视为长期设计。

本计划的目标不是继续增强这套模板式宏语法，而是逐步废弃它，并用真正的宏能力替代：

- 用 `quote / unquote` 替代 `{{...}}` 文本替换
- 用 compile-time AST splice 替代 `<yield/>` 的特例拼接
- 用 compile-time environment 和 macro evaluator 替代“只靠 scope + 模板拼接”的宏实现方式

也就是说，现有 MVP 宏语法是过渡方案，不是目标方案。

## Language-Specific Constraints

`scriptlang` 的宏系统必须服从当前语言边界：

1. 表达式当前仍以 Rhai 文本为主
   - 宏阶段可以整体搬运 expr
   - 可以做变量名级别的 hygiene 重写
   - 不允许任意深入改函数名、控制结构、运算结构

2. 宏操作对象应以 `Form` AST 为主
   - `quote` 返回的是 `Form` / `FormItem` 结构，不是字符串源码
   - `unquote` 按槽位类型 splice，而不是文本替换

3. 宏阶段和运行阶段必须严格分离
   - `<temp>` 继续表示 runtime temp
   - 宏局部变量必须有独立 compile-time 语义

## Target Model

目标模型接近 Elixir，但不照搬表层语法。

### 1. Compile-Time Values

引入 compile-time value system，至少覆盖：

- `AstNode(Form)`
- `AstItems(Vec<FormItem>)`
- `Expr(ExprSource)`
- `String(String)`
- `Bool(bool)`
- `Int(i64)`
- `Nil`

后续如有需要再扩展，但第一阶段不要做大而全。

### 2. Macro Environment

引入显式 `MacroEnv`，至少包含：

- `current_module`
- `imports`
- `macro_name`
- `attributes`
- `content`
- `locals`
- `gensym_counter`

这层环境用于承载：

- `get_attribute`
- `get_content`
- `gensym`
- 当前 module / import 可见性
- 后续编译期辅助函数

### 3. Quote / Unquote

`quote` 和 `unquote` 的核心规则：

- `<quote>` 产出 AST，不产出 XML 字符串
- `<unquote>` 只能在 `<quote>` 内部使用
- `unquote` 的可插入值由上下文槽位决定

建议支持的 splice 规则：

- children 位置：`AstNode` / `AstItems`
- expr 槽位：`Expr`
- 普通字符串属性：`String`

类型不匹配时直接编译报错。

### 4. Compile-Time Bindings

不要复用 runtime `<temp>` 做宏变量。建议新增 compile-time binding form，例如：

```xml
<let name="when_expr" type="expr">
  <get-attribute name="when" />
</let>
```

也就是说：

- `<temp>` 仍属于运行时语义
- `<let>` 属于宏求值语义

### 5. Minimal Hygiene

本阶段只做“绑定名 hygiene”，不做更深的 expr hygiene。

目标是：

- 宏 quote 中引入的 runtime 绑定名自动 gensym
- 对应 expr 中的变量引用同步改名
- 不尝试改写 expr 内部函数调用、运算结构或控制逻辑

## Example Target

目标是能自然表达类似这样的宏：

```xml
<macro name="unless" attributes="when:expr" content="ast">
  <let name="when_expr" type="expr">
    <get-attribute name="when" />
  </let>

  <let name="content_ast" type="ast">
    <get-content />
  </let>

  <quote>
    <temp name="condition" type="bool">
      <unquote>when_expr</unquote>
    </temp>
    <if when="!condition">
      <unquote>content_ast</unquote>
    </if>
  </quote>
</macro>
```

这里的关键点是：

- `when_expr` 是 compile-time expr value
- `content_ast` 是 compile-time AST value
- `quote` 产出的是高层 `Form` AST
- `condition` 是 quote 中引入的 runtime temp，需要 hygiene

同一套机制也应足以承载标准 `if-else` 宏：

```xml
<macro name="if-else" attributes="when:expr" content="ast">
  <let name="when_expr" type="expr">
    <get-attribute name="when" />
  </let>

  <let name="do_ast" type="ast">
    <get-content head="do" />
  </let>

  <let name="else_ast" type="ast">
    <get-content head="else" />
  </let>

  <quote>
    <temp name="condition" type="bool">
      <unquote>when_expr</unquote>
    </temp>
    <if when="condition">
      <unquote>do_ast</unquote>
    </if>
    <if when="!condition">
      <unquote>else_ast</unquote>
    </if>
  </quote>
</macro>
```

这要求 `get_content` 不只是“取全部 children”，还要支持按直接子标签分槽取 AST。

## Required Refactors

在正式落地 `quote / unquote` 之前，至少要补这几层。

### 1. Expand Context

继续把现有 [`semantic/expand/`](/Users/xuming/work/scriptlang-new/crates/sl-compiler/src/semantic/expand) 收敛成真正的宏语义中心：

- `ExpandEnv` 继续作为统一入口
- 新增 `MacroEnv`
- `ExpandRegistry` 同时支持 builtin 和 macro-time evaluator hooks

### 2. Expr Boundary

保持当前“expr 只整体操作”的原则，但把 compile-time expr value 形式化：

- 继续复用 [`semantic/expr/`](/Users/xuming/work/scriptlang-new/crates/sl-compiler/src/semantic/expr)
- 引入宏期可传递的 `ExprSource`
- 不做完整 AST
- 允许变量级别 rewrite，禁止深层语义改写

### 3. Macro Evaluator

新增一层 macro evaluator，负责：

- 运行 `<let>`
- 提供 `get_attribute` / `get_content`
- 执行 `quote`
- 解析 `unquote`
- 维护 `MacroEnv.locals`

### 4. Quote Builder

需要一个专门的 quote builder：

- 读取 `<quote>` 内部 form
- 在合适位置执行 `unquote`
- 返回 `Form` / `FormItem` 结果
- 做 hygiene 所需的局部 gensym

## Suggested File Layout

建议在当前 `semantic/expand/` 之下新增：

- `macros.rs`
  - 宏定义查找
  - 宏入口分发
- `macro_env.rs`
  - `MacroEnv`
  - compile-time bindings
- `macro_values.rs`
  - `MacroValue`
- `macro_eval.rs`
  - `<let>`
  - `get_attribute`
  - `get_content`
- `quote.rs`
  - `quote / unquote`
  - splice 规则
  - hygiene helper

现有文件的角色：

- [`dispatch.rs`](/Users/xuming/work/scriptlang-new/crates/sl-compiler/src/semantic/expand/dispatch.rs)
  继续做 expand dispatch
- [`macros.rs`](/Users/xuming/work/scriptlang-new/crates/sl-compiler/src/semantic/expand/macros.rs)
  承载宏定义收集和宏展开
- [`quote.rs`](/Users/xuming/work/scriptlang-new/crates/sl-compiler/src/semantic/expand/quote.rs)
  承载 `quote / unquote` 与最小 hygiene
- [`scripts.rs`](/Users/xuming/work/scriptlang-new/crates/sl-compiler/src/semantic/expand/scripts.rs)
  继续做 runtime-side script lowering
- [`expr/`](/Users/xuming/work/scriptlang-new/crates/sl-compiler/src/semantic/expr)
  继续承载 expr 前端边界

## Implementation Phases

### Phase 1: Compile-Time Value Layer

目标：

- 引入 `MacroValue`
- 引入 `MacroEnv`
- 接入 `ExpandRegistry`

完成标准：

- 宏系统内部不再只能做 `{{attr_name}}` 文本替换
- 已能从结构上表达 `ast` / `expr` / `string`

### Phase 2: Quote / Unquote

目标：

- 实现 `<quote>`
- 实现 `<unquote>`
- 支持 children / expr / string 槽位 splice

完成标准：

- 能用 `quote` 产出可继续进入 `expand` / `assemble` 的 `Form`
- 能把 attribute expr 和 content AST 拼回 quote 结果
- 新宏不再依赖 `{{attr_name}}` 和 `<yield/>`

### Phase 3: Compile-Time Let And Builtins

目标：

- 实现 `<let>`
- 实现 `get_attribute`
- 实现 `get_content`
- 实现最小 compile-time builtin function 集

完成标准：

- 不依赖 `{{attr_name}}` 文本替换也能写简单实用宏

### Phase 4: Minimal Hygiene

目标：

- 对 quote 中引入的 runtime binding 做 gensym
- 对应 expr 中变量引用同步改写

完成标准：

- `unless` / 包裹型宏不会和调用点 temp 名直接冲突

### Phase 5: Standard Library Macro Examples

目标：

- 在 `kernel` 中落地 1-2 个真正依赖 `quote/unquote` 的宏
- 首选 `unless`
- 再补 `if-else`
- 可选 `with_temp` / `surround_if`

完成标准：

- 集成测试覆盖真实宏展开链路
- 旧的 `{{...}}` / `<yield/>` 路径进入废弃状态，不再作为推荐写法

## Testing Strategy

至少覆盖：

1. `MacroValue` 类型和值传递
2. `quote` 返回 AST
3. `unquote` 在不同槽位的合法/非法 splice
4. `<let>` 局部变量遮蔽
5. `get_attribute("when")` 返回 expr value
6. `get_content()` 返回 AST items
7. imported macro 继续可见
8. hygiene 至少覆盖 temp 名冲突场景
9. `unless` / `if-else` 集成测试

## Current Status

当前仓库已具备的基础：

- env-driven `expand`
- builtin / macro 共用 `ExpandRegistry`
- module / imported macro 可见性
- `semantic/expr/` 统一 expr 前端边界
- 已有可工作的 compile-time 宏链路：
  - `<let>`
  - `<get-attribute>`
  - `<get-content>`
  - `<quote>`
  - `<unquote>`
  - 最小 temp hygiene
- `kernel` 中已经可以写出标准 `unless` 和 `if-else` 宏

当前尚未具备：

- 完整的显式 `MacroEnv` 公共模型
- 更完整的 compile-time AST / expr / string value system
- 更广泛的 quote splice 规则
- 超出 temp 变量名的更完整 hygiene
- 用真正宏系统完全替代旧的 `{{...}}` / `<yield/>` 路径

## Exit Criteria

当以下条件同时满足时，本计划可视为完成：

- `quote / unquote` 已进入主路径
- `MacroEnv` 和 compile-time values 已落地
- `kernel` 中已有基于 `quote / unquote` 的真实宏
- 至少两个标准宏可以稳定工作：`unless` 和 `if-else`
- `IMPLEMENTATION.md` 已足够描述这些能力

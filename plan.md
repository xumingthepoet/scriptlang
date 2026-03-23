# Next Plan

本文档是当前阶段交给下级 agent 的执行计划。它只描述“从当前实现继续往前推进”的工作，不重复已经完成的历史任务。

当前仓库已经完成了这些关键节点：

- compile-time macro language 已经落地最小 evaluator
- module reducer 已经让宏生成的 module-level form 真正回灌定义期环境
- `use -> require + __using__` 已经能跑通
- kernel 控制流宏已经迁移到新参数协议

所以接下来的目标，不是再把 `if / unless / use` 这类个案补功能，而是继续缩小 sl 和 Elixir 在“宏系统精神”上的差距。

当前剩余差距主要集中在 6 个方面：

- 远程宏调用还不是严格的 module-qualified dispatch
- compile-time value / quote / remote invoke 的值模型还没有统一
- AST 还不是一等 compile-time 数据，宏对 AST 的操作能力偏弱
- 缺少 module-level compile-time 累积状态，难以承载 Elixir 风格的“注册型 DSL”
- caller env 和错误定位还太薄
- hygiene 还主要覆盖 `<temp>`，尚未覆盖隐藏 helper 定义
- compile-time language 还是固定 DSL，缺少足够的控制/遍历/匹配能力来支撑真实 narrative DSL 宏库

## 对独立分析方案的取舍

下面两点要明确写给下级 agent，避免在错误前提上发散：

- “宏递归展开”不是当前主缺口。
  - 现有实现已经有两层递归展开主路径：
    - `expand_generated_items()` 会对宏产出的 form 再次走 `expand_form_items()`
    - `module_reducer` 会把宏产出的 module-level form 重新入队，再次进入定义期 reducer
  - 因此“先补一个递归展开框架”不是当前优先事项。
  - 如果后续发现组合宏回归缺陷，处理方式应是补回归测试并修具体 bug，而不是重开一个大阶段。
- “同一语言原则”不作为当前目标。
  - Elixir 的宏与 runtime 共享同一宿主语言；sl 明确不是这个路线。
  - sl 的目标是保持 compiler/runtime 边界清晰，用 compile-time language 承载高层构造。
  - 因此当前要追的是“宏协议、AST 操作能力、组合性、hygiene、编译期累积状态”，不是让宏去复用 Rhai 本身。

独立分析里真正值得吸收的点有三项：

- 把“module attribute / module-level compile-time accumulation”单独拉成主任务
- 把 compile-time language 的后续增强写实成“遍历 / 匹配 / 组合”，而不是抽象表述
- 在非目标里明确：表达式级 quote、宏作为一等值等更激进元编程能力，不是当前阶段主线

## 执行规则

- 每一步都必须单独可落地、可验收、可回滚
- 只要这一步修改了 `crates/` 下代码，完成前必须跑通 `make gate`
- 只要这一步改变了 crate 行为、宏系统语义、编译流程、测试结构或支持范围，必须同步更新 `IMPLEMENTATION.md`
- 每一步除了单元测试，还必须补对应的 `crates/sl-integration-tests/examples`
- 不要为了“先过例子”往 runtime 下沉新 primitive；本计划默认 runtime 基本不动，重点在 compiler

## 1. 真正的 Module-Qualified Remote Macro Dispatch

Status: **已完成** (2026-03-23)

目标：

- 让 `invoke_macro(module_ref, macro_name, args)` 真正按目标 module 精确分派
- 不允许再通过“当前 module / require 列表里的同名宏”误命中
- 让 `use module="x"` 的 `__using__` 调用语义严格绑定到 `x.__using__`

主要代码落点：

- `crates/sl-compiler/src/semantic/env.rs`
- `crates/sl-compiler/src/semantic/macro_lang/builtins.rs`
- 如有必要，新增 `ProgramState::resolve_macro_in(module, name)`

具体工作：

- 在 `ProgramState` 中增加“按 module + macro name 精确查找”的 API
- 保留现有 `resolve_macro(name)` 作为普通非限定宏调用路径
- `invoke_macro` 改成先解析目标 module，再显式命中该 module 下的宏定义
- 私有宏可见性检查基于“目标 module + caller module”做，不再混杂在裸名字解析里
- 对不存在目标 module、目标 module 中不存在该宏、未 `require` 的情况分别给出稳定报错

验收 examples：

- `51-remote-macro-targeted-dispatch`
  - `a` 和 `b` 都定义同名宏 `mk`
  - caller 明确调用 `invoke_macro("a", "mk", ...)`
  - 结果必须稳定命中 `a.mk`
- `52-remote-macro-target-does-not-fall-back-to-local`
  - caller 自己也定义同名宏
  - 远程调用仍必须命中 provider module，而不是 caller local
- `53-invalid-invoke-macro-target-not-required`
  - 未 require 时直接远程调用
  - `error.txt` 断言错误文本里必须明确 target module 不在 scope

完成定义：

- `invoke_macro` 的行为不再依赖当前 `resolve_macro(name)` 的查找顺序
- `use` 的 `__using__` 分派不再受同名宏污染

## 2. 统一 Compile-Time Value / Quote / Remote Invoke 的值模型

Status: pending

目标：

- 消除 `CtValue`、`MacroValue`、`quote/unquote`、`invoke_macro` 之间的临时桥接和语义丢失
- 让 `ast / keyword / list / module` 都能作为一等 compile-time 值跨宏边界流动

主要代码落点：

- `crates/sl-compiler/src/semantic/macro_lang/ast.rs`
- `crates/sl-compiler/src/semantic/macro_lang/eval.rs`
- `crates/sl-compiler/src/semantic/expand/macro_values.rs`
- `crates/sl-compiler/src/semantic/expand/quote.rs`
- `crates/sl-compiler/src/semantic/macro_lang/builtins.rs`

具体工作：

- 明确 `CtValue` 与 `MacroValue` 的长期关系
- 如果继续保留两套值类型，必须做到信息不丢失
- `List`、`Keyword`、`Ast`、`ModuleRef` 不能再在桥接时退化成字符串占位值
- `invoke_macro` 允许传递嵌套 `keyword`、`list`、`ast`、`module`
- `quote/unquote` 对 compile-time 值的支持范围要和值模型一致

验收 examples：

- `54-remote-macro-pass-ast`
  - caller 把 AST 参数传给远程宏
  - provider 原样 splice 回最终结果
- `55-remote-macro-pass-nested-keyword`
  - `use` 或远程宏参数里包含嵌套 keyword/list
  - provider 能正确读取内部字段
- `56-quote-roundtrip-list-and-keyword`
  - compile-time list/keyword 经 quote/unquote 后语义不丢失

完成定义：

- `CtValue -> MacroValue` 不再出现“list 变假 keyword / caller_env 变字符串占位”这种临时退化
- 远程宏参数模型和本地宏参数模型一致

## 3. 把 AST 提升为一等 Compile-Time 数据

Status: pending

目标：

- 让宏真正能“编程地操作 AST”，而不是只会 `get_content(head="...") + quote`
- 继续保持 XML-native 表层，但 compile-time 层必须获得结构化 AST API

主要代码落点：

- `crates/sl-compiler/src/semantic/macro_lang/ast.rs`
- `crates/sl-compiler/src/semantic/macro_lang/builtins.rs`
- 如有必要，新增 `crates/sl-compiler/src/semantic/macro_lang/ast_ops.rs`

具体工作：

- 定义长期可维护的 AST 值表示
- 提供最小但够用的 AST builtins，例如：
  - `ast_head`
  - `ast_children`
  - `ast_attr_get`
  - `ast_attr_set`
  - `ast_wrap`
  - `ast_concat`
  - `ast_filter_head`
- 保证 text/form 顺序不丢
- 保证 AST 改写后仍能回到 reducer / quote 主路径

验收 examples：

- `57-ast-rewrite-by-head`
  - 宏按 head 精确选中某类节点并改写
- `58-ast-wrap-content-preserve-order`
  - 宏包装 body，但 text/form 顺序完全保持
- `59-ast-build-module-fragments`
  - 宏通过 AST API 组合出多个 script/choice 片段

完成定义：

- 宏作者不再只能依赖 `get_content(head="...")`
- 可以做结构化 AST 选择、改写和重组

## 4. 丰富 Caller Env 与错误定位

Status: pending

目标：

- 让宏系统具备更接近 Elixir 的 caller 感知能力
- 让复杂宏链条里的错误报告可诊断，而不是只靠拼接字符串

主要代码落点：

- `crates/sl-compiler/src/semantic/expand/macro_env.rs`
- `crates/sl-compiler/src/semantic/macro_lang/builtins.rs`
- `crates/sl-core/src/error.rs`
- 必要时补充 pipeline / expand 错误包装逻辑

具体工作：

- `caller_env()` 至少补到：
  - current module
  - source file
  - line / column
  - current macro name
  - imports / requires / aliases
  - 调用栈或最小 expansion trace
- 远程宏、`use`、嵌套宏失败时，错误需要同时带：
  - provider 信息
  - caller 信息
  - 尽量精确的 source 位置

验收 examples：

- `60-macro-caller-env-source-location`
  - 宏读取 caller 文件与行号并写进输出
- `61-invalid-use-error-has-provider-and-caller`
  - `use` 失败时错误文本同时带 provider 和 caller
- `62-macro-expansion-stack-nested-error`
  - 嵌套宏失败时错误文本能看出扩展链路

完成定义：

- 调试宏不再只能靠猜
- `caller_env()` 对真实宏库实现已经足够有用

## 5. Module-Level Compile-Time Accumulation（Elixir 式模块属性精神）

Status: pending

目标：

- 给 macro system 增加 module-level compile-time 累积状态
- 让 DSL 能实现“注册型”编译期协议，而不只是即时展开
- 捕捉 Elixir `@attr` 机制背后的精神，而不是复制其表层语法

主要代码落点：

- `crates/sl-compiler/src/semantic/env.rs`
- `crates/sl-compiler/src/semantic/macro_lang/builtins.rs`
- 如有必要，新增：
  - `crates/sl-compiler/src/semantic/expand/module_attrs.rs`
  - 或把 module-level compile-time state 直接收敛进 `ExpandEnv`

具体工作：

- 在 `ExpandEnv` 引入 module-level compile-time state
  - 不要求照抄 Elixir `@attr`
  - 但必须支持“同一 module 内前面宏写、后面宏读”的累积模型
- 提供最小 builtin：
  - `module_get(name)`
  - `module_put(name, value)`
  - `module_update(name, ...)` 或等价写法
- 值类型至少要支持：
  - `string`
  - `int`
  - `bool`
  - `list`
  - `keyword`
  - `ast`
- 明确 module-level state 与局部 `CtEnv` 的边界
- 如果 `use` 注入和 module-level registry 冲突，需要给出稳定错误

验收 examples：

- `63-module-state-accumulate-via-use`
  - 多次 `use` 同一 provider 或多个 provider
  - provider 通过 module-level state 累积注册信息
- `64-module-state-read-after-write`
  - 同一 module 中，后一个宏能读取前一个宏写入的 state
- `65-invalid-module-state-conflict`
  - 重复注册或类型不匹配时报稳定错误

完成定义：

- sl 获得“注册型 DSL”能力，而不只是立即展开型宏
- 后续 narrative DSL 能基于 compiler 内部状态做分阶段组装

## 6. 扩展 Hygiene 到隐藏 Helper 定义层

Status: pending

目标：

- 不只对 `<temp>` 做 gensym
- 把 `use` 或普通宏引入的隐藏 `script / function / const / var` 也纳入系统级 hygiene
- 区分“公开注入”与“隐藏 helper”

主要代码落点：

- `crates/sl-compiler/src/semantic/expand/quote.rs`
- `crates/sl-compiler/src/semantic/expand/module_reducer.rs`
- `crates/sl-compiler/src/semantic/env.rs`

具体工作：

- 设计隐藏 helper 的显式协议
- 对隐藏 helper 名做自动 gensym 或等价的 hygienic rename
- 公开注入成员继续保留严格冲突检测
- provider/caller 冲突错误不能再依赖 source_name 拼字符串判断

验收 examples：

- `66-use-hidden-script-gensym`
  - provider 注入隐藏 script，caller 自己定义同名前缀 helper
  - 两者不冲突
- `67-use-hidden-function-gensym`
  - provider 注入隐藏 function，不污染 caller
- `68-invalid-public-inject-conflict-reports-provider`
  - provider 注入公开成员与 caller 冲突
  - 错误文本必须明确 provider module / caller module / 成员名

完成定义：

- “隐藏 helper 靠手写 `__internal__` 命名规约”不再是主方案
- `use` 的注入边界可控、可预测

## 7. 把 Compile-Time Language 提升成可承载 Narrative DSL 宏库的子语言

Status: pending

目标：

- 不追求把 compile-time language 做成第二个 Elixir
- 但必须让它具备足够的组合能力，使后续 narrative DSL 宏不需要不停向 compiler 加 builtin

主要代码落点：

- `crates/sl-compiler/src/semantic/macro_lang/ast.rs`
- `crates/sl-compiler/src/semantic/macro_lang/eval.rs`
- `crates/sl-compiler/src/semantic/macro_lang/builtins.rs`
- 如有必要，新增 compile-time stdlib 相关模块

具体工作：

- 不再只写“增强组合能力”，而要明确补这些能力：
  - 对 `list / keyword` 的遍历
  - 对 `CtValue` 的匹配分派
  - 对 `ast` 的批量构造与组合
- 优先补 narrative DSL 场景最需要的 compile-time 能力：
  - `for_each` / `map` / `fold` 或等价 builtin
  - `match` / `case` 风格 builtin
  - 基于 compile-time list 批量生成 script / choice / text 结构
  - 组合多个 provider 的宏，而不是每个 provider 单打独斗
- 保持 runtime 不变，能力全部留在 compiler / 标准库宏层

验收 examples：

- `69-macro-iterate-over-keyword-opts`
  - 宏遍历 keyword opts 并生成多个 text/script 片段
- `70-macro-generate-multiple-scripts-from-list`
  - compile-time list 驱动批量 script 生成
- `71-macro-match-on-compile-time-values`
  - 宏对 bool/int/string/keyword 做 compile-time 匹配分支
- `72-macro-compose-use-provider`
  - 一个 provider 的 `__using__` 内部安全地组合另一个 provider 的宏

完成定义：

- narrative DSL 宏库开始具备真正的“可组合编译期编程”能力
- 不需要每遇到一个新场景就去 compiler 里再塞一条特判 builtin

## 明确暂不追求的内容

当前阶段不要发散去做这些：

- 完整复制 Elixir 的全部 `Macro.Env` 字段和语义
- 复制 Elixir 的通用宿主语言 compile-time 执行模型
- BEAM 相关机制，如 behaviours / protocols / module attributes 全量映射
- 为了实现宏能力而给 runtime 增加新 primitive
- 单独重开一条“递归展开框架重写”主线
- 现在就追“宏作为一等值”或“表达式级 quote”这类更激进的元编程能力

## 完成定义

只有当以下条件同时满足，这个阶段才算完成：

- 远程宏调用是严格的 module-qualified dispatch
- compile-time value / quote / remote invoke 模型统一
- AST 是一等 compile-time 数据
- module-level compile-time accumulation 已可用于 DSL 注册模式
- caller env 和错误定位足够支撑真实宏库开发
- hygiene 扩展到隐藏 helper 定义层
- compile-time language 已能承载下一阶段 narrative DSL 宏库
- 所有步骤对应的 examples 和单元测试都已补齐
- `make gate` 通过
- `IMPLEMENTATION.md` 已同步到当前真实状态

## 进度记录

### Step 1: 真正的 Module-Qualified Remote Macro Dispatch (2026-03-23)

**本次做了什么：**
- 新增 `ProgramState::resolve_macro_in(target_module, name)` API，严格在目标 module 中查找宏
- `builtin_invoke_macro` 改用 `resolve_macro_in` 替代 `resolve_macro`，消除 fallback 到当前 module / imports / kernel 的误命中风险
- 新增 `ExpandEnv::use_provider_module` 字段，`expand_macro_hook` 在展开 `kernel.use` 时设置
- `check_use_conflict` 使用 `use_provider_module` 报告准确的 provider 信息（之前显示 `<unknown>`）
- `builtin_invoke_macro` 错误模型改为三层：
  1. Module not known（不在 `module_macros` 中）
  2. Module not in scope（存在但未 require）
  3. Macro not defined in module（module 存在但没有该宏）
- 更新单元测试期望值以匹配新错误消息
- 集成测试 51/52/53 验收通过

**本次发现的问题、踩的坑：**
- `begin_module` 只设置 `env.module`（当前 in-progress module），不写入 `program.module_macros`；后者只在 `register_macro` 时写入；测试需要用 `register_module_for_test` 辅助方法
- `<goto script="a_script">` 中的 `script` 属性是 Rhai 表达式，会把 `a_script` 当变量查找；需用 `<const type="script">@a_script</const>` 声明 const 后再引用
- `<const type="string">from a</const>` 的值被 `const_eval` 解析为 reference path（不识别空格）；正确写法是 `<const type="string">"from a"</const>`
- `module="a"` 在 `convert.rs` 中因 `"a"` 是 alphanumeric 被当作变量引用（`CtExpr::Var`）；实际值在 `opts` keyword 中，需要通过 `string:module_name` 参数传递

**下一步方向：**
- Step 2: 统一 CtValue / MacroValue / quote / invoke_macro 值模型
- 关键点：确保 `List` / `Keyword` / `Ast` / `ModuleRef` 跨宏边界流动时不退化为字符串占位

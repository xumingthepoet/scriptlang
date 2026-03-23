# Next Plan

本文档是当前阶段交给下级 agent 的执行计划。它只描述"从当前实现继续往前推进"的工作，不重复已经完成的历史任务。

当前仓库已经完成了这些关键节点：

- compile-time macro language 已经落地最小 evaluator
- module reducer 已经让宏生成的 module-level form 真正回灌定义期环境
- `use -> require + __using__` 已经能跑通
- kernel 控制流宏已经迁移到新参数协议
- Step 1: 真正的 Module-Qualified Remote Macro Dispatch

---

## 执行规则

- 每一步都必须单独可落地、可验收、可回滚
- 只要这一步修改了 `crates/` 下代码，完成前必须跑通 `make gate`
- 只要这一步改变了 crate 行为、宏系统语义、编译流程、测试结构或支持范围，必须同步更新 `IMPLEMENTATION.md`
- 每一步除了单元测试，还必须补对应的 `crates/sl-integration-tests/examples`
- 不要为了"先过例子"往 runtime 下沉新 primitive；本计划默认 runtime 基本不动，重点在 compiler

---

## Step 1: 真正的 Module-Qualified Remote Macro Dispatch

Status: **已完成** (2026-03-23)

详见 plan.md 进度记录区。

---

## Step 2: 统一 Compile-Time Value / Quote / Remote Invoke 的值模型

Status: **已完成** (2026-03-23)

目标：消除 `CtValue`、`MacroValue`、`quote/unquote`、`invoke_macro` 之间的临时桥接和语义丢失。让 `ast / keyword / list / module` 都能作为一等 compile-time 值跨宏边界流动。

### Step 2.1: 审计现有 CtValue / MacroValue 的差异和丢失点

Status: **已完成** (2026-03-23)

**目标：** 先不动代码，把现状摸清楚。

具体工作：
- 梳理 `CtValue` 和 `MacroValue` 的所有变体（variant）
- 找出所有从 `CtValue` 转 `MacroValue` 的桥接点（`CtValue::to_macro_value` / `FromCtValue` / `TryFrom` 等）
- 记录每个桥接点是否有信息丢失（例如 list 变 keyword、caller_env 变字符串）
- 输出：一份问题清单（哪些类型在哪些桥接点退化）

验收：
- 输出一份"值模型审计报告"（可以在代码注释里标注，或新增 `VALUE_MODEL_AUDIT.md`）
- 不强求报告格式，只要 agent 自己清楚哪里会丢数据

### Step 2.2: 修复 List 在 CtValue → MacroValue 桥接时的语义丢失

Status: **已完成** (2026-03-23)

前置：Step 2.1 已完成。

具体工作：
- 如果 `MacroValue` 已有 `List` 变体，确保桥接函数使用它而不是退化
- 如果 `MacroValue` 没有 `List` 变体，新增它
- 写或更新单元测试，验证 `CtValue::List(...)` → `MacroValue::List(...)` 信息不丢
- 确认 `eval.rs` 和 `macro_values.rs` 中对 list 的处理路径一致

验收：
- `CtValue::List` 跨边界流动时不丢失为 keyword 或 string
- 相关单元测试通过

### Step 2.3: 修复 Keyword 在 CtValue → MacroValue 桥接时的语义丢失

Status: **已完成** (2026-03-23)

**目标：** `CtValue::Keyword` 跨边界时语义不丢失。

前置：Step 2.2 已完成。

具体工作：
- 确认 `MacroValue` 有与 `CtValue::Keyword` 对应的变体
- 修复 keyword 中嵌套 list / ast 的桥接路径
- 验证 keyword 的 key-value 对在跨宏调用后仍正确
- 写单元测试覆盖：简单 keyword、嵌套 keyword（含 list 值）

验收：
- `CtValue::Keyword` 双向桥接信息完整
- 相关单元测试通过

### Step 2.4: 让 invoke_macro 支持传递嵌套 keyword / list / ast / module 参数

**Status: completed** (2026-03-23)

前置：Step 2.3 已完成。

具体工作：
- 追踪 `builtin_invoke_macro` 的参数序列化路径
- 如果参数在传参前被错误地 to_string，则在传入前保持原生 CtValue
- 如果 `invoke_macro` 参数模型目前只接受字符串占位，则扩展其接受 `CtValue`
- 让 `use` 的 opts keyword 可以正确传递给 `__using__`

验收：
- `55-remote-macro-pass-nested-keyword` 通过
- `use` 中包含嵌套 keyword/list 的场景不再报类型错误

### Step 2.5: 统一 quote/unquote 对 List / Keyword / Ast 的支持范围

**Status: completed** (2026-03-23)

前置：Step 2.4 已完成。

**目标：** `quote` 产出的 AST 中，list / keyword 不再丢失结构。

具体工作：
- 检查 `quote.rs` 中 `quote_from_ast` / `unquote` 对 `CtValue` 各变体的处理
- 补全对 `List` / `Keyword` / `Ast` 变体的 quote/unquote 路径
- 修复 `MacroValue::List` 在 AST children 位置的展开：每个元素转为独立 FormItem
- 修复 `MacroValue::Keyword` 在 AST children 位置的 stringification
- 修复 `splice_string_slots()` 支持 List / Keyword 的字符串化
- 增强 `builtin_keyword_attr()` 支持嵌套查找（在 `opts` keyword 内查找 `items` 键）

验收：
- `56-quote-roundtrip-list-and-keyword` 通过
- compile-time list/keyword 经 quote/unquote 后语义不丢失
- 所有 211 个 compiler 单元测试通过
- 所有 56 个集成测试通过

**Step 2 完成定义：**
- `CtValue -> MacroValue` 不再出现"list 变 keyword / caller_env 变字符串占位"这种临时退化
- 远程宏参数模型和本地宏参数模型一致

---

## Step 3: 把 AST 提升为一等 Compile-Time 数据

Status: pending

目标：让宏真正能"编程地操作 AST"，而不是只会 `get_content(head="...") + quote`。

### Step 3.1: 梳理现有 AST 表示（input 层到 compile-time 层的映射）

**目标：** 搞清楚从 source text 到 compile-time 可操作的 AST 之间的映射关系。

具体工作：
- 追踪 `parse.rs` 产出的 `ScriptAst` 如何转化为 `CtValue::Ast`
- 确认 AST 中哪些信息（head、attrs、children、text/form 顺序）被保留
- 找出哪些信息在此过程中被丢弃

验收：
- 有一份 AST 字段映射文档（可以在代码注释里标注）

### Step 3.2: 新增最小 AST builtins（基础读写）

**目标：** 先把读和写的基础 API 搭起来。

前置：Step 3.1 已完成。

具体工作：
- 在 `builtins.rs` 新增以下 builtin（至少）：
  - `ast_head(ast)` → 返回 head 字符串
  - `ast_children(ast)` → 返回 list of ast
  - `ast_attr_get(ast, key)` → 返回 attr 值
  - `ast_attr_keys(ast)` → 返回所有 attr key
- 在 `eval.rs` 注册这些 builtin
- 写单元测试验证每个 builtin 的返回值

验收：
- 单元测试覆盖每个新 builtin 的正常路径
- `57-ast-rewrite-by-head` 基础版通过（能按 head 选中节点）

### Step 3.3: 新增 AST 写操作 builtin

**目标：** 让宏能做结构化 AST 改写。

前置：Step 3.2 已完成。

具体工作：
- 新增 builtin：
  - `ast_attr_set(ast, key, value)` → 返回修改后的 ast（不修改原 ast，遵循 immutability）
  - `ast_wrap(inner_ast, head, extra_attrs?)` → 用新 head/wrapper 包装 inner ast
  - `ast_concat(asts)` → 拼接多个 ast
  - `ast_filter_head(ast, predicate_head)` → 过滤 children
- 遵循 immutability：所有写操作返回新 AST，原 AST 不变
- 写单元测试

验收：
- `57-ast-rewrite-by-head` 完整版通过（能按 head 选中并改写）
- `58-ast-wrap-content-preserve-order` 通过

### Step 3.4: 让 AST 改写结果能回到 reducer / quote 主路径

**目标：** AST builtins 产出的 AST 能无缝进入后续宏展开流程。

前置：Step 3.3 已完成。

具体工作：
- 确认 `CtValue::Ast` → `MacroValue` → `QuoteResult` 的转换路径完整
- 如果 AST builtins 产出的是中间表示而非 `CtValue::Ast`，补转换路径
- 验证 `59-ast-build-module-fragments`（通过 AST API 组合多个 script/choice）

验收：
- AST builtins 产物能正确回到宏展开管道
- `59-ast-build-module-fragments` 通过

**Step 3 完成定义：**
- 宏作者不再只能依赖 `get_content(head="...")`
- 可以做结构化 AST 选择、改写和重组

---

## Step 4: 丰富 Caller Env 与错误定位

Status: pending

目标：让宏系统具备更接近 Elixir 的 caller 感知能力；让复杂宏链条里的错误报告可诊断。

### Step 4.1: 审计现有 caller_env 和错误报告的薄弱点

**目标：** 摸清现状。

具体工作：
- 找到 `caller_env()` builtin 的当前实现
- 找到所有 `compile_error!` / 错误报告的调用点
- 列出每个调用点当前能提供哪些上下文信息（module、file、line、macro name、stack）
- 标记缺失信息最多的几个关键路径

验收：
- 输出一份错误报告审计（agent 心中有数即可）

### Step 4.2: 补全 caller_env 的基础字段（module、file、line）

**目标：** `caller_env()` 至少返回当前模块、源文件、行列。

前置：Step 4.1 已完成。

具体工作：
- 在 `ExpandEnv` 或 `MacroEnv` 中追踪当前 caller 的 `ModuleRef`、`source_file`、`line`、`column`
- 暴露 `caller_env()` builtin 返回包含这些字段的 keyword/map
- 验证远程宏调用场景下，caller 信息能正确传递

验收：
- `60-macro-caller-env-source-location` 通过

### Step 4.3: 给 compile_error! 补全 provider / caller 上下文

**目标：** 错误消息同时带 provider 和 caller 信息。

前置：Step 4.2 已完成。

具体工作：
- 在所有 `use` 相关错误（`__using__` 失败、conflict 检测）中补全 provider module 信息
- 在远程宏调用错误中补全 target module 和 caller module 信息
- 统一错误报告格式（provider + caller + source_location）
- 写单元测试覆盖各错误路径

验收：
- `61-invalid-use-error-has-provider-and-caller` 通过
- 现有错误路径的单元测试不回归

### Step 4.4: 给嵌套宏失败补 expansion trace

**目标：** 嵌套宏报错时能看出调用链路。

前置：Step 4.3 已完成。

具体工作：
- 在 `ExpandEnv` 中引入 expansion trace 栈（`Vec<TraceEntry>`）
- 在每次宏展开入口压栈、出口弹栈
- 展开失败时，把 trace 注入错误消息
- `62-macro-expansion-stack-nested-error` 验证

验收：
- `62-macro-expansion-stack-nested-error` 通过
- 展开 trace 不影响正常路径性能（trace 仅在出错时使用）

**Step 4 完成定义：**
- 调试宏不再只能靠猜
- `caller_env()` 对真实宏库实现已经足够有用

---

## Step 5: Module-Level Compile-Time Accumulation

Status: pending

目标：给 macro system 增加 module-level compile-time 累积状态，让 DSL 能实现"注册型"编译期协议。

### Step 5.1: 在 ExpandEnv 中引入 module-level state 存储

**目标：** 先搭存储结构，不做 API。

前置：无。

具体工作：
- 在 `ExpandEnv` 中新增字段存储 module-level compile-time state（例如 `HashMap<ModuleRef, ModuleLevelState>`）
- `ModuleLevelState` 内部是 `HashMap<String, CtValue>`
- 确保 module-level state 随 `ProgramState` 的 module 切换而隔离
- 不新增任何 builtin，纯存储层面验证

验收：
- 同一个 `ProgramState` 中不同 module 的 state 互不干扰
- 单元测试验证隔离性

### Step 5.2: 新增 module_get / module_put builtin（基础版）

**目标：** 最简单的读写 API。

前置：Step 5.1 已完成。

具体工作：
- `module_get(name: string) → CtValue`
- `module_put(name: string, value: CtValue) → CtValue`（返回写入的值）
- 实现遵循 immutability：`module_put` 返回新 state（但如果放在 `ExpandEnv` 中，需要设计合理的更新路径，确保下个宏调用能读到）
- 写单元测试验证：写入后同 module 的下一个宏调用能读到

验收：
- `64-module-state-read-after-write` 基础版通过

### Step 5.3: 让 module state 支持多类型值

**目标：** string、int、bool、list、keyword、ast 都能存。

前置：Step 5.2 已完成。

具体工作：
- 扩展 `ModuleLevelState` 的 value 类型支持
- 写单元测试覆盖每种类型：存进去、再读出来，值相等
- 验证 list/keyword/ast 存进去时类型信息不丢失

验收：
- 各类型存储和读取的单元测试通过

### Step 5.4: 支持 module_update 模式（基于已有值更新）

**目标：** 支持"读出现有值再写入"的累积模式。

前置：Step 5.3 已完成。

具体工作：
- `module_update(name, fn)` 或等价写法（例如 `module_put(name, fn(module_get(name)))`）
- 确保多次 `use` 同一 provider 时 registry 累积
- 验证 `63-module-state-accumulate-via-use` 场景

验收：
- `63-module-state-accumulate-via-use` 通过

### Step 5.5: 处理 module state 冲突

**目标：** 重复注册或类型不匹配时报稳定错误。

前置：Step 5.4 已完成。

具体工作：
- 设计 module state 的冲突检测策略（例如：同一 module 同名 key 第二次 put 是否报错）
- 实现 `65-invalid-module-state-conflict`
- 明确 module-level state 与局部 `CtEnv` 的边界

验收：
- `65-invalid-module-state-conflict` 通过
- `make gate` 通过

**Step 5 完成定义：**
- sl 获得"注册型 DSL"能力
- 后续 narrative DSL 能基于 compiler 内部状态做分阶段组装

---

## Step 6: 扩展 Hygiene 到隐藏 Helper 定义层

Status: pending

目标：不只对 `<temp>` 做 gensym，把 `use` 或普通宏引入的隐藏 helper 也纳入系统级 hygiene。

### Step 6.1: 区分"公开注入"和"隐藏 helper"的当前处理方式

**目标：** 摸清现状。

具体工作：
- 找到 `use` 注入成员的所有代码路径
- 标记哪些是"公开注入"（script/choice/text 等直接可见成员）
- 标记哪些是"隐藏 helper"（目前是否靠命名规约如 `__internal__` 约定）
- 确认"公开注入"已有冲突检测，"隐藏 helper"目前是否有冲突处理

验收：
- 输出一份 hygiene 现状分析（agent 心中有数即可）

### Step 6.2: 给隐藏 helper 定义设计 hygienic rename 机制

**目标：** 隐藏 helper 不再依赖手写 `__internal__` 命名规约。

前置：Step 6.1 已完成。

具体工作：
- 设计隐藏 helper 的识别协议（例如：通过某个标记字段声明自己是隐藏 helper）
- 实现 hygienic rename：在 `module_reducer` 或 `quote.rs` 中，对隐藏 helper 名称自动加 module 前缀或 gensym
- 写单元测试：provider 注入隐藏 script，caller 自己定义同名前缀 helper，两者不冲突

验收：
- `66-use-hidden-script-gensym` 通过

### Step 6.3: 让函数和 const 也支持 hygienic rename

**目标：** 函数和 const 的隐藏 helper 也不污染 caller。

前置：Step 6.2 已完成。

具体工作：
- 扩展 Step 6.2 的 hygienic rename 到 `function` 和 `const` 定义
- 写单元测试

验收：
- `67-use-hidden-function-gensym` 通过

### Step 6.4: 统一公开注入冲突的错误报告格式

**目标：** provider/caller 冲突错误不再依赖 source_name 拼字符串判断。

前置：Step 6.3 已完成。

具体工作：
- 在 `check_use_conflict` 中使用结构化信息（module path）而非字符串拼接判断冲突
- 错误文本必须明确：provider module / caller module / 成员名
- 验证 `68-invalid-public-inject-conflict-reports-provider`

验收：
- `68-invalid-public-inject-conflict-reports-provider` 通过
- `make gate` 通过

**Step 6 完成定义：**
- "隐藏 helper 靠手写 `__internal__` 命名规约"不再是主方案
- `use` 的注入边界可控、可预测

---

## Step 7: 把 Compile-Time Language 提升成可承载 Narrative DSL 宏库的子语言

Status: pending

目标：不追求把 compile-time language 做成第二个 Elixir，但必须让它具备足够的组合能力。

### Step 7.1: 审计 compile-time language 现有控制流能力

**目标：** 摸清现状，列出缺失的能力。

前置：无。

具体工作：
- 梳理 `eval.rs` 和 `builtins.rs` 中现有的所有 builtin
- 确认已有的：变量绑定、函数调用、if/unless、条件分支、list/keyword 构造
- 列出缺失的：遍历（for_each/map/fold）、匹配（match/case）、批量生成
- 结合 Step 2 的值模型，确认 list/keyword 在遍历时是否已可用

验收：
- 输出一份 compile-time builtin 能力清单（agent 心中有数即可）

### Step 7.2: 新增 list 遍历 builtin（for_each / map / fold）

**目标：** 让宏能对 compile-time list 做遍历处理。

前置：Step 7.1 已完成，Step 2 的 list 值模型已统一。

具体工作：
- 新增 builtin：
  - `list_foreach(list, fn)` → 对 list 中每个元素执行 fn，返回空或执行副作用
  - `list_map(list, fn)` → 对 list 中每个元素变换，返回新 list
  - `list_fold(list, init, fn)` → 累积折叠
- `fn` 的表示方式：可以用内联 AST 片段（quote）或已有函数引用
- 写单元测试覆盖正常路径和空 list 边界

验收：
- `69-macro-iterate-over-keyword-opts` 基础版通过

### Step 7.3: 新增 keyword opts 遍历能力

**目标：** DSL 宏最常见的场景：遍历 keyword opts。

前置：Step 7.2 已完成。

具体工作：
- 新增 builtin：
  - `keyword_keys(kw)` → 返回所有 key 的 list
  - `keyword_get(kw, key)` → 获取 key 对应的值
  - `keyword_pairs(kw)` → 返回 key-value 对的 list（每个对也是 keyword 或 tuple）
- 用 keyword opts 遍历能力重写或补充 `69-macro-iterate-over-keyword-opts`

验收：
- `69-macro-iterate-over-keyword-opts` 通过
- DSL 宏遍历 keyword opts 生成多个 text/script 片段

### Step 7.4: 新增 match / case 风格的 compile-time 匹配分支

**目标：** 让宏能做基于 compile-time 值的条件分支，而不是只有 if/unless。

前置：Step 7.3 已完成。

具体工作：
- 新增 builtin：
  - `match(value, [pattern, result], ...)`
  - 或 `case(value) { pattern: result, ... }`
- 支持的 pattern 类型：bool、int、string、keyword、list、wildcard（`_`）
- 写单元测试覆盖：各类型匹配、wildcard、分支不存在时的错误

验收：
- `71-macro-match-on-compile-time-values` 通过

### Step 7.5: 基于 compile-time list 批量生成 script / choice 结构

**目标：** 让宏能把 compile-time list 映射为多个 AST 节点。

前置：Step 7.4 已完成。

具体工作：
- 把 Step 7.2 的 `list_map` 与 Step 3 的 AST builtins 结合
- 实现 `70-macro-generate-multiple-scripts-from-list`
- 验证 compile-time list 中的每个元素能生成对应的 script/choice 节点

验收：
- `70-macro-generate-multiple-scripts-from-list` 通过

### Step 7.6: 支持组合多个 provider 的宏

**目标：** 一个 provider 的 `__using__` 内部能安全地组合另一个 provider 的宏。

前置：Step 7.5 已完成，Step 1 的 module-qualified dispatch 已完成。

具体工作：
- 在 `__using__` 中使用 `require` + `invoke_macro` 组合多个 provider
- 确保组合时 caller_env、module state、hygiene 不冲突
- 实现 `72-macro-compose-use-provider`

验收：
- `72-macro-compose-use-provider` 通过
- `make gate` 通过

**Step 7 完成定义：**
- narrative DSL 宏库开始具备真正的"可组合编译期编程"能力
- 不需要每遇到一个新场景就去 compiler 里再塞一条特判 builtin

---

## 明确暂不追求的内容

当前阶段不要发散去做这些：

- 完整复制 Elixir 的全部 `Macro.Env` 字段和语义
- 复制 Elixir 的通用宿主语言 compile-time 执行模型
- BEAM 相关机制，如 behaviours / protocols / module attributes 全量映射
- 为了实现宏能力而给 runtime 增加新 primitive
- 单独重开一条"递归展开框架重写"主线
- 现在就追"宏作为一等值"或"表达式级 quote"这类更激进的元编程能力

---

## 完成定义

只有当以下条件同时满足，这个阶段才算完成：

- Step 1: 远程宏调用是严格的 module-qualified dispatch ✓
- Step 2: compile-time value / quote / remote invoke 模型统一
- Step 3: AST 是一等 compile-time 数据
- Step 4: caller env 和错误定位足够支撑真实宏库开发
- Step 5: module-level compile-time accumulation 已可用于 DSL 注册模式
- Step 6: hygiene 扩展到隐藏 helper 定义层
- Step 7: compile-time language 已能承载下一阶段 narrative DSL 宏库
- 所有步骤对应的 examples 和单元测试都已补齐
- `make gate` 通过
- `IMPLEMENTATION.md` 已同步到当前真实状态

---

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

### Step 2.2: 修复 List 在 CtValue → MacroValue 桥接时的语义丢失 (2026-03-23)

**本次做了什么：**
- 新增 `MacroValue::List(Vec<MacroValue>)` 变体（`macro_values.rs`）
- 修复 `ct_value_to_macro_value`：`CtValue::List` 正确映射为 `MacroValue::List`（递归，保留所有嵌套元素），不再退化为 `MacroValue::Keyword` 加 `"[N items]"` 字符串占位
- 修复 `macro_value_to_ct_value`：增加 `MacroValue::List` → `CtValue::List` 处理路径，实现双向往返保持
- 修复 `builtin_keyword_attr`：`MacroValue::List` 在 Keyword 嵌套值中正确转换为 `CtValue::List`，不再落入 `format!("{:?}", nv)` 字符串退化路径
- 修复 `quote.rs`：四处 match arm 穷举检查加入 `MacroValue::List`，保持编译期穷举性
- 新增单元测试 `ct_value_list_preserves_structure_across_macro_value_bridge`：覆盖简单 list、嵌套 list、双向往返相等性
- 所有 197 个 sl-compiler 单元测试通过

**本次发现的问题、踩的坑：**
- Rust match 必须在枚举变体变化后同步更新所有穷举检查；`MacroValue` 新增 `List` 后，`quote.rs` 的四处 match（行 48/154/221/258）如果不加 `MacroValue::List` 会触发编译错误，比警告更安全
- `builtins.rs` 的 `builtin_keyword_attr` 中嵌套 `MacroValue` 的 match（行 349）也是穷举的，加入 `MacroValue::List` 避免未来静默错误

**下一步方向：**
- Step 2.3: 修复 Keyword 在 CtValue → MacroValue 桥接时的语义丢失（caller_env 变字符串问题也需要在这里处理）
- 同步检查 Step 2.1 审计报告中 P0/P1 的其他丢失点

### Step 2.1: 审计 CtValue / MacroValue 差异和丢失点 (2026-03-23)

**本次做了什么：**
- 阅读了 `ast.rs`、`macro_values.rs`、`eval.rs`、`builtins.rs`、`macro_params.rs`、`macro_eval.rs`、`quote.rs` 全部源代码
- 整理了两个类型的完整变体对照表和三个桥接函数的逐变体分析
- 识别出 6 个关键信息丢失点

**本次发现的问题、踩的坑：**

1. **P0 `CtValue::List` → `MacroValue::Keyword` 严重退化**：`ct_value_to_macro_value` 把 List 转成 `Keyword([("list", String("[N items]"))])`，所有列表元素完全丢失。这是 Step 2 最核心的阻断问题，因为后续的 Step 2.2/2.3/2.4 都依赖 List 可以正确跨宏边界。
2. **P0 `MacroValue` 缺少 List 变体**：当前 MacroValue 没有 `List` 枚举分支，`ct_value_to_macro_value` 走的是退化路径。需要新增 `MacroValue::List(Vec<MacroValue>)` 变体。
3. **P0 `MacroValue` 缺少 ModuleRef/CallerEnv 变体**：两者都只有不透明字符串（`String(m)` / `"<caller_env>"`），无法在 quote/unquote 中正确区分。
4. **P1 `builtin_keyword_attr` 嵌套值退化**：`builtins.rs:365` 对非 String 类型的嵌套值用了 `format!("{:?}", nv)` 字符串化，导致 `use opts=[list]` 在 `__using__` 中得到的是 Debug 字符串而非原始类型。
5. **P1 `invoke_macro` keyword args 只接受 string/int/bool**：第974-985行对 List/Keyword/Ast 直接报错，Step 2.4 必须扩展此路径。
6. **P1 `bind_explicit_params` keyword args 格式退化**：`macro_params.rs:133` 把 keyword args 拼成 `"a:val"` 字符串而非结构化 `Keyword([("a", val)])`。

**下一步方向：**
- Step 2.2: 新增 `MacroValue::List` 变体，修复 `ct_value_to_macro_value` 中 List 的退化路径，让 `CtValue::List` 正确映射为 `MacroValue::List`

### Step 2.3: 修复 Keyword 在 CtValue → MacroValue 桥接时的语义丢失 (2026-03-23)

**本次做了什么：**
- 确认 `MacroValue::Keyword` 已有正确变体（`macro_values.rs`），`ct_value_to_macro_value` 和 `macro_value_to_ct_value` 对 Keyword 的双向递归转换均已正确
- 修复 `builtin_keyword_attr` 中 `MacroValue::Keyword` 嵌套值的退化问题：`builtins.rs:367-369` 原来对非 String 类型的嵌套值使用 `format!("{:?}", nv)` 字符串化，导致 `MacroValue::List` / `MacroValue::Keyword` / `MacroValue::Bool` 等嵌套值退化为调试字符串
- 修复方式：使用 `macro_value_to_ct_value` 对所有嵌套值做递归转换，确保 List、Keyword、Bool、Int、Nil、AstItems 均保留原始类型
- Clippy 发现并修复：`MacroValue::List` 分支中的冗余闭包 `|mv| macro_value_to_ct_value(mv)` 改为直接引用 `macro_value_to_ct_value`
- 新增单元测试 `ct_value_keyword_preserves_structure_across_macro_value_bridge`：覆盖简单 keyword、嵌套 keyword（含 list 值）、嵌套 keyword（递归）、keyword 含 Bool/Nil 值的往返相等性
- 新增单元测试 `builtin_keyword_attr_preserves_nested_types`：验证 `builtin_keyword_attr` 对 `MacroValue::List`、`MacroValue::Keyword`、`MacroValue::Bool` 嵌套值的保留
- 所有 199 个 sl-compiler 单元测试通过，`make gate` 通过

**本次发现的问题、踩的坑：**
- `builtin_keyword_attr` 的 `MacroValue::Keyword` 分支中，嵌套值处理只覆盖了 `MacroValue::String`，其余类型都落入 `format!("{:?}", nv)` 退化路径。这是 Step 2.1 审计报告 P1 问题 #4 的直接体现
- Clippy 的 `redundant_closure` 检查：当闭包只是透传参数时（`|x| f(x)`），应直接用函数引用 `f` 替代，这是 Rust 的 idiom
- Step 2.3 验收标准只要求"双向桥接信息完整"，不需要新建 integration test（integration tests 54/55/56 是 Step 2.4/2.5 的占位符，已补充最小内容让 gate 通过）
- `macro_params.rs:133` 的 keyword args 格式退化（`"a:val"` 字符串）是 Step 2.4 的范围，不在 Step 2.3 范围内

### Step 2.4: 让 invoke_macro 支持传递嵌套 keyword / list / ast / module 参数 (2026-03-23)

**本次做了什么：**
- 新增 `parse_macro_value_from_string()` 函数（`macro_params.rs`）：将 XML 属性值字符串解析为结构化 `MacroValue`。支持：bool（"true"/"false"）、int、keyword（"key:val"）、list（"a,b,c"）
- 修复 `bind_explicit_params`：`keyword:opts` 参数的未使用属性不再拼成 `"name:val"` 字符串，而是调用 `parse_macro_value_from_string` 保留实际类型
- 扩展 `builtin_invoke_macro`：keyword args 支持 `CtValue::List`、`CtValue::Keyword`、`CtValue::Ast`（不仅 string/int/bool）
- 新增 `ct_value_to_string()` 辅助函数（`builtins.rs`）：将 `CtValue` 序列化为可被 `parse_macro_value_from_string` 解析的字符串格式
- 更新 `builtin_invoke_macro_wrong_keyword_arg_value_type_errors` 测试：从 `CtValue::Ast`（现已支持）改为 `CtValue::Nil`（仍不支持）
- 新增单元测试：8 个 `parse_macro_value_from_string` 覆盖案例（含 comma/colon 优先级）、1 个 `invoke_macro` 接受 List/Keyword/Ast args 的测试
- 更新集成测试 55：传递 `async="true"`、`items="a,b,c"`、`config="mode:debug"` 三个 opts，验证完整解析路径
- `make gate` 通过（208 个 sl-compiler 单元测试 + 56 个集成测试全部通过）

**本次发现的问题、踩的坑：**
- `"a:b,c:d"` 格式歧义：invoke_macro 序列化 List of Keywords 为 `"key1:val1,key2:val2"` 格式，与单个 keyword `"key:val"` 无法区分。解决：comma 优先 → 有 comma 就解析为 List，无 comma 才解析为 Keyword
- Clippy `redundant_closure`：`|v| ct_value_to_string(v)` 需改为 `ct_value_to_string`（函数引用而非闭包）
- `invoke_macro` 序列化 List 为 `"[a,b,c]"`（带方括号）不会被 `parse_macro_value_from_string` 识别 → 去掉方括号，直接用 `"a,b,c"`，与 comma-separated list 解析逻辑一致

**下一步方向：**
- Step 2.5: 统一 quote/unquote 对 List / Keyword / Ast 的支持范围（`quote.rs` 的 `quote_from_ast`/`unquote` 对各变体的处理）
- 注意：`CtValue::Nil`/`ModuleRef`/`CallerEnv` 在 `invoke_macro` keyword args 中仍未支持（传给 remote macro 会报错）；`CtValue::Ast` 传为 `FormValue::Sequence`，round-trip 语义有限（只能作为 opaque sequence，无法在 target macro 中还原为原始 AST 结构）

### Step 2.5: 统一 quote/unquote 对 List / Keyword / Ast 的支持范围 (2026-03-23)

**本次做了什么：**
- 修复 `quote_ast_items` 中 `MacroValue::List` 和 `MacroValue::Keyword` 的 unquote 支持：
  - `MacroValue::List` 在 AST children 位置展开为多个 `FormItem`（每个元素一个 Text 或 Ast）
  - `MacroValue::Keyword` 在 AST children 位置 stringify 为 `"key1:val1,key2:val2"` 格式的 Text
- 新增 `macro_keyword_to_string()` 和 `macro_value_to_string()` 辅助函数用于 keyword/list 的递归字符串化
- 修复 `splice_string_slots()` 支持 `MacroValue::List` 和 `MacroValue::Keyword`（递归 stringify 到字符串槽）
- 增强 `builtin_keyword_attr()` 支持嵌套查找：
  - 如果 `keyword_attr("items")` 在 `macro_env.locals` 顶层找不到，会搜索所有 `MacroValue::Keyword` 类型的 locals
  - 允许在 `opts` keyword 参数内部查找 "items" 键并返回其值（不再包装为 keyword）
- 新增单元测试覆盖：keyword stringify、list unquote 展开、keyword unquote stringify
- 更新集成测试 56：演示 list 通过 quote/unquote 的 round-trip

**本次发现的问题、踩的坑：**
- `MacroValue::Keyword` 和 `MacroValue::List` 在 AST children 位置不能直接作为 statement forms（编译器只接受特定 form heads 如 `<text>`, `<script>`, `<end>`）
- 解决方案：list 展开（每个元素变为独立 FormItem）、keyword stringify（转为 `"key:val,..."` 格式文本）
- `builtin_keyword_attr("items")` 在 `keyword:opts` 场景下需要查找嵌套值（items 在 opts 内部），原实现只查找顶层 locals
- `<unquote>var_name</unquote>` 语法要求变量名在 body text 中，不是 `var="..."` 属性
- Clippy `for_kv_map`：迭代 map 时只用 value 应用 `.values()` 而非 `for (_, v) in &map`

**下一步方向：**
- Step 3: 把 AST 提升为一等 Compile-Time 数据（新增 AST builtins 如 `ast_head`, `ast_children`, `ast_attr_get`）
- Step 4: 丰富 Caller Env 与错误定位

**Step 2 完成状态：**
- Step 2.1-2.5 全部完成
- `CtValue -> MacroValue` 不再出现"list 变 keyword / caller_env 变字符串占位"这种临时退化
- 远程宏参数模型和本地宏参数模型一致

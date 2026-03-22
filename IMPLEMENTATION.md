# Current Implementation

本文档只描述当前代码库中已经落地的实现，不描述长期目标。长期架构原则仍以 `AGENTS.md` 为准。

## Workspace Layout

当前项目已经拆成多 crate workspace：

- `sl-core`
  - 放共享核心类型
  - 包括错误类型、parser 产物类型、编译产物类型、IR、runtime step 结果、snapshot
  - 不依赖任何其他本地 crate
- `sl-parser`
  - 负责 `XML -> Form`
  - 会在 XML parse 前拒绝 `&quot;`、`&lt;`、`&amp;`、`&#...;` 这类实体转义写法
  - 只依赖 `sl-core`
- `sl-compiler`
  - 负责 `Form -> CompiledArtifact`
  - 只依赖 `sl-core`
- `sl-runtime`
  - 负责执行 `CompiledArtifact`
  - 只依赖 `sl-core`
- `sl-api`
  - 负责组合 parser / compiler / runtime
  - 自动把 crate 内置 `lib/*.xml` 加入高层编译入口
  - 提供较方便的一体化入口
- `sl-repl`
  - 提供类似 IEx 的真实执行型 REPL
  - 默认只加载 `kernel`，并维护隐藏 session module / script
  - 支持按单个 XML fragment 编译执行、`:load PATH`、choice 恢复和中间产物 inspect
- `sl-integration-tests`
  - 独立的集成测试 crate
  - 通过 `sl-api` 驱动例子用例

根 crate `scriptlang-new` 当前主要做 re-export，方便外部统一使用。

## Current Language Scope

当前实现支持的 XML 子集：

- `<module>`
- `<import>`
- `<require>`
- `<alias>`
- `<macro>`
- `private="true"` attribute on module-level `<const>`, `<var>`, `<script>`, `<function>`
- `<script>`
- `<function>`
- `<var>`
- `<const>`
- `<temp>`
- `<if when="">`
- `<while when="">`
- `<break>`
- `<continue>`
- `<code>`
- `<text>`
- `<choice>`
- `<goto>`
- `<end>`

当前仅在 macro body 中支持的 compile-time XML 子集：

- `<let>`
- `<get-attribute>`
- `<get-content>`
- `<quote>`
- `<unquote>`

当前 quote 字符串槽位还支持 `${local_name}` 形式的 compile-time splice：

- 用于普通字符串属性
- 用于普通文本节点
- 用于 expr 属性

当前明确不支持：

- `<else>`
- `<call>`
- `<return>`

当前语义约束：

- `<if>` 只有单分支，没有 `else`
- 表层 `<if>` 当前完全由 `kernel` 宏提供，并展开成基于 non-capturing `<while>` 的单次执行结构；compiler 不再保留 builtin `if` 语义节点
- `<while>` 当前已支持 `break` / `continue`
- `<goto script="">` 现在是表达式槽位，运行时要求其结果为 script key 字符串
- `<import>`、`<require>`、`<alias>` 只能出现在 `<module>` 下，并按源码顺序向后影响当前 module 的编译期上下文
- `private="true"` 目前只影响 module 边界导出；同一 module 内仍可直接引用 private const / var / script / function
- `@main.loop` / `@loop` 是 script 字面量；`@loop` 会在编译期展开为当前 module 下的完整 script key
- `#main.pick` / `#pick` 是 function 字面量；`#pick` 会在编译期展开为当前 module 下的完整 function key
- `var / temp / const` 的 `type="..."` 现在是必填
- 当前 MVP 识别的显式类型有 `int / bool / string / script / function / array / object`
- `<function>` 当前支持 `args="type:name,..."` 和 `return_type="type"`；body 是一段 Rhai `code` 风格源码，并通过 `return ...;` 产出结果
- function 当前只能在 expr / code 路径里被调用，不引入新的 script-level call primitive
- 直接函数调用当前支持 `main.run(x)` / `run(x)` 这类 expr 内调用；编译期会重写成统一的 builtin 调用
- 当前也支持 `invoke(fn_var, [args])`；首参要求是 `function` 字符串值
- runtime 不保留 module 概念，只按 `script_id + pc` 执行

## Parser / Compiler / Runtime

### Parser

`sl-parser` 负责：

- 读取 XML
- 校验根节点必须为 `<module>`
- 另外也提供单根 fragment 解析入口，供 `sl-repl` 接收 `<text>` / `<if>` / `<module>` 这类单条交互输入
- 生成宿主无关的编译前表示 `Form { head, meta, fields }`
- 保留属性顺序，并在 `fields` 末尾固定追加 `children`
- 在 `children` 中递归保留文本项和子 form 的顺序

parser 不再承担 MVP 标签白名单和语义下沉；它当前只负责把 XML 结构化成可供宏和编译层消费的宿主无关前表示。

另外，parser 当前会在 XML parse 之前直接拒绝实体转义写法：

- `&quot;`
- `&apos;`
- `&lt;`
- `&gt;`
- `&amp;`
- `&#...;` / `&#x...;`

当前约定是不允许用户在 XML 源里写这些转义实体；expr 里的比较/逻辑应走 ScriptLang 自己的 expr 规则，而不是回退到 XML 实体。

### Compiler

`sl-compiler` 负责：

- 以显式 pipeline 执行编译：
  - `Form -> semantic expand`
  - `expand` 直接消费 raw `Form`，顺序推进定义期状态，并把 module children / exports / imports / requires / aliases / const declarations / macro definitions 沉淀到 `ProgramState`
  - `expand` 是当前唯一的前端语义入口；其内部通过 `ExpandEnv`、`ExpandRegistry` 和 `semantic/expand/*` 子模块完成定义期状态推进、macro 分派、名称解析和结构降解
  - `semantic program -> runtime IR`
  - 对外还公开了分段 inspect 入口：
    - `expand_to_semantic`
    - `assemble_semantic_program`
    - `compile_pipeline`
  - 当前还支持 `CompileOptions { default_entry_script_ref }`
  - 无 options 的兼容入口仍默认以 `main.main` 作为默认入口
- 源码目录当前按阶段分成：
  - 顶层 `pipeline.rs`
  - `semantic/`：名称解析、`<const>` 编译期求值、文本模板解析和语义下沉；当前包含 `env.rs`、`form.rs`、`expand/`、`expr/` 和 `types.rs`
  - `semantic/expand/`：承载 builtin/module macro expansion、module/import definition-time state、module catalog、scope resolution、const evaluation 和 script lowering analysis
  - `assemble/`：声明收集、lowering、boot script、`CompiledArtifact` 装配
- `semantic/form.rs` 当前统一承载 raw `Form` 的属性、body、children 和错误定位 helper；旧 `classify.rs` 已删除
- `expand` 入口会直接对 raw `Form` 做 module / import / require / alias / const / var / script / local temp 的顺序遍历和定义期状态维护；`ExpandEnv` 会累计整份程序的 module 状态快照，包括 module order、children、exports、imports、requires、aliases、const declarations 和 macro definitions
- `semantic/expand/` 当前已经按职责拆分：
  - [`dispatch.rs`](/Users/xuming/work/scriptlang-new/crates/sl-compiler/src/semantic/expand/dispatch.rs)：统一 expand 分派入口，负责 builtin / macro hook 路由
  - [`imports.rs`](/Users/xuming/work/scriptlang-new/crates/sl-compiler/src/semantic/expand/imports.rs)：`import` / `require` / `alias` 目标校验
  - [`macro_env.rs`](/Users/xuming/work/scriptlang-new/crates/sl-compiler/src/semantic/expand/macro_env.rs)：显式 `MacroEnv`，承载 current module、imports、requires、aliases、attributes、content、locals 和 gensym 状态
  - [`macro_values.rs`](/Users/xuming/work/scriptlang-new/crates/sl-compiler/src/semantic/expand/macro_values.rs)：compile-time `MacroValue`
  - [`macros.rs`](/Users/xuming/work/scriptlang-new/crates/sl-compiler/src/semantic/expand/macros.rs)：macro 定义收集、可见性查找和模板式宏展开
  - [`quote.rs`](/Users/xuming/work/scriptlang-new/crates/sl-compiler/src/semantic/expand/quote.rs)：`quote / unquote`、AST splice 和最小 hygiene
  - [`modules.rs`](/Users/xuming/work/scriptlang-new/crates/sl-compiler/src/semantic/expand/modules.rs)：module catalog 与 script / function 字面量查找
  - [`scope.rs`](/Users/xuming/work/scriptlang-new/crates/sl-compiler/src/semantic/expand/scope.rs)：module scope、const catalog 和 var/const 解析
  - [`program.rs`](/Users/xuming/work/scriptlang-new/crates/sl-compiler/src/semantic/expand/program.rs)：program/module 级语义总调度
  - [`scripts.rs`](/Users/xuming/work/scriptlang-new/crates/sl-compiler/src/semantic/expand/scripts.rs)：script body 和 statement lowering
  - [`declared_types.rs`](/Users/xuming/work/scriptlang-new/crates/sl-compiler/src/semantic/expand/declared_types.rs)：声明类型解析与 `<const>` 声明注册
  - [`const_eval.rs`](/Users/xuming/work/scriptlang-new/crates/sl-compiler/src/semantic/expand/const_eval.rs)：builtin 常量求值与常量替换
- `semantic/expr/` 统一承载 expr 前端处理；`script literal` 会先经过统一 token 扫描，模板 `${...}` 的洞会先落到 `ExprSource` 外壳后再回到当前 `TextTemplate` 主路径
- `semantic/expr/` 当前还统一负责 expr 记号预处理：
  - 把 `LT / LTE / AND` 规范化成 Rhai 对应操作符
  - 把单引号字符串统一转成 Rhai 的双引号字符串表示
  - 这层只按“expr 字符串”工作，不按 `when` / `goto` / `${...}` / `<var>` 等具体槽位分散实现
  - 普通 expr body 和模板 `${...}` 洞共享同一套预处理入口
  - 预处理完成后，compiler 会使用最小 Rhai compile 入口先生成 AST，再从 AST 做 free-variable analysis
- `assemble/lowering.rs` 当前把所有落到 runtime 的 expr / code / function body 统一收口成 `CompiledExpr { source, referenced_vars }`
  - `source` 是最终 lower 后交给 runtime 的源码字符串
  - `referenced_vars` 现在由 compiler 侧的 Rhai AST free-variable analysis 提取，不再依赖手写字符串扫描
  - 这层统一覆盖 `EvalGlobalInit / EvalTemp / EvalCond / ExecCode / JumpScriptExpr / CompiledFunction.body / CompiledTextPart::Expr`
  - 文本模板里的纯变量 `${name}` 现在由 AST 的简单变量节点识别，并进一步 lower 成确定性的 `CompiledTextPart::VarRef(name)`，不再走 Rhai eval
- builtin form 的 expand 处理当前已收敛到 [`semantic/expand/dispatch.rs`](/Users/xuming/work/scriptlang-new/crates/sl-compiler/src/semantic/expand/dispatch.rs) 的统一调度；macro 定义和宏展开细节则收敛到 [`semantic/expand/macros.rs`](/Users/xuming/work/scriptlang-new/crates/sl-compiler/src/semantic/expand/macros.rs)
- `ExpandRegistry` 当前已经提供 builtin / macro 共用的统一分发入口；macro 当前支持：
  - 当前宏展开要求产出恰好一个根 form
- 当前宏系统以 `quote / unquote + MacroEnv` 为主路径：
  - 现在已经支持最小的 `quote / unquote`
  - 现在已经支持 compile-time `<let>`
  - 现在已经支持 `get_attribute` / `get_content`
  - `get_content` 现在支持 `head="..."`，可按调用点直接子标签筛选 AST 片段
  - `quote` 中的普通字符串属性和文本节点支持 `${local_name}` compile-time splice
  - 现在已经支持最小 hygiene：quote 中引入的 runtime `<temp>` 名会 gensym，并同步改写后续 expr 引用
  - gensym 当前按“当前调用 module 内 seed + 调用内局部计数”生成，既保持确定性，也更接近 Elixir 的 module-scoped counter 思路
  - 当前已有显式 `MacroEnv`
  - 当前 compile-time values 至少覆盖 `string / expr / ast / bool / int`
- program 级 macro registry 当前按 module 归档定义；expand dispatch 会按“当前 module -> 已 require modules -> 隐式 kernel”顺序解析可见宏
- `<macro>` 声明当前不再带 `scope`；宏定义本身不感知 module/script/statement 这类后续语义位置
- expand dispatch 仍然保留“当前调用位置”的内部上下文，但它只用于 builtin 分派和展开结果消费，不再参与宏声明注册
- 同名 macro 当前在同一 module 内不允许重复声明；registry 只按名字注册和解析
- `expand` 当前已经支持嵌套 `<module>`；子模块会在编译期展平成限定名 module
  - 例如 `main` 下的 `helper` 会变成 `main.helper`
  - 嵌套深度不限，`main.helper.grand` 这类多级子模块会递归展平
  - 父模块会自动获得直接子模块的词法短名 alias
  - 例如 `main` 可直接写 `@helper.entry`，`main.helper` 可直接写 `@grand.entry`
  - 父模块的 `children` 不再保留嵌套 `<module>` 节点
  - 子模块内的 `<macro>` 也会按限定名 module 递归注册到 program macro registry
- `import` 当前只负责成员短名可见性
- `require` 当前只负责宏可见性
- `<require>` 的典型用法是先引入提供 macro 的 module，再直接使用该 macro，例如：

```xml
<module name="main">
  <require name="helper" />
  <mk name="main" />
</module>
```

  `helper` 中的 `<macro name="mk">...</macro>` 会在 expand 阶段变成当前 module 可见的宏定义
- `alias` 当前只负责 module 名缩写；可用于 const / var / script / function 的显式限定引用
- 当前宏统一通过 compile-time 路径展开：`<let> + <quote> + <unquote>`
- 当前 compile-time 宏路径已可支撑标准 `if`、`unless`、`if-else` 宏；`kernel.xml` 中已有真实示例
- 在 form semantics 阶段完成 MVP 标签校验、属性校验、`<import>` / `<require>` / `<alias>` 上下文推进、统一名称解析、`<const>` 编译期求值和结构下沉
- `<const>` 只在 semantic analyze 阶段内存在；进入 `SemanticProgram` 后不再保留 const 声明
- compiler 当前为每个 module 隐式提供 kernel 上下文：
  - macro 解析有隐式 kernel fallback，不依赖显式 `require kernel`
  - semantic scope 仍会给每个 module 最早生效的 implicit `import kernel`
- semantic 当前的 module 导出目录已由 expand 阶段写入 `ProgramState`，`semantic/expand/*` 内部 helper 再做查询与消费，并解析 const / var 引用及 script 字面量
- semantic 当前会区分“module 内声明存在”和“对 import 暴露的导出成员”；`private="true"` 会从导出目录中隐藏该声明
- `assemble` 不再消费 import / scope / context 信息；它只消费已经解析好的语义结果
- `SemanticProgram / SemanticModule / SemanticScript / SemanticStmt / SemanticFunction / SemanticVar / DeclaredType` 当前已经作为 public inspect surface 暴露，供 `sl-repl` 读取中间结果
- semantic 中的 var 引用当前会先重写成“已解析变量占位符”；真正的 runtime global 命名只在 assemble 阶段生成
- `semantic/expand/const_eval.rs` 当前负责 builtin 常量求值、稳定字面量回写和表达式 / 模板替换，不再自己实现 import 可见性规则
- 在 assemble 阶段收集 module 级 `<var>` 声明、为 script 分配全局唯一 `script_id`
- 构造 `CompiledArtifact`
- 生成 boot script，先执行全局初始化，再跳转到默认入口
- 默认入口当前由 `CompileOptions.default_entry_script_ref` 决定；兼容入口仍默认使用 `main.main`
- `<const>` 当前只支持 module 级，且只支持 builtin 常量值与对前面已定义 const 的引用
- `<const>` 会按声明类型做编译期校验；当前已覆盖 `int / bool / string / script / function / array / object`
- `type="script"` 的 `<const>` 只允许 script 字面量或前置 script const 引用
- `<const>` 在 compiler 内消解为源码替换，不进入 runtime，也不会出现在 `CompiledArtifact.globals`
- `<var>` / `<temp>` 当前要求显式类型，但除 `script` 外暂不做完整表达式静态类型流转；semantic 先把可见 var 引用解析成规范占位符，assemble 再统一 lower 成 runtime 名
- const 名字解析当前支持：
  - 当前 module 的短名 const
  - imported module 的短名 const
  - `m1.zero` 形式的显式模块限定 const
  - `alias_name.zero` 形式的 alias 限定 const
  - imported private const 不可见，会报 `does not export const`
- var 名字解析当前支持：
  - 当前 module 的短名 var
  - imported module 的短名 var
  - `m1.value` 形式的显式模块限定 var
  - `alias_name.value` 形式的 alias 限定 var
  - imported private var 不可见，会报 `does not export var`
- `script` 字面量当前支持：
  - `@loop` 当前 module 下的短字面量
  - `@m1.entry` 形式的完整字面量
  - `@alias_name.entry` 形式的 alias 字面量
  - 编译期会校验字面量引用的 script 是否存在
- `function` 字面量当前支持：
  - `#pick` 当前 module 下的短字面量
  - `#m1.pick` 形式的完整字面量
  - `#alias_name.pick` 形式的 alias 字面量
  - 编译期会校验字面量引用的 function 是否存在
- module function 当前支持：
  - `<function name="run" args="int:x" return_type="int">return x + 1;</function>`
  - expr / code 内直接写 `main.run(3)`、`run(3)`、`helper.add1(value)`
  - `when="main.is_ready(value)"` 这类条件表达式里也可直接调用 function
  - `invoke(fn_ref, [args])` 用于通过 `function` 值做动态调用
  - function body 内同样支持 direct call / `invoke(...)`
- `<goto>` 当前不再做 script ref 名字解析；它只保留表达式并 lower 成运行时动态跳转
- runtime IR 中的表达式字符串当前不再分散挂在各指令字段上；统一使用 `CompiledExpr`
- 文本模板当前会 lower 成 `CompiledText { parts }`
  - 字面量片段保留为 `Literal`
  - 纯变量插值保留为 `VarRef`
  - 其余模板洞保留为 `Expr(CompiledExpr)`
- `kernel.xml` 当前只保留最小控制流宏集；API 单测和 integration example 已覆盖 required module macro 可见性解析，以及基于 `quote / unquote` 的 `if` / `unless` / `if-else` 标准宏
- `kernel` 当前还提供标准 `<if>` 宏；它通过 non-capturing `<while>` 结构实现，底层已不再保留单独的 builtin `if` lowering
- `<while>` 当前还支持 compiler-internal 属性 `__sl_skip_loop_control_capture="true"`
  - 默认值为 `false`
  - 默认情况下 `<while>` 会捕获内部 `<break>` / `<continue>`
  - `kernel` 中基于 `<while>` 实现的 `if / unless / if-else` 会显式打开这个内部开关，从而让这些控制流继续绑定外层真实循环
- `script_text` / `zero` 这类更偏示例性质的能力当前不再放在 kernel；对应示例改为用户 module 自己定义：
  - `19-user-script-text` 展示用户自定义 module macro 生成 `<script>`
  - `12-kernel-lib-const` 当前实际展示“用户自定义 `zero` const”的局部常量写法

当前 IR 指令包括：

- `EvalGlobalInit`
- `EvalTemp`
- `EvalCond`
- `ExecCode`
- `EmitText`
- `BuildChoice`
- `JumpIfFalse`
- `Jump`
- `JumpScript`
- `JumpScriptExpr`
- `ReturnToHost`
- `End`

`<choice>` 当前会 lower 成：

- 一条 `BuildChoice`
- 每个分支对应的一段线性 instruction
- 分支末尾插入 `Jump` 回 join 点

### Runtime

`sl-runtime` 负责：

- 执行 `CompiledArtifact`
- 用 `script_id + pc` 作为唯一执行定位
- 提供 `start(entry_script_ref) / step / choose / snapshot / resume`
- 使用 Rhai 执行表达式和代码块
- 首次执行某段 Rhai 源码时编译 AST，并在 runtime 内缓存
- runtime 现在优先按 IR 执行：
  - `CompiledTextPart::VarRef` 直接读取当前 local/global 并转成文本
  - `CompiledExpr.source` 才会进入 Rhai
- 对 `JumpScriptExpr` 先求值出 script key 字符串，再通过 `artifact.script_refs` 做跳转
- `ReturnToHost` 是 compiler/runtime internal instruction；它会结束当前执行并把控制权交还给宿主，而不是表示真实程序结束
- 运行时会为 expr / code 注册 `invoke` 和 compiler-internal `__sl_call`
- module function 当前编译进 `CompiledArtifact.functions`，运行时按函数 key 查找并在独立 Rhai scope 中执行 `CompiledFunction.body.source`

当前 runtime 不做：

- 名称解析
- module 语义处理
- AST 节点解释执行
- 宏展开
- `import` / `require` / `alias` 的编译期可见性处理
- script ref 级别的 compile-time 可见性规则

## API

`sl-api` 当前提供：

- `parse_modules_from_sources`
- `compile_artifact`
- `compile_artifact_from_xml_map`
- `start_runtime_session_from_xml_map`

其中 `parse_modules_from_sources` / `parse_module_xml` 返回 `Form`，而不是旧的 `ParsedModule`。

当前 `sl-api` 会在这些高层入口里自动加载内置库 XML。现阶段内置库只提供 `crates/sl-api/lib/kernel.xml`，并把它作为普通 module 一起参与编译；kernel 当前主要承载标准控制流宏，而不是示例性质的常量或文本宏。

这是当前最推荐的对外入口。

## REPL

`sl-repl` 当前已经从 inspect-only 工具变成真实执行型 REPL。它启动时只带 `kernel` 和一个隐藏的 session module / script，不会自动执行项目里的 `main.main`。

当前公开的主接口有：

- `ReplSession::new`
- `ReplSession::load_path`
- `ReplSession::submit_xml`
- `ReplSession::choose`
- `ReplSession::inspect`
- `ReplSession::eval_command`

`submit_xml` 当前接受三类顶层输入：

- statement-style fragment
  - 例如 `<text>`、`<temp>`、`<if>`、`<choice>`、`<goto>`、`<end>`
  - 也允许经过 `require` 后直接输入 statement macro 调用
- session context fragment
  - `<import>`
  - `<require>`
  - `<alias>`
- `<module>`
  - 允许在 REPL 内定义完整 module
  - 包括 module 级 macro / const / var / import / require / alias
  - 也包括 `<script>` 和 `<function>`
  - 这些定义一旦提交成功，就会进入当前 session 的后续编译环境

当前 REPL session 会持久维护三类状态：

- 顶层 program state
  - REPL 自己输入的顶层 `<module>` 与顶层 `<import>/<require>/<alias>`
  - 这些 form 会作为 session 的持久顶层程序参与后续编译
- 编译期 session context
  - 由持久顶层 program state 中的 `<import>` / `<require>` / `<alias>` 投影得到
- module overlay
  - `:load PATH` 读入的外部 XML module
  - 与 REPL 自己输入的顶层 `<module>` 一起参与编译
  - 同名 module 以后一次为准，REPL 输入会覆盖同名 loaded module
- 运行时状态
  - 顶层用户 `<temp>` 声明形成的持久 temp 绑定
  - 已执行全局变量的 runtime 值

顶层 XML 的执行模型当前是：

- REPL 会先把输入解析成一组顶层 form
- `<module>` 与顶层 `<import>/<require>/<alias>` 会进入 session 的持久顶层程序
- 顶层可执行 form 会作为本次增量入口执行
- compiler 会为隐藏 session script 生成 temp prelude 和本次增量执行入口
- prelude 只负责把旧 temp 名字重新声明回当前编译单元
- runtime 在 prelude 执行后，把上一次成功执行时保存的 temp / global 值恢复回当前 engine
- 本次增量入口后面会自动接一个 internal `ReturnToHost`
- 普通顶层执行跑到 `ReturnToHost` 后回到提示符
- `<choice>` 会 suspend，等待后续 `choose`
- `<end>` 会真实结束 REPL
- `<goto>` 如果跳到别的 script，则沿着目标 script 跑到真实 `End`；由于 `goto` 已放弃当前 session script 上下文，所以目标 script 结束时 REPL 也结束

当前 `sl-repl` crate 还带一个 binary，支持三种入口模式：

- 交互模式
  - 不带参数直接启动
  - CLI 会持续读入多行，直到当前 XML fragment 标签配平后再提交
  - 输入层使用 readline 风格终端编辑，因此方向键移动、history 和基础行内编辑都由终端库处理，不会把 `^[[D` 这类控制序列直接回显到 REPL
- 命令执行模式
  - `sl-repl --command '<text>hello</text>'`
  - `--command` 可以重复出现，按顺序执行多条 REPL 输入
  - 每条输入仍然走和交互模式相同的命令 / XML 提交语义
- 文件执行模式
  - `sl-repl --file path/to/session.repl`
  - 文件内容现在按“顶层 XML 输入”解释，而不是 transcript helper 模式
  - 一个文件里可以直接混合多个顶层 `<module>`、顶层 `<import>/<require>/<alias>` 和顶层可执行 XML
  - `--file` 不再支持把 `:load` / `:choose` 这类 host command 写进文件里

仓库级别另外提供了一个便捷脚本 [`scripts/repl-run.sh`](/Users/xuming/work/scriptlang-new/scripts/repl-run.sh)：

- 不带参数时
  - 会直接执行仓库里写死的 [`scripts/repl-target.xml`](/Users/xuming/work/scriptlang-new/scripts/repl-target.xml)
  - 这个文件本身就是普通 XML fragment，可以直接反复修改它来观察效果
- 传入任意文件时
  - 一律按 `sl-repl --file` 方式执行
  - 不再区分“module XML”和“REPL XML”
  - 因此 `<module>`、`<text>`、`<goto>`、以及多段顶层 XML 都在同一套顶层 session 语义下执行

当前支持的 host-side helper commands 有：

- `:help`
- `:load PATH`
- `:ast`
- `:semantic`
- `:ir`
- `:bindings`
- `:modules`
- `:choose INDEX`
- `:quit`

当前 REPL 的定位已经不是“手动 step runtime 的调试壳”，而是让 macro / lowering / runtime 行为都能以接近 IEx 的交互方式直接验证，同时保留 `:ast / :semantic / :ir` 这种中间态观察能力。

`sl-api::start_runtime_session_from_xml_map` 当前也不再直接以 `entry_script_ref` 启动 runtime。它会先构造一个隐藏 session module/script，再以一条内部 `<goto script="module.script"/>` 作为统一入口，因此传统 `module.script` 启动语义已经和 REPL submit 路径对齐。

为了兼容旧调用点，`create_engine_from_xml_map` 目前仍然保留为一个薄封装，但新的仓库内调用已经统一切到 `start_runtime_session_from_xml_map`。

## Integration Tests

当前 `make gate` 里的 coverage gate 只统计核心语言实现 crate：

- `sl-core`
- `sl-parser`
- `sl-compiler`
- `sl-runtime`

像 `sl-repl` 这种工具型 crate 当前不纳入 coverage 阈值；它仍然会参加 workspace `check / test / clippy`，但不会影响 `llvm-cov` 的行覆盖率和函数覆盖率门禁。

集成测试已经迁移到独立 crate：`sl-integration-tests`。

例子目录结构为：

```text
crates/sl-integration-tests/examples/<example>/
  xml/
    *.xml
  runs/<case>/
    actions.txt
    results.txt
    error.txt
```

约定：

- `xml/` 下放该例子的所有 XML 源文件
- `actions.txt` 描述运行时操作；如果没有 choice / snapshot 操作，可以省略
- `results.txt` 描述期望的可见输出序列
- `error.txt` 用于编译失败场景

当前支持的动作：

- `choose N`
- `snapshot-progress N`
- `snapshot-on-choice`
- `resume-snapshot`

当前测试 runner 会：

1. 读取例子目录下所有 XML
2. 通过 `sl-api` 执行 parse / compile / start runtime session
3. 按 `actions.txt` 驱动 runtime
4. 把实际结果和 `results.txt` 对比

当前例子集已覆盖的代表性场景包括：

- nested sub module 多级展平、父子模块短名引用与跳转
- `while / break / continue` lower 到现有 jump 指令
- kernel `<if>` 通过 `<while>` 宏化提供
- `26-kernel-if-via-while` 覆盖 `<if>` 经 kernel macro 展开后的运行链路
- `require` 导入的普通 module macro
  - 代表例子：`20-imported-module-macro`
- `function` 字面量 `#foo`
- `27-function-invoke` 覆盖 module function 定义、direct call、`invoke(fn_var, [args])`，以及 `when="..."` 中的 function 调用
- kernel `unless` / `if-else` 标准宏

## Build Commands

当前常用命令：

- `make fmt`
- `make test`
- `make lint`
- `make gate`

`make gate` 会按整个 workspace 执行 `fmt + test + lint`。

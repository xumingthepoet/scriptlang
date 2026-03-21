# Current Implementation

本文档只描述当前代码库中已经落地的实现，不描述长期目标。长期架构原则仍以 `AGENTS.md` 为准。

注意：当前仓库正在进入“macro enhancement”阶段，目标是在保留现有 env-driven expand 主线的前提下，补上 `quote / unquote`、编译期环境和最小 hygiene。当前计划见 [`MACRO_ENHANCEMENT_PLAN.md`](/Users/xuming/work/scriptlang-new/MACRO_ENHANCEMENT_PLAN.md)。

## Workspace Layout

当前项目已经拆成多 crate workspace：

- `sl-core`
  - 放共享核心类型
  - 包括错误类型、parser 产物类型、编译产物类型、IR、runtime step 结果、snapshot
  - 不依赖任何其他本地 crate
- `sl-parser`
  - 负责 `XML -> Form`
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
- `sl-integration-tests`
  - 独立的集成测试 crate
  - 通过 `sl-api` 驱动例子用例

根 crate `scriptlang-new` 当前主要做 re-export，方便外部统一使用。

## Current Language Scope

当前实现支持的 XML 子集：

- `<module>`
- `<import>`
- `<macro>`
- `private="true"` attribute on module-level `<const>`, `<var>`, `<script>`
- `<script>`
- `<var>`
- `<const>`
- `<temp>`
- `<if when="">`
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

当前明确不支持：

- `<while>`
- `<else>`
- `<call>`
- `<return>`

当前语义约束：

- `<if>` 只有单分支，没有 `else`
- `<goto script="">` 现在是表达式槽位，运行时要求其结果为 script key 字符串
- `<import>` 只能出现在 `<module>` 下，并按源码顺序向后影响当前 module 的编译期上下文
- `private="true"` 目前只影响 module 边界导出；同一 module 内仍可直接引用 private const / var / script
- `@main.loop` / `@loop` 是 script 字面量；`@loop` 会在编译期展开为当前 module 下的完整 script key
- `var / temp / const` 的 `type="..."` 现在是必填
- 当前 MVP 识别的显式类型有 `int / bool / string / script / array / object`
- runtime 不保留 module 概念，只按 `script_id + pc` 执行

## Parser / Compiler / Runtime

### Parser

`sl-parser` 负责：

- 读取 XML
- 校验根节点必须为 `<module>`
- 生成宿主无关的编译前表示 `Form { head, meta, fields }`
- 保留属性顺序，并在 `fields` 末尾固定追加 `children`
- 在 `children` 中递归保留文本项和子 form 的顺序

parser 不再承担 MVP 标签白名单和语义下沉；它当前只负责把 XML 结构化成可供宏和编译层消费的宿主无关前表示。

### Compiler

`sl-compiler` 负责：

- 以显式 pipeline 执行编译：
  - `Form -> semantic expand`
  - `expand` 直接消费 raw `Form`，顺序推进定义期状态，并把 module children / exports / imports / const declarations / macro definitions 沉淀到 `ProgramState`
  - `expand` 是当前唯一的前端语义入口；其内部通过 `ExpandEnv`、`ExpandRegistry` 和 `semantic/expand/*` 子模块完成定义期状态推进、macro 分派、名称解析和结构降解
  - `semantic program -> runtime IR`
- 源码目录当前按阶段分成：
  - 顶层 `pipeline.rs`
  - `semantic/`：名称解析、`<const>` 编译期求值、文本模板解析和语义下沉；当前包含 `env.rs`、`form.rs`、`expand/`、`expr/` 和 `types.rs`
  - `semantic/expand/`：承载 builtin/module macro expansion、module/import definition-time state、module catalog、scope resolution、const evaluation 和 script lowering analysis
  - `assemble/`：声明收集、lowering、boot script、`CompiledArtifact` 装配
- `semantic/form.rs` 当前统一承载 raw `Form` 的属性、body、children 和错误定位 helper；旧 `classify.rs` 已删除
- `expand` 入口会直接对 raw `Form` 做 module / import / const / var / script / local temp 的顺序遍历和定义期状态维护；`ExpandEnv` 会累计整份程序的 module 状态快照，包括 module order、children、exports、imports、const declarations 和 macro definitions
- `semantic/expand/` 当前已经按职责拆分：
  - [`dispatch.rs`](/Users/xuming/work/scriptlang-new/crates/sl-compiler/src/semantic/expand/dispatch.rs)：统一 expand 分派入口，负责 builtin / macro hook 路由
  - [`imports.rs`](/Users/xuming/work/scriptlang-new/crates/sl-compiler/src/semantic/expand/imports.rs)：import 目标校验
  - [`macros.rs`](/Users/xuming/work/scriptlang-new/crates/sl-compiler/src/semantic/expand/macros.rs)：macro 定义收集、可见性查找和模板式宏展开
  - [`quote.rs`](/Users/xuming/work/scriptlang-new/crates/sl-compiler/src/semantic/expand/quote.rs)：`quote / unquote`、AST splice 和最小 hygiene
  - [`modules.rs`](/Users/xuming/work/scriptlang-new/crates/sl-compiler/src/semantic/expand/modules.rs)：module catalog 与 script 字面量查找
  - [`scope.rs`](/Users/xuming/work/scriptlang-new/crates/sl-compiler/src/semantic/expand/scope.rs)：module scope、const catalog 和 var/const 解析
  - [`program.rs`](/Users/xuming/work/scriptlang-new/crates/sl-compiler/src/semantic/expand/program.rs)：program/module 级语义总调度
  - [`scripts.rs`](/Users/xuming/work/scriptlang-new/crates/sl-compiler/src/semantic/expand/scripts.rs)：script body 和 statement lowering
  - [`declared_types.rs`](/Users/xuming/work/scriptlang-new/crates/sl-compiler/src/semantic/expand/declared_types.rs)：声明类型解析与 `<const>` 声明注册
  - [`const_eval.rs`](/Users/xuming/work/scriptlang-new/crates/sl-compiler/src/semantic/expand/const_eval.rs)：builtin 常量求值与常量替换
- `semantic/expr/` 统一承载 expr 前端处理；`script literal` 会先经过统一 token 扫描，模板 `${...}` 的洞会先落到 `ExprSource` 外壳后再回到当前 `TextTemplate` 主路径
- builtin form 的 expand 处理当前已收敛到 [`semantic/expand/dispatch.rs`](/Users/xuming/work/scriptlang-new/crates/sl-compiler/src/semantic/expand/dispatch.rs) 的统一调度；macro 定义和宏展开细节则收敛到 [`semantic/expand/macros.rs`](/Users/xuming/work/scriptlang-new/crates/sl-compiler/src/semantic/expand/macros.rs)
- `ExpandRegistry` 当前已经提供 builtin / macro 共用的统一分发入口；macro 目前支持最小 MVP：
  - 当前支持 `scope="statement"` 和 `scope="module"`
  - 宏体支持 `{{attr_name}}` 属性替换
  - 宏体支持 `<yield/>` 把调用点 children 拼接进宏体
  - 当前宏展开要求产出恰好一个根 form
- 当前宏系统仍是 MVP：
  - `scope / {{attr_name}} / <yield/>` 这套宏表层语法是过渡方案；长期目标是以 `quote / unquote + MacroEnv` 取代它
  - 现在已经支持最小的 `quote / unquote`
  - 现在已经支持 compile-time `<let>`
  - 现在已经支持 `get_attribute` / `get_content`
  - `get_content` 现在支持 `head="..."`，可按调用点直接子标签筛选 AST 片段
  - 现在已经支持最小 hygiene：quote 中引入的 runtime `<temp>` 名会 gensym，并同步改写后续 expr 引用
  - 但仍没有完整的显式 `MacroEnv` 公共模型
  - 也还没有完整的 compile-time value system 和更广泛的 quote splice 规则
- program 级 macro registry 当前按 module 归档定义；expand dispatch 会按“当前 module -> 已 import modules -> 隐式 kernel”顺序解析可见宏
- 同名 macro 当前允许在不同 `scope` 下共存；分派时会按 `(name, scope)` 而不是只按名字解析
- macro 当前同时支持两条路径：
  - 旧的模板式路径：`{{attr_name}}` + `<yield/>`
  - 新的 compile-time 路径：`<let> + <quote> + <unquote>`
- 新的 compile-time 宏路径当前已可支撑标准 `unless` 和 `if-else` 宏；`kernel.xml` 中已有真实示例
- 在 form semantics 阶段完成 MVP 标签校验、属性校验、`<import>` 上下文推进、统一名称解析、`<const>` 编译期求值和结构下沉
- `<const>` 只在 semantic analyze 阶段内存在；进入 `SemanticProgram` 后不再保留 const 声明
- compiler 当前为每个 module 隐式提供最早生效的 `import kernel` 上下文
- semantic 当前的 module 导出目录已由 expand 阶段写入 `ProgramState`，`semantic/expand/*` 内部 helper 再做查询与消费，并解析 const / var 引用及 script 字面量
- semantic 当前会区分“module 内声明存在”和“对 import 暴露的导出成员”；`private="true"` 会从导出目录中隐藏该声明
- `assemble` 不再消费 import / scope / context 信息；它只消费已经解析好的语义结果
- semantic 中的 var 引用当前会先重写成“已解析变量占位符”；真正的 runtime global 命名只在 assemble 阶段生成
- `semantic/expand/const_eval.rs` 当前负责 builtin 常量求值、稳定字面量回写和表达式 / 模板替换，不再自己实现 import 可见性规则
- 在 assemble 阶段收集 module 级 `<var>` 声明、为 script 分配全局唯一 `script_id`
- 构造 `CompiledArtifact`
- 生成 boot script，先执行全局初始化，再跳转到默认入口
- 默认入口当前固定为 `main.main`；若不存在则编译报错
- `<const>` 当前只支持 module 级，且只支持 builtin 常量值与对前面已定义 const 的引用
- `<const>` 会按声明类型做编译期校验；当前已覆盖 `int / bool / string / script / array / object`
- `type="script"` 的 `<const>` 只允许 script 字面量或前置 script const 引用
- `<const>` 在 compiler 内消解为源码替换，不进入 runtime，也不会出现在 `CompiledArtifact.globals`
- `<var>` / `<temp>` 当前要求显式类型，但除 `script` 外暂不做完整表达式静态类型流转；semantic 先把可见 var 引用解析成规范占位符，assemble 再统一 lower 成 runtime 名
- const 名字解析当前支持：
  - 当前 module 的短名 const
  - imported module 的短名 const
  - `m1.zero` 形式的显式模块限定 const
  - imported private const 不可见，会报 `does not export const`
- var 名字解析当前支持：
  - 当前 module 的短名 var
  - imported module 的短名 var
  - `m1.value` 形式的显式模块限定 var
  - imported private var 不可见，会报 `does not export var`
- `script` 字面量当前支持：
  - `@loop` 当前 module 下的短字面量
  - `@m1.entry` 形式的完整字面量
  - 编译期会校验字面量引用的 script 是否存在
- `<goto>` 当前不再做 script ref 名字解析；它只保留表达式并 lower 成运行时动态跳转
- `kernel.xml` 当前除常量外，已可声明最小 kernel macro；API 单测和 integration example 已覆盖 statement-scope 与 module-scope 的基本宏展开路径、imported module macro 可见性解析，以及基于 `quote / unquote` 的 `unless` / `if-else` 标准宏

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
- 对 `JumpScriptExpr` 先求值出 script key 字符串，再通过 `artifact.script_refs` 做跳转

当前 runtime 不做：

- 名称解析
- module 语义处理
- AST 节点解释执行
- 宏展开
- `import` / module var 可见性处理
- script ref 级别的 compile-time 可见性规则

## API

`sl-api` 当前提供：

- `parse_modules_from_sources`
- `compile_artifact`
- `compile_artifact_from_xml_map`
- `create_engine_from_xml_map`

其中 `parse_modules_from_sources` / `parse_module_xml` 返回 `Form`，而不是旧的 `ParsedModule`。

当前 `sl-api` 会在这些高层入口里自动加载内置库 XML。现阶段内置库只提供 `crates/sl-api/lib/kernel.xml`，并把它作为普通 module 一起参与编译；默认 `zero` 这类内置名字来自 compiler 的隐式 `import kernel` 上下文，而不是 API 侧源码注入。

这是当前最推荐的对外入口。

## Integration Tests

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
2. 通过 `sl-api` 执行 parse / compile / create engine
3. 按 `actions.txt` 驱动 runtime
4. 把实际结果和 `results.txt` 对比

## Build Commands

当前常用命令：

- `make fmt`
- `make test`
- `make lint`
- `make gate`

`make gate` 会按整个 workspace 执行 `fmt + test + lint`。

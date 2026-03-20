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

当前明确不支持：

- `<while>`
- `<else>`
- `<call>`
- `<return>`

当前语义约束：

- `<if>` 只有单分支，没有 `else`
- `<goto>` 可以跳到可解析的其他 script，包括跨 module script
- `<import>` 只能出现在 `<module>` 下，并按源码顺序向后影响当前 module 的编译期上下文
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
  - `Form -> macro expansion`
  - `expanded Form -> module/script/var/stmt 语义结构`
  - `semantic program -> runtime IR`
- 源码目录当前按阶段分成：
  - 顶层 `expand.rs` / `pipeline.rs`
  - 顶层 `form_util.rs`：`Form` 读取和定位报错辅助
  - `semantic/`：名称解析、`<const>` 编译期求值、文本模板解析、语义下沉
  - `assemble/`：声明收集、lowering、boot script、`CompiledArtifact` 装配
- 当前 macro expansion 阶段已经独立成单独步骤，但仍是 no-op passthrough
- 在 form semantics 阶段完成 MVP 标签校验、属性校验、`<import>` 上下文推进、统一名称解析、`<const>` 编译期求值和结构下沉
- `<const>` 只在 semantic analyze 阶段内存在；进入 `SemanticProgram` 后不再保留 const 声明
- compiler 当前为每个 module 隐式提供最早生效的 `import kernel` 上下文
- semantic 当前会先建立 module 导出目录和作用域，再把 const / goto 等名字解析成规范目标
- `assemble` 不再消费 import / scope / context 信息；它只消费已经解析好的语义结果
- `const_eval` 当前只负责 builtin 常量求值、稳定字面量回写和表达式 / 模板替换，不再自己实现 import 可见性规则
- 在 assemble 阶段收集 module 级 `<var>` 声明、为 script 分配全局唯一 `script_id`
- 构造 `CompiledArtifact`
- 生成 boot script，先执行全局初始化，再跳转到默认入口
- 默认入口当前固定为 `main.main`；若不存在则编译报错
- `<const>` 当前只支持 module 级，且只支持 builtin 常量值与对前面已定义 const 的引用
- `<const>` 在 compiler 内消解为源码替换，不进入 runtime，也不会出现在 `CompiledArtifact.globals`
- `<var>` 当前支持跨 module 引用；compiler 会把可见 var 名字重写成内部 runtime global 名，而不是把 import 可见性规则泄漏到 runtime
- const 名字解析当前支持：
  - 当前 module 的短名 const
  - imported module 的短名 const
  - `m1.zero` 形式的显式模块限定 const
- var 名字解析当前支持：
  - 当前 module 的短名 var
  - imported module 的短名 var
  - `m1.value` 形式的显式模块限定 var
- `<goto>` 当前支持：
  - 当前 module 短名 script
  - imported module 的短名 script
  - `@m1.entry` 形式的显式模块限定 script
  - 显式模块限定目标若 module 已存在但未 import，会在 semantic 阶段报可见性错误

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
- `End`

`<choice>` 当前会 lower 成：

- 一条 `BuildChoice`
- 每个分支对应的一段线性 instruction
- 分支末尾插入 `Jump` 回 join 点

### Runtime

`sl-runtime` 负责：

- 执行 `CompiledArtifact`
- 用 `script_id + pc` 作为唯一执行定位
- 提供 `start / step / choose / snapshot / resume`
- 使用 Rhai 执行表达式和代码块
- 首次执行某段 Rhai 源码时编译 AST，并在 runtime 内缓存

当前 runtime 不做：

- 名称解析
- module 语义处理
- AST 节点解释执行
- 宏展开
- `import` / module var 可见性处理

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

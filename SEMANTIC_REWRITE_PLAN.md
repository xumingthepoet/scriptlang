# Semantic Rewrite Plan

本文档只保留当前仍有决策价值的内容：最终目标、当前差距和下一步。已完成的历史迁移步骤不再逐条记录；已经落地的事实以 [`IMPLEMENTATION.md`](/Users/xuming/work/scriptlang-new/IMPLEMENTATION.md) 为准。

Last updated: 2026-03-21

## Goal

`sl-compiler` 前端要继续向 Elixir 风格的 env-driven expand 收敛：

1. `XML -> Form`
2. `Form -> expand`
3. `expand result -> assemble`
4. `assemble -> runtime boundary IR`

这里的关键不是“单独加一个 macro pass”，而是：

- `expand` 成为前端语义中心
- builtin form 和 macro 共享同一套分派模型
- definition-time state 在 expand 中推进
- `assemble` 只负责降到 runtime 最小边界

## Current Baseline

当前已经成立的前提：

- 旧的 `classify / analyze / resolve / const_eval` 文件名已从源码树删除
- pipeline 主线已经是 `Form -> semantic::expand -> assemble`
- `semantic/form.rs` 承担 raw form helper
- `semantic/expr/` 已承担 expr 扫描、rewrite、模板洞解析
- `ExpandEnv + ProgramState` 已是定义期状态主入口
- macro 已支持：
  - `kernel` module 中声明
  - `scope="statement"` 和 `scope="module"`
  - `{{attr_name}}`
  - `<yield/>`
  - imported module macro 可见性解析
  - 同名不同 scope 共存

## Remaining Gaps

还没有完全收敛到目标形态的地方主要有 3 类。

### 1. `expand` 仍有明显 helper 分层

当前 [`semantic/expand/`](/Users/xuming/work/scriptlang-new/crates/sl-compiler/src/semantic/expand) 里仍然能看到比较强的 helper 边界：

- [`program.rs`](/Users/xuming/work/scriptlang-new/crates/sl-compiler/src/semantic/expand/program.rs)
- [`scope.rs`](/Users/xuming/work/scriptlang-new/crates/sl-compiler/src/semantic/expand/scope.rs)
- [`const_values.rs`](/Users/xuming/work/scriptlang-new/crates/sl-compiler/src/semantic/expand/const_values.rs)

它们虽然已经不再是旧阶段文件，但语义上还没有完全被“统一的 expand rule + env”模型吞掉。

### 2. expr 仍然主要是 Rhai 文本外壳

当前 expr 已经进入统一入口，但还只是轻量结构：

- `script literal` 会先扫描
- 模板洞会先落到 `ExprSource`
- 但主干仍然是 Rhai 文本 + rewrite

这比之前稳定，但还没有形成更完整的前端表达式表示。

### 3. macro 还不是完整的 Elixir 式生命周期

当前已经可以靠 `kernel` 和 imported module macro 扩语法，但还缺：

- 更完整的 module/import macro 生命周期
- 更明确的冲突/优先级策略
- 真正的 user macro 接入面
- declaration-generating macro 的更强定义期状态支持

## Immediate Next Steps

接下来继续收敛时，优先级建议如下：

1. 继续压缩 [`semantic/expand/program.rs`](/Users/xuming/work/scriptlang-new/crates/sl-compiler/src/semantic/expand/program.rs)、[`semantic/expand/scope.rs`](/Users/xuming/work/scriptlang-new/crates/sl-compiler/src/semantic/expand/scope.rs)、[`semantic/expand/const_values.rs`](/Users/xuming/work/scriptlang-new/crates/sl-compiler/src/semantic/expand/const_values.rs) 的 helper 边界。
2. 让更多语义决策直接挂到 `ExpandRegistry` 和 `ExpandEnv`，减少“先 expand，再进另一套 helper 分析”的感觉。
3. 继续把 expr 从“Rhai 文本 + rewrite”往更稳定的前端表示推进，但不急着上完整 AST。
4. 在现有 macro registry 基础上，补更完整的 module/import/user macro 生命周期设计。

## When To Delete This File

当以下条件同时满足时，这个文件可以删除：

- `IMPLEMENTATION.md` 已足够描述当前稳定前端结构
- 这里不再存在真实未完成项
- `semantic` 前端已经不再处于“继续重构中”的状态

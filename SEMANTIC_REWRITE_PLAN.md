# Semantic Rewrite Plan

本文档描述 `sl-compiler` 前端语义层向 Elixir 风格编译前端靠拢的最终目标、实施步骤和当前进展。

这不是历史记录文档，而是当前生效的重构目标文档。每次前端语义层、expr 处理方式、expand 结构或阶段边界发生变化时，都要同步更新本文档。

Last updated: 2026-03-21 (Phase 1 advanced: kernel module macro MVP landed)

## Why

当前 compiler 前端已经验证了一批 MVP 语言能力，但仍有几类结构性问题：

- 前端职责分散在 `classify / analyze / resolve / const_eval / expr/*` 多个并列模块中
- expr 仍以 Rhai 字符串为主载体，并依赖启发式扫描与局部重写
- `special form`、未来 macro、名称解析、const 展开还没有统一到 env-driven expand 模型
- `semantic` 当前更像“若干后处理阶段”，而不是“以 expansion 为中心的前语义主战场”

如果继续沿现状增量演进，后续 macro、module injection、definition-time state、expr 类型检查会越来越难以落地。

## End Goal

最终目标不是“加一个 macro pass”，而是把 compiler 前端重构成：

1. `XML -> Form`
2. `Form -> env-driven expand`
3. `expanded / normalized high-level program -> assemble`
4. `assemble -> runtime boundary IR`

其中：

- `expand` 是前端语义中心
- `special form` 视为内建 expand rules
- 用户 macro 将来复用同一套 env-driven expansion 机制
- `assemble` 只负责从稳定的前端输出降到 runtime 最小边界

## Target Frontend Architecture

目标结构：

- `semantic/env.rs`
  - `ExpandEnv`
  - `ModuleState`
  - `LocalScope`
  - `CompilePhase`
- `semantic/form.rs`
  - raw `Form` helper
  - attribute/body/children access
  - shared source-location error formatting
- `semantic/expand/`
  - `mod.rs`
  - `module.rs`
  - `consts.rs`
  - `rules.rs`
- `semantic/expr/`
  - `mod.rs`
  - `types.rs`
  - `scan.rs`
  - `rewrite.rs`
  - `template.rs`
- `assemble/`
  - 保持 compiler/runtime 边界 lowering 职责

目标职责：

- `env`
  - 统一承载当前 module、imports、visible consts、locals、phase、source context
- `expand`
  - 处理内建 form
  - 处理 kernel macro
  - 推进 env
  - 做 const 展开、名字解析、slot interpretation、definition-time checks
  - 产出 normalized high-level program
- `expr`
  - 统一处理 expr 的扫描、特殊 token、rewrite、模板洞
  - 禁止其他模块自行直接扫描 expr 字符串
- `assemble`
  - 只消费 expand 结果
  - 不再承载 import / scope / visibility / macro 语义

## Expr Strategy

expr 是这轮重构的关键点，单独列出目标。

### Hard Requirements

- expr 仍然允许以 Rhai 作为 runtime 执行后端
- compile artifact 仍然保持 JSON 友好
- `@...` 继续作为 `script` literal 语法
- `${...}` 继续表示 runtime 求值并转成文本

### Target Model

不直接上完整通用表达式 AST，但至少要形成一层轻量结构化表示：

- `ExprSource`
  - `raw`
  - `kind`
  - `tokens / spans`
- `ExprKind`
  - `Rhai`
  - `TemplateHole`
- `SpecialToken`
  - `ScriptLiteral`
  - `IdentRef`
  - `QualifiedRef`

### Rules

- 普通 Rhai 表达式可以保持黑盒文本
- 语言级特殊语法必须先统一扫描成结构化 token
- const / var / script literal rewrite 只能经由 `semantic/expr/*`
- `scope.rs`、`program.rs`、未来 `expand/*` 不允许再各自实现 expr 扫描逻辑

## Migration Strategy

不是直接把旧 `semantic/` 物理清空，而是分阶段替换，并保留旧实现快照供参考。

旧实现快照路径：

- [`semantic-legacy-20260321`](/Users/xuming/work/scriptlang-new/.codex-snapshots/semantic-legacy-20260321)

### Phase 0: Stabilize Current Baseline

目标：

- 保留可运行的现有实现
- 收紧明显边界，为重写创造条件

完成条件：

- `script ref` 旧系统删除
- `<goto script="">` 变成 expr
- `var / temp / const` 明确要求 `type`
- expr 字符串重写逻辑被集中管理

### Phase 1: Build New Semantic Skeleton

目标：

- 建立新的 `semantic/env.rs`
- 建立新的 `semantic/expand/`
- 建立新的 `semantic/expr/`

完成条件：

- 新目录结构存在
- pipeline 可以切到新入口
- 旧逻辑允许暂时通过适配层复用

### Phase 2: Move Const + Expr Into New Frontend Core

目标：

- 把 const 处理迁入新的 expand/env 模型
- 把 expr 处理迁入新的 `semantic/expr/*`

完成条件：

- 旧 `const_eval.rs` 不再作为独立前端中心
- expr 不再由多个模块各自扫描

### Phase 3: Replace classify / analyze / resolve With env-driven expand

目标：

- 内建 form 改为 builtin expand rules
- imports / consts / locals / module state 全部在 expand 中推进

完成条件：

- `classify.rs`、`analyze.rs`、旧 query/lookup 职责的阶段边界都会被新 expand 吸收
- pipeline 简化为 `Form -> expand -> assemble`

### Phase 4: Prepare For User Macros

目标：

- 在已有内建 expand rules 基础上，为用户 macro 留出统一挂载点

完成条件：

- builtin form 和 user macro 共享同一 env-driven expansion 模型
- module definition-time state 能承载 declaration-generating macros

## Current Progress

状态说明：

- `done`: 已完成并进入当前代码基线
- `in_progress`: 已开始但未形成最终结构
- `pending`: 尚未开始

### Done

- `done` 删除旧 script-ref 解析系统，改为 `script` 值类型
- `done` `<goto script="">` 改为 expr 槽位，runtime 新增动态 script 跳转
- `done` `var / temp / const` 改为必须显式 `type`
- `done` 当前支持的显式类型闭包补到 `int / bool / string / script / array / object`
- `done` 删除顶层空 `expand.rs`
- `done` 旧 `classify.rs` 已从主路径和源码树删除，raw-form helper 收敛到 [`semantic/form.rs`](/Users/xuming/work/scriptlang-new/crates/sl-compiler/src/semantic/form.rs)
- `done` expr 启发式 rewrite 已集中到 [`semantic/expr/rewrite.rs`](/Users/xuming/work/scriptlang-new/crates/sl-compiler/src/semantic/expr/rewrite.rs)
- `done` 旧前端实现已保存快照，便于重写时参考
- `done` pipeline 已切到 [`semantic/expand/mod.rs`](/Users/xuming/work/scriptlang-new/crates/sl-compiler/src/semantic/expand/mod.rs) 作为唯一前端入口
- `done` `analyze / resolve` 已改为直接消费 raw `Form`，不再依赖 `ClassifiedForm`
- `done` [`ExpandEnv`](/Users/xuming/work/scriptlang-new/crates/sl-compiler/src/semantic/env.rs) 已开始累计整份程序的 module 状态快照，而不再只保存当前 module
- `done` `ProgramState` 现在会保存 module order、expanded module children、exports、imports 和 const declarations
- `done` `ModuleCatalog` 已改为从 `ProgramState` 构建，不再自行扫描 raw forms 生成导出目录
- `done` `analyze` 当前优先消费 `ProgramState`，而不是再从 top-level forms 重建模块世界
- `done` builtin form 的 expand 处理已收敛到 [`semantic/expand/rules.rs`](/Users/xuming/work/scriptlang-new/crates/sl-compiler/src/semantic/expand/rules.rs) 的统一 rule 调度
- `done` 旧的 `resolve.rs` / `analyze.rs` / `const_eval.rs` / `query.rs` 文件名已从源码树删除；当前对应 helper 已收敛进 [`semantic/expand/`](/Users/xuming/work/scriptlang-new/crates/sl-compiler/src/semantic/expand)
- `done` kernel module 现在已支持最小 macro MVP：`<macro name=\"...\" scope=\"statement|module\">...</macro>`
- `done` `ExpandRegistry` 当前已经提供 builtin / kernel macro 共用的统一分发入口
- `done` 当前宏体支持 `{{attr_name}}` 替换和 `<yield/>` children 拼接，并已有 API / integration coverage
- `done` `semantic/expr/*` 已开始进入主路径：`script literal` 先经统一 token 扫描再 rewrite，模板洞也会先落到 `ExprSource`
- `done` program 级 macro registry 已从 `kernel` 特例表泛化为按 module 归档的定义表，expand dispatch 现在会按当前 module / imports / implicit kernel 解析可见宏
- `done` macro registry 当前已经按 `(name, scope)` 分派，允许同名 statement/module macro 共存

### In Progress

- `in_progress` 现有 `semantic` 仍保留旧语义思路按 `program / scope / const_values` helper 分层，尚未被 `expand rules + env` 完全吞掉
- `in_progress` expr 仍然主要以 Rhai 文本为载体；虽然 `ExprSource` 已开始进入主路径，但还没有形成更完整的轻量结构化表示
- `in_progress` `program / scope / consts` 还没有被统一到 env-driven expand
- `in_progress` 已建立新的 [`semantic/env.rs`](/Users/xuming/work/scriptlang-new/crates/sl-compiler/src/semantic/env.rs) 骨架
- `in_progress` 已建立新的 [`semantic/expr/`](/Users/xuming/work/scriptlang-new/crates/sl-compiler/src/semantic/expr) 目录，并把 rewrite/scan 逻辑迁入其中
- `in_progress` 已建立新的 [`semantic/expand/mod.rs`](/Users/xuming/work/scriptlang-new/crates/sl-compiler/src/semantic/expand/mod.rs) 入口；当前会先维护定义期状态，再进入 `expand/program.rs` 中的 semantic program analysis
- `in_progress` 已建立 [`semantic/expand/module.rs`](/Users/xuming/work/scriptlang-new/crates/sl-compiler/src/semantic/expand/module.rs)、[`consts.rs`](/Users/xuming/work/scriptlang-new/crates/sl-compiler/src/semantic/expand/consts.rs) 和 [`rules.rs`](/Users/xuming/work/scriptlang-new/crates/sl-compiler/src/semantic/expand/rules.rs) 骨架
- `in_progress` `expand` 现在已开始顺序维护定义期状态：module name、imports、exports、local temps、const declarations，并把 module 状态沉淀进程序级 env 快照
- `in_progress` `type` 解析已开始从旧 `analyze / query` 侧收敛到 [`semantic/expand/consts.rs`](/Users/xuming/work/scriptlang-new/crates/sl-compiler/src/semantic/expand/consts.rs)

### Pending

- `pending` declaration-time module state 继续替换剩余静态 resolve/analyze 逻辑
- `in_progress` 当前已具备 import 可见宏解析，但还没有更完整的 module/import macro 生命周期与冲突策略
- `pending` 用户 macro 接入点

## Immediate Next Steps

建议按下面顺序推进：

1. 建 `semantic/env.rs`
2. 建 `semantic/expr/`，先把当前 rewrite/scan 逻辑拆进去
3. 建 `semantic/expand/mod.rs`，先让 pipeline 切到新入口
4. 建 `semantic/expand/module.rs` / `consts.rs` / `rules.rs`
5. 用 `semantic/form.rs` 接住 raw-form helper，删掉 `classify.rs`
6. 把 `const` 处理迁进新 expand/env
7. 把 `ModuleScope / ConstCatalog / ScopeResolver` 逐步改成直接围绕 `ProgramState`
8. 把 `program / scope / const_values` 的逻辑逐步吸收入新 expand rule / env 模型

## Constraints

- 不修改 runtime 最小边界，除非新前端需求明确证明 runtime primitive 不足
- compile artifact 继续保持 JSON 友好
- Rhai 继续作为 runtime 执行后端，但不再定义 compiler 前端的内部表示
- 每次阶段边界、目录结构、支持范围变化，都要同步更新：
  - [`IMPLEMENTATION.md`](/Users/xuming/work/scriptlang-new/IMPLEMENTATION.md)
  - 本文档

## Definition Of Done

当以下条件同时满足时，这轮重构才算完成：

- pipeline 只剩 `Form -> expand -> assemble`
- 旧 `classify / analyze / resolve / const_eval` 不再作为前端主阶段存在
- expr rewrite 不再散落在多个模块
- `ExpandEnv / ModuleState` 成为前端上下文单一事实来源
- builtin forms 和未来 macro 共享统一 expansion 模型
- `IMPLEMENTATION.md` 与本文档都反映新结构

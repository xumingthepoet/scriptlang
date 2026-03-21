# Semantic Rewrite Plan

本文档描述 `sl-compiler` 前端语义层向 Elixir 风格编译前端靠拢的最终目标、实施步骤和当前进展。

这不是历史记录文档，而是当前生效的重构目标文档。每次前端语义层、expr 处理方式、expand 结构或阶段边界发生变化时，都要同步更新本文档。

Last updated: 2026-03-21

## Why

当前 compiler 前端已经验证了一批 MVP 语言能力，但仍有几类结构性问题：

- 前端职责分散在 `classify / analyze / resolve / const_eval / expr_rewrite` 多个并列模块中
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
- `semantic/expand/`
  - `mod.rs`
  - `module.rs`
  - `builtin.rs`
  - `consts.rs`
  - `stmts.rs`
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
- `resolve.rs`、`analyze.rs`、未来 `expand/*` 不允许再各自实现 expr 扫描逻辑

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

- `classify.rs`、`analyze.rs`、`resolve.rs` 的职责被新 expand 吸收
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
- `done` `classify.rs` 已下沉到 `semantic/`
- `done` expr 启发式 rewrite 已集中到 [`semantic/expr_rewrite.rs`](/Users/xuming/work/scriptlang-new/crates/sl-compiler/src/semantic/expr_rewrite.rs)
- `done` 旧前端实现已保存快照，便于重写时参考

### In Progress

- `in_progress` 现有 `semantic` 仍是旧分层，只是先做了边界收缩
- `in_progress` expr 仍然主要以 Rhai 文本为载体，尚未形成新的轻量结构化表示
- `in_progress` `classify / analyze / resolve / const_eval` 还没有被统一到 env-driven expand

### Pending

- `pending` 新的 `semantic/env.rs`
- `pending` 新的 `semantic/expand/`
- `pending` 新的 `semantic/expr/`
- `pending` pipeline 切换到 `expand` 中心
- `pending` declaration-time module state 替换静态 catalog
- `pending` builtin form handler 化
- `pending` 用户 macro 接入点

## Immediate Next Steps

建议按下面顺序推进：

1. 建 `semantic/env.rs`
2. 建 `semantic/expr/`，先把 `expr_rewrite.rs` 拆进去
3. 建 `semantic/expand/mod.rs`，先只做内建 forms
4. 把 `const` 处理迁进新 expand/env
5. 把 `classify / analyze / resolve` 的逻辑逐步吸收入新 expand

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

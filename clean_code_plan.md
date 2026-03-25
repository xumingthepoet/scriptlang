# Clean Code Plan

## 目标
以 Clean Code 为目标，系统性地优化代码质量。每轮独立寻找优化点，实施后保证测试通过再提交。

## 当前代码库状态

### 统计数据
- 总 Rust 文件：72 个
- >400 行的文件：15 个（原 22 个，-7）
- >800 行的文件：1 个（原 8 个，-7）
- 总代码行数：~30,000+

> 拆分 builtins.rs 后，>800 行文件减少 7 个，可维护性显著提升。

### 分层架构
```
sl-core      → 共享核心类型（无依赖）
sl-parser    → XML → Form 解析
sl-compiler  → Form → CompiledArtifact
sl-runtime   → Artifact 执行
sl-api       → 统一 API（parser + compiler + runtime）
sl-repl      → REPL 实现
```

---

## 优化优先级

### 🔴 P0 - 必须处理（影响可维护性）

| 文件 | 行数 | 问题 | 建议 |
|------|------|------|------|
| `macro_lang/tests.rs` | 6,171 | 超大测试文件 | 按功能拆分为独立模块 |
| `builtins.rs` | 2,329 | 单文件承载所有内置函数 | 按类别拆分 |

### 🟡 P1 - 重要（影响可读性）

| 文件 | 行数 | 问题 | 建议 |
|------|------|------|------|
| `sl-repl/src/lib.rs` | 1,962 | Session 管理与命令解析混在一起 | 拆分 session/commands/inspector |
| `const_eval.rs` | 1,112 | 接近 800 上限 | 考虑拆分 parser 逻辑 |
| `engine/mod.rs` | 1,007 | 运行时引擎主文件 | 检查是否有提取空间 |

### 🟢 P2 - 改进（持续优化）

- 减少不必要的 `.clone()` 调用
- 提取重复模式为通用辅助函数
- 统一错误消息格式
- 添加缺失的文档注释

---

## 执行计划

### Round 1: 拆分测试文件
- [x] 将 `macro_lang/tests.rs` 按功能拆分为：
  - `tests/tests_helpers.rs` - 共享 use 导入 + helper 函数
  - `tests/ct_lang_tests/tests_ct_eval.rs` - 27 基本 eval + 核心 builtin 测试
  - `tests/ct_lang_tests/tests_convert.rs` - 93 convert 测试
  - `tests/ct_lang_tests/tests_builtins.rs` - 38 高级 builtin 测试 (AST/module/list/keyword/match)
- 状态：**完成** (make gate 通过，158 测试全通过，覆盖率 89.78%)

### Round 2: 拆分 Builtins
- [x] 将 `builtins.rs` 按类别拆分：
  - `builtins/builtins_registry.rs` - 注册表结构 + register_defaults
  - `builtins/builtins_attr.rs` - attr / content / has_attr
  - `builtins/builtins_keyword.rs` - keyword_get/attr/has/keys/values/pairs
  - `builtins/builtins_scalar.rs` - list_length / to_string / parse_bool / parse_int
  - `builtins/builtins_module.rs` - caller_env / require / import / alias / invoke_macro
  - `builtins/builtins_ast_read.rs` - ast_head / children / attr_get / attr_keys
  - `builtins/builtins_ast_write.rs` - ast_attr_set / wrap / concat / filter_head
  - `builtins/builtins_module_data.rs` - module_get / put / update
  - `builtins/builtins_list.rs` - list / list_concat / foreach / map / fold / match
- 状态：**完成** (make gate 通过，330 测试全通过，覆盖率 89.78%)
- `builtins.rs` 改为 facade，声明模块树并 re-export BuiltinRegistry + BuiltinResult

### Round 3: 拆分 REPL
- [ ] 将 `sl-repl/src/lib.rs` 拆分为：
  - `session.rs` - ReplSession 结构和方法
  - `commands.rs` - 命令处理
  - `inspector.rs` - 状态检查工具

### Round 4: 代码质量改进
- [ ] 检查并优化 clone 调用
- [ ] 提取重复模式
- [ ] 统一错误处理

---

## 约束条件

1. **每轮独立提交**：每轮优化完成后单独提交
2. **测试必须通过**：每次提交前运行 `make gate`
3. **无功能变更**：只做重构，不改变行为
4. **保持向后兼容**：公共 API 不变

---

## 验证流程

每轮完成后执行：
```bash
make gate
```

确保全部通过：
- [ ] `cargo check --workspace`
- [ ] `cargo fmt --all --check`
- [ ] `cargo test --workspace -q`
- [ ] `cargo clippy --workspace`
- [ ] `cargo llvm-cov` (覆盖率 ≥ 89.9%)

---

## 进度记录

### Round 1 - 拆分 macro_lang/tests.rs ✅ (2026-03-25)

**本次做了什么：**
- 将 6171 行的 `tests.rs` 拆分为 4 个文件：
  - `tests/tests_helpers.rs`：共享 `use` 导入 + `empty_macro_env`/`empty_expand_env` 工具函数
  - `tests/ct_lang_tests/mod.rs`：父模块，重新导出 helpers
  - `tests/ct_lang_tests/tests_ct_eval.rs`（740 行，27 个测试）：基本 eval + 核心 builtin 测试
  - `tests/ct_lang_tests/tests_convert.rs`（1781 行，93 个测试）：Form→CtStmt/CtExpr 转换测试
  - `tests/ct_lang_tests/tests_builtins.rs`（3172 行，38 个测试）：高级 builtin 测试（AST/module/list/keyword/match）
- 原始 `tests.rs`（6171 行）已删除

**本次发现的问题/踩的坑：**

1. **Rust `#[path]` 路径解析陷阱**：`#[path = "foo.rs"]` 的路径是相对于**声明所在文件**（而非模块）的目录。例如 `tests/mod.rs` 中的 `#[path = "tests_ct_eval.rs"]` 会被解析为 `tests/tests_ct_eval.rs`（正确），但若在 `tests/mod.rs` 的 `mod ct_lang_tests {}` 内部声明则会创建 `tests/ct_lang_tests/` 子目录，导致路径错误。

2. **模块嵌套 vs 文件即模块体**：用 `#[path]` 时，文件内容是模块的**函数体**，不是嵌套的 `mod foo { }`。若文件内还有 `mod foo { }` 会导致嵌套模块——多余且错误。

3. **`pub use` 不能重导出 `pub(crate)` 条目**：`ExpandEnv` 和 `MacroEnv` 是 `pub(crate)`，不能用 `pub use` 重导出。这是 Rust 可见性规则的硬限制。

4. **`use` 导入的可见性传递**：子模块不能通过父模块的私有 `use` 获取类型。解决方案是让每个子模块自己 `use` 需要的类型。

5. **每个测试函数必须有所有必要的 `use`**：不能用"继承"的思路期望子模块自动获取父模块的 `use` 导入。

6. **保留 `#[cfg(test)]` 属性**：原来 `tests.rs` 顶层的 `#[cfg(test)] mod ct_lang_tests { ... }` 保留在 `tests/mod.rs` 中，确保所有子模块都在 test 配置下编译。

**对后续有价值的经验：**
- Rust 模块拆分最佳实践：创建 `tests/` 目录，`tests/mod.rs` 作为入口，用 `#[path]` 指向兄弟文件，文件内容直接是函数定义（不加额外的 `mod` 包裹）
- 若需要在多个测试文件中共享 `use` 导入，创建 `tests_helpers.rs` 作为兄弟模块，在需要的地方 `use super::tests_helpers::*;` 或直接 `use crate::...`
- 拆分大测试文件时，用 `#[path]` 时文件是模块体不是嵌套模块；用 `mod foo;` 声明时 Rust 会自动查找 `foo.rs` 或 `foo/mod.rs`

**下一步方向：**
- Round 2: 拆分 `builtins.rs`（2329 行，按类别分为 registry/attr/keyword/list/module/ast）

### Round 2 - 拆分 builtins.rs ✅ (2026-03-25)

**本次做了什么：**
- 将 2329 行的 `builtins.rs` 拆分为 9 个文件：
  - `builtins/builtins_registry.rs`（156 行）：`BuiltinRegistry` 结构体 + `register_defaults`
  - `builtins/builtins_attr.rs`（187 行）：`attr` / `content` / `has_attr`
  - `builtins/builtins_keyword.rs`（306 行）：`keyword_get`/`attr`/`has`/`keys`/`values`/`pairs` + `convert_macro_value_to_ct_value`
  - `builtins/builtins_scalar.rs`（126 行）：`list_length` / `to_string` / `parse_bool` / `parse_int`
  - `builtins/builtins_module.rs`（634 行）：`caller_env`/`module`/`expand_alias`/`require_module`/`define_import`/`alias`/`require`/`invoke_macro`
  - `builtins/builtins_ast_read.rs`（187 行）：`ast_head`/`children`/`attr_get`/`attr_keys` + helpers
  - `builtins/builtins_ast_write.rs`（340 行）：`ast_attr_set`/`wrap`/`concat`/`filter_head` + helpers
  - `builtins/builtins_module_data.rs`（115 行）：`module_get`/`put`/`update`
  - `builtins/builtins_list.rs`（350 行）：`list`/`list_concat`/`foreach`/`map`/`fold`/`match` + callbacks
- `builtins.rs` 改为 facade（25 行），声明模块树并 re-export `BuiltinRegistry` + `BuiltinResult`
- 原始 `builtins.rs`（2329 行）内容已迁移

**本次发现的问题/踩的坑：**

1. **`mod foo` 会匹配同名目录**：在 `builtins.rs` 中声明 `mod builtins_attr;` 时，Rust 会尝试找 `builtins/builtins_attr.rs`（子目录），而非 `builtins_attr.rs`（同级文件）。解决方案：用 `#[path = "builtins_attr.rs"] mod builtins_attr;`，或者将子模块放入实际子目录（如 `builtins/`）并让 `builtins.rs` 声明 `mod builtins;`（目录名与文件名无冲突时）。

2. **`#[path]` 路径相对于声明文件所在目录**：指定 `#[path = "builtins_attr.rs"]` 时，路径相对于 `builtins.rs` 所在的目录，而非相对于包含 `builtins` 模块的父目录。

3. **模块可见性与 `pub use` 重导出**：用 `#[path]` 声明的模块是 `mod`（private），无法用 `pub use X as Y` 重导出。解决方案：要么用 `pub mod`，要么只在 facade 保留必要的公开 API（如 `BuiltinRegistry`），其他子模块对外部完全隐藏。

4. **Rust 模块内联 vs 文件分离**：模块内可以声明 `pub use X;` 来重导出子模块内容，但子模块本身必须是 `pub`（或用 `#[path]` 后再 `pub`）。

5. **循环依赖陷阱**：`BuiltinFn` 类型需要 `BuiltinRegistry` 作为参数，而 `BuiltinRegistry` 需要 `BuiltinFn` 类型。解决：把两者都放在 `builtins_registry.rs` 中定义，用绝对路径 `crate::` 在其他子模块中引用它们。

**对后续有价值的经验：**
- Rust 中同一 crate 的模块拆分有两条路：目录树（`mod foo;` → `foo/mod.rs`/`foo.rs`）或 `#[path]` 强制路径。前者更标准，后者用于将文件拆分但不让目录名与文件名冲突。
- 拆分大文件时的模块层级：`crate/semantic/macro_lang/builtins.rs`（facade）+ `crate/semantic/macro_lang/builtins/*.rs`（子模块），`#[path]` 让 `mod foo;` 声明指向 `builtins/foo.rs`。
- `pub(crate)` 类型的可见性：子模块间通过 `crate::` 路径互相引用，不需要 `pub`。
- 拆分时保持一个"facade"文件作为入口，声明所有子模块并只暴露必要的公开 API，可以让外部调用者无感知变化。

**下一步方向：**
- Round 3: 拆分 `sl-repl/src/lib.rs`（1962 行，按 session/commands/inspector 拆分）
- Round 2 完成后，>800 行文件从 8 个减少到 1 个，可维护性显著提升

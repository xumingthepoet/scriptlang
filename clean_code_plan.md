# Clean Code Plan

## 目标
以 Clean Code 为目标，系统性地优化代码质量。每轮独立寻找优化点，实施后保证测试通过再提交。

## 当前代码库状态

### 统计数据
- 总 Rust 文件：63 个
- >400 行的文件：22 个
- >800 行的文件：8 个
- 总代码行数：~30,000+

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
- [ ] 将 `macro_lang/tests.rs` 按功能拆分为：
  - `tests_ct_eval.rs` - 常量求值测试
  - `tests_ct_expr.rs` - 表达式测试
  - `tests_ct_stmt.rs` - 语句测试
  - `tests_builtins.rs` - 内置函数测试

### Round 2: 拆分 Builtins
- [ ] 将 `builtins.rs` 按类别拆分：
  - `builtins_registry.rs` - 注册表结构
  - `builtins_attr.rs` - 属性操作
  - `builtins_keyword.rs` - 关键字操作
  - `builtins_list.rs` - 列表操作
  - `builtins_module.rs` - 模块操作
  - `builtins_ast.rs` - AST 操作

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

# Next Plan

当前优先推进 4 件事，顺序按“先编译期环境，再新增值类型，最后补循环构造”安排。

## 1. Sub Module

Status: completed

目标：

- 支持像旧项目一样的 sub module
- 但实现方式保持当前架构方向：module 结构只存在于编译期，不进入 runtime

方案：

- 在 `semantic::expand` 阶段支持嵌套 `<module>`
- 将子模块展平成限定名 module
- 例如：
  - `main` 下的 `helper`
  - 展开后得到 `main.helper`
- 嵌套深度不限，`main.helper.grand` 这类多级子模块递归展平
- 父模块会自动获得直接子模块的词法短名 alias
  - 例如 `main` 可直接写 `@helper.entry`
  - `main.helper` 可直接写 `@grand.entry`
- `ProgramState`、module catalog、import/alias/require 查询都统一基于展平后的完整 module 名

预期收益：

- 更接近 Elixir 的 module 组织方式
- 为后续 `alias`、`require`、`function literal` 提供稳定的 module path 基础

## 2. Require / Alias

Status: completed

目标：

- 支持类似 Elixir 的 `require` 与 `alias`
- 与现有 `import` 明确分工，不再把所有能力混在 `import` 里

方案：

- `require`
  - 只负责宏可见性
  - 不自动引入 const / var / script / function 的短名可见性
- `alias`
  - 只负责 module 名缩写
  - 可用于 script / function / const / var 的显式限定引用
- `import`
  - 保持成员可见性导入职责
- 三者都只在 `ExpandEnv` / `ProgramState` / scope 查询层生效，不进入 runtime

预期收益：

- macro 可见性模型更接近 Elixir
- 名字解析边界更清楚
- 为 `#foo` / `@m.loop` / 未来更多限定引用提供一致规则

## 3. Function / `type="function"` / `#foo`

Status: completed

目标：

- 添加 `<function>`
- 添加 `type="function"`
- 使用 `#foo` 或 `#m.foo` 作为 function literal

方案：

- 第一阶段先把 function 当“可传递引用值”实现，不急着引入复杂调用语义
- 在 compiler 中支持：
  - `<function name="...">`
  - `type="function"`
  - `#foo` / `#m.foo` function literal
  - 编译期校验 function 是否存在
- runtime 第一阶段继续像 `script` 一样用字符串 key 承载 function 值
- 若后续需要 expr 内直接调用 function value，再单独设计 runtime 调用桥

预期收益：

- 与现有 `script` 值模型保持一致
- 先建立稳定的 function value 语义，再决定调用模型

## 4. While

Status: completed

目标：

- 支持 `while` 循环
- 支持 `<break>` 和 `<continue>`
- 保持 runtime 边界尽量不变

方案：

- 不通过 macro 实现第一版 `while`
- 先像 `if` 一样，把 `<while>` 做成 builtin 高层构造
- `<break>` / `<continue>` 也作为 builtin statement 构造处理
- runtime 不新增新的公开语言关键字；主要在 compiler 内完成 lowering
- `assemble` 将 `while` / `break` / `continue` lower 成现有跳转指令：
  - `EvalCond`
  - `JumpIfFalse`
  - `Jump`
- `continue` 跳回当前 loop head
- `break` 跳到当前 loop exit
- lowering 时需要维护 loop stack，用于解析嵌套 `while` 中的 `break` / `continue`

预期收益：

- runtime 边界保持稳定
- 用户直接拥有清晰的循环能力，不必暴露低层 jump 语法
- 后续如果要把 `if`、`while` 等控制流进一步统一，也可以建立在同一套 compiler-internal 控制流 lowering 上

## Recommended Order

1. 先做 sub module
2. 再做 require / alias
3. 然后做 function value
4. 最后做 while

## Notes

- `sub module`、`require`、`alias` 都属于编译期环境能力，优先级高于新值类型和循环
- `function` 第一阶段先解决“引用值”，不要过早把 runtime 调用模型做重
- `while` 第一阶段直接作为 builtin 做稳，再考虑未来是否把更多控制流统一到更抽象的编译期表示

## 5. Experiment: Replace `if` With Kernel Macro Based On `while`

Status: completed

目标：

- 在 `while` / `break` / `continue` builtin 稳定后，探索把 `<if>` 从 builtin 下沉为 kernel macro
- 验证控制流构造是否能更多地回收到标准库宏层

方案：

- 保留 compiler 内部稳定的循环和跳转 lowering
- 在 `kernel.xml` 中尝试将 `<if>` 重写为基于 `while` 的标准宏
- 评估：
  - 可读性是否仍可接受
  - 宏展开后的结构是否清晰
  - hygiene 和局部 temp 引入是否足够稳
  - 是否会让调试、错误定位和后续维护显著变差

预期收益：

- 进一步验证“高层语言构造优先由编译期扩展承载”的路线
- 判断 `if` 是否可以从 builtin 缩减为标准库宏

注意：

- 这是第 4 项完成后的探索任务，不作为当前 `while` 第一版的前置条件
- 如果实验结果不理想，`if` 可以继续保留为 builtin

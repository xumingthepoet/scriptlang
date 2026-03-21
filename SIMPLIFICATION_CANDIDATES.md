# Simplification Candidates

本文档只记录当前代码库里“值得删减/收缩”的候选点，不代表这些点都已经实现。

判断标准只有两个：

- 能否让代码更简单、特例更少、边界更清晰
- 是否符合 Elixir 的实现精神：把复杂度尽量收在编译期环境、展开和统一语义里，而不是把很多分散特例长期留在 compiler / runtime 边界上

## 现在就该删

### 1. 收缩 `MacroEnv` 到真实需要的字段

当前 [`macro_env.rs`](/Users/xuming/work/scriptlang-new/crates/sl-compiler/src/semantic/expand/macro_env.rs) 里有一批上下文字段：

- `imports`
- `requires`
- `aliases`
- `context_label()`

它们目前主要服务错误文案和测试断言，并没有参与真实的 macro evaluator 语义。

当前真正对宏求值有用的核心字段只有：

- `current_module`
- `macro_name`
- `attributes`
- `content`
- `locals`
- gensym 状态

所以这批未被真实语义消费的上下文可以优先删掉。

这符合 Elixir 的精神：环境对象应该承载真实影响展开行为的状态，而不是先保存一批暂时没用的上下文。

### 2. 删除 `__sl_loop_capture` 这种内部属性泄漏

当前 `if / unless / if-else` 宏是通过 `while` 实现的，但 kernel 宏还需要显式写：

- `__sl_loop_capture="false"`

相关位置：

- [`kernel.xml`](/Users/xuming/work/scriptlang-new/crates/sl-api/lib/kernel.xml)
- [`scripts.rs`](/Users/xuming/work/scriptlang-new/crates/sl-compiler/src/semantic/expand/scripts.rs)

这说明 compiler 的内部 lowering 细节还泄漏到了表层 form 语法。

更合理的收缩方向是：

- 改成 compiler-only 内部构造
- 或改成内部语义节点

而不是继续让 kernel 宏显式携带这个属性。

这也符合 Elixir 的精神：高层宏和标准库看到的是高层结构，低层控制流细节不应长期暴露在表层 AST 里。

### 3. 合并 `script` / `function` literal 的双份实现

当前：

- `@...` script literal
- `#...` function literal

几乎是两套平行实现，逻辑分散在：

- [`names.rs`](/Users/xuming/work/scriptlang-new/crates/sl-compiler/src/names.rs)
- [`modules.rs`](/Users/xuming/work/scriptlang-new/crates/sl-compiler/src/semantic/expand/modules.rs)
- [`scope.rs`](/Users/xuming/work/scriptlang-new/crates/sl-compiler/src/semantic/expand/scope.rs)
- [`const_eval.rs`](/Users/xuming/work/scriptlang-new/crates/sl-compiler/src/semantic/expand/const_eval.rs)
- [`expr/rewrite.rs`](/Users/xuming/work/scriptlang-new/crates/sl-compiler/src/semantic/expr/rewrite.rs)

这非常适合收成一套统一的“命名字面量”机制，只保留：

- 短名/限定名/alias 归一化
- module/member 存在性校验
- 重写为最终 runtime 值

这符合 Elixir 的精神：统一 dispatch、统一名字解析和统一环境消费，而不是为同构能力复制两份逻辑。

## 有前提再删

### 4. 评估是否从全局 `dispatch` 里删除 `option`

当前 [`dispatch.rs`](/Users/xuming/work/scriptlang-new/crates/sl-compiler/src/semantic/expand/dispatch.rs) 把 `option` 当成 statement-scope builtin 处理。

但 `option` 只有在 `choice` 下才合法，最终消费点也只在 [`scripts.rs`](/Users/xuming/work/scriptlang-new/crates/sl-compiler/src/semantic/expand/scripts.rs) 的 `choice` 分支里。

如果后续把 `choice` 子树的 rewrite 彻底下沉到 `choice` 自己处理，那么 `option` 有机会从全局 dispatch 特例里删除。

这项收缩是合理的，但优先级低于前面三项。

### 5. 评估是否删除部分 convenience kernel macro

当前 kernel 里有一些示范/便利宏，例如：

- `when_text`
- `script_text`

这些不是内核必须能力，只是标准库糖。

如果目标是“最小语言基线”，可以删除。
如果目标是“保留少量标准宏示例”，也可以继续保留。

这类点不属于必须马上清理的复杂度。

### 6. 继续收缩 `form.rs` 里的硬编码槽位 helper

当前 [`form.rs`](/Users/xuming/work/scriptlang-new/crates/sl-compiler/src/semantic/form.rs) 里还保留了：

- `body_expr`
- `body_template`
- `trimmed_text_items`

以及一部分按标签分类的硬编码逻辑。

如果后续继续向 env-driven expand 收拢，这部分可以再缩，但它不是现阶段最该动的地方。

## 现在不建议删

### 7. 不建议现在删除静态 `JumpScript`

当前：

- `JumpScriptExpr` 服务用户级动态 `<goto>`
- `JumpScript` 仍服务 compiler-internal 的 boot/default-entry 跳转

虽然动态跳转已经存在，但这不自动意味着静态跳转就是历史残留。

按当前架构原则：

- 能在编译期静态确定的，就不必故意降级成运行时动态求值

所以 `JumpScript` 现在不建议删除。

这也符合 Elixir 的精神：不是原语种类越少越好，而是静态能确定的事情应当尽量在编译期确定。

### 8. 不建议删除 `child_aliases`

[`child_aliases`](/Users/xuming/work/scriptlang-new/crates/sl-compiler/src/semantic/env.rs) 当前不是冗余字段。

它承载了：

- 父 module 可直接从子 module 短名开始引用

并且在 [`scope.rs`](/Users/xuming/work/scriptlang-new/crates/sl-compiler/src/semantic/expand/scope.rs) 中被真实消费。

删掉它会退化当前语言行为，而不是让实现更抽象。

### 9. 不建议把 `import / require / alias` 再合并回去

当前三者职责已经比较清楚：

- `import`：成员短名可见性
- `require`：宏可见性
- `alias`：模块名缩写

这比早期“统一糊成一种编译期上下文动作”更接近 Elixir 的分层思路。

所以这里不建议为了“表面更少概念”而重新合并。

## 推荐的收缩顺序

如果接下来真的要继续做“能删就删”的整理，建议顺序是：

1. 收缩 `MacroEnv` 未使用字段
2. 干掉 `__sl_loop_capture` 对表层 form 的泄漏
3. 合并 `script/function literal` 双份实现
4. 评估 `option` 是否能从全局 dispatch 下沉
5. 最后再决定是否删掉部分 convenience kernel macro

## 总结

最符合 Elixir 精神的“简化”，不是把能力砍掉，而是：

- 删重复
- 删泄漏
- 删无效上下文
- 删全局特例

当前最值钱的三个删减点是：

1. `MacroEnv` 冗余上下文
2. `__sl_loop_capture`
3. `script/function literal` 双轨逻辑

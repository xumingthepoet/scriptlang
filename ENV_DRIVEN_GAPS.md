# Env-Driven Gaps

本文档专门记录当前实现中，哪些点还不符合 Elixir 式的 env-driven expand 思路。

这里说的 “Elixir 式” 不是指表层语法模仿，而是指：

- `alias / require / import` 在 expand 过程中顺序生效
- env 是真实驱动后续语义的唯一事实来源
- 名字解析、宏分派、局部绑定和上下文判断尽量在 expand 过程中完成
- 高层 form 的语义主要通过 env 推进和结构重写建立，而不是靠后置并列阶段补救

本文件只描述当前偏差点，不代表这些点都必须立刻修复。

## 1. `MacroEnv` 里的 env 还不是“活环境”

当前 [`macro_env.rs`](/Users/xuming/work/scriptlang-new/crates/sl-compiler/src/semantic/expand/macro_env.rs) 里保存了：

- `current_module`
- `imports`
- `requires`
- `aliases`
- `attributes`
- `content`
- `locals`

但其中真正被宏求值逻辑实质消费的，主要只有：

- `attributes`
- `content`
- `locals`
- gensym 相关状态

`imports / requires / aliases` 当前更多只是上下文拷贝和错误文案素材，而不是驱动宏行为的活环境。

这和 Elixir 不一致。Elixir 的 `Macro.Env` 里这些字段之所以存在，是因为：

- 它们在 expand 期间会被持续更新
- 之后的 alias / require / import 分派、宏调用和名字解析会真实消费这些字段

当前缺口：

- 这些字段还没有真正驱动 macro evaluator 的语义
- 宏体也还没有能显式消费这些环境能力的 compile-time API

## 2. `alias / import / require` 仍然分散在多个 helper 中生效

当前这三者已经开始在 expand 阶段按源码顺序生效，但影响路径仍然比较分散：

- [`module.rs`](/Users/xuming/work/scriptlang-new/crates/sl-compiler/src/semantic/expand/module.rs)
- [`imports.rs`](/Users/xuming/work/scriptlang-new/crates/sl-compiler/src/semantic/expand/imports.rs)
- [`scope.rs`](/Users/xuming/work/scriptlang-new/crates/sl-compiler/src/semantic/expand/scope.rs)
- [`program.rs`](/Users/xuming/work/scriptlang-new/crates/sl-compiler/src/semantic/expand/program.rs)

这意味着：

- env 的推进逻辑和 env 的消费逻辑还没有完全收成一个中心
- 某些名字解析/可见性判断仍然更像“后续 helper 查询”，而不是 expand 期间直接依赖当前 env 决定

和 Elixir 相比，当前实现更像：

- expand 已经开始推进 env
- 但后续很多语义仍通过专门 catalog / resolver helper 回头查询

而不是：

- expand 当下就决定后续 form 的解释方式

## 3. `scope.rs` 仍然偏重“后置查询器”而不是“当前 env 的直接视图”

[`scope.rs`](/Users/xuming/work/scriptlang-new/crates/sl-compiler/src/semantic/expand/scope.rs) 当前职责很重：

- alias 解析
- import 可见性
- const catalog
- var/const/function/script 名字解析
- child module alias 可见性

这会导致一个问题：

- env 虽然在推进
- 但真正大量决定语义的仍是一个集中 resolver / catalog 层

这和 Elixir 那种“expand 过程中 env 自身就是语义中心”的方式仍有差距。

更接近 Elixir 的方向应该是：

- `scope` 更像 env 的轻量视图
- 而不是一个重量级二次语义中心

## 4. 宏体仍然不是“小型编译期语言”，只是最小 evaluator

当前 macro evaluator 支持的 compile-time forms 只有：

- `<let>`
- `<get-attribute>`
- `<get-content>`
- `<quote>`
- `<unquote>`

相关实现：

- [`macro_eval.rs`](/Users/xuming/work/scriptlang-new/crates/sl-compiler/src/semantic/expand/macro_eval.rs)
- [`quote.rs`](/Users/xuming/work/scriptlang-new/crates/sl-compiler/src/semantic/expand/quote.rs)

这说明当前宏系统虽然已有 `MacroEnv`，但还没达到 Elixir 那种：

- 宏本身就是一段真正会运行的编译期逻辑
- `quote/unquote` 只是 AST 构造机制

当前缺口：

- 没有 compile-time control flow
- 没有真正的 compile-time function/builtin 调度层
- 没有显式的 env 查询 API

所以目前更像“带局部绑定能力的 AST 模板求值器”，还不是更完整的 env-driven macro execution model。

## 5. 局部变量 / hygiene 仍然是后补式，而不是 env 的一等成员

当前最小 hygiene 已经有了，见：

- [`quote.rs`](/Users/xuming/work/scriptlang-new/crates/sl-compiler/src/semantic/expand/quote.rs)

但它主要还是：

- quote 内 `<temp>` 引入名的 gensym
- expr 中局部变量名的同步改写

这和 Elixir 的：

- `versioned_vars`
- lexical tracking
- env 中显式变量版本信息

相比仍然偏弱。

当前缺口：

- 宏展开中的变量环境没有作为 env 一等成员统一表示
- 变量 hygiene 还不是 env/versioning 驱动，而是 quote 时定点改写

## 6. `form.rs` 仍保留一部分“按标签判断槽位语义”的旧痕迹

[`form.rs`](/Users/xuming/work/scriptlang-new/crates/sl-compiler/src/semantic/form.rs) 里仍然存在：

- `body_expr`
- `body_template`
- `trimmed_text_items`

以及按标签硬编码的一些规则。

这说明当前“某个槽位是什么语义”这件事还没有完全纳入 env-driven expand 过程，而仍有一部分依赖 helper 里的标签分类。

这不算严重错误，但和 Elixir 式的“expand 过程决定结构语义”相比，仍然是老架构残留。

## 7. 内部 lowering 细节还会泄漏到高层宏实现

最典型的例子就是曾经存在的：

- `__sl_loop_capture`

虽然这条现在已经被纳入 simplification candidates，但它也体现了一个 env-driven gap：

- 高层宏实现还需要知道部分 compiler 内部 lowering 细节

Elixir 精神更倾向于：

- 高层宏只操作高层 AST
- lowering 细节藏在后面的统一编译阶段

## 8. `choice / option` 仍然体现出局部结构语义没有完全收进 expand 中心

当前：

- `option` 在 global expand dispatch 中仍然是一个 builtin case
- 但它只在 `choice` 下才有意义

这说明：

- 某些结构性语义还没有完全就地封装在所属 form 的 expand/analysis 逻辑里
- 而是通过全局 dispatch 做了一层分散特判

这和 Elixir 式“按构造职责拆分，并由 expand 中心调度”相比仍然不够收敛。

## 9. `script/function` literal 解析仍是“功能平行复制”，不是统一 env 机制

当前：

- `@...`
- `#...`

分别对应 script/function literal，但解析、校验、rewrite 的实现高度平行。

这代表：

- env 已经足够表达“某个 module 下某个命名成员是否存在”
- 但实现层还没有把这件事抽成统一机制

这不只是代码重复问题，也说明 env-driven 的抽象还没有完全形成。

## 当前最关键的偏差总结

如果只提炼 3 个当前最关键、最影响后续演进的 gap，是这三个：

1. `alias / import / require` 还没有彻底收成“expand 期间活环境”
2. `scope.rs` 仍然过于像后置 resolver 中心
3. 宏系统还不是更完整的 compile-time language / env execution model

## 最合理的收敛顺序

建议后续如果继续向 Elixir 思路收敛，顺序如下：

1. 先把 `alias / import / require` 的 env 推进与消费进一步统一
2. 再收缩 `scope.rs`，让它更像 env 的轻量视图，而不是二次语义中心
3. 再扩 macro evaluator，使其更像真正的小型编译期语言
4. 最后继续清理 `form.rs`、`option` dispatch、literal 双轨实现等残余特例

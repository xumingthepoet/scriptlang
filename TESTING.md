# Testing Guide

本文档只描述 `scriptlang-new` 当前已经落地的测试结构、执行方式和测试约束。

测试目标不是“把 gate 糊过去”，而是验证当前架构分层是否成立，并保证 compiler / runtime 边界在重构中保持稳定。

## 1. 当前测试结构

当前仓库测试分两层：

- crate 内单元测试
  - 放在各自源文件内的 `#[cfg(test)] mod tests`
  - 用来直接覆盖该文件自己的函数、分支和错误路径
- workspace 级 examples 集成测试
  - 位于 `crates/sl-integration-tests`
  - 通过 `sl-api` 串起 parser / compiler / runtime
  - 用 example case 验证跨 crate 的真实行为

当前 examples runner 入口：

- `crates/sl-integration-tests/tests/examples.rs`
- `crates/sl-integration-tests/tests/support.rs`

当前 example 目录结构：

```text
crates/sl-integration-tests/examples/<example>/
  xml/
    *.xml
  runs/<case>/
    actions.txt
    results.txt
    error.txt
```

含义如下：

- `xml/`：该 example 对应的全部输入源文件
- `runs/<case>/results.txt`：期望的可见输出序列
- `runs/<case>/actions.txt`：运行时驱动动作；没有动作时可省略
- `runs/<case>/error.txt`：编译失败场景的期望错误片段

当前支持的动作：

- `choose N`
- `snapshot-progress N`
- `snapshot-on-choice`
- `resume-snapshot`

## 2. Gate 与执行命令

统一入口是根目录 `Makefile`：

- `make check`
- `make fmt`
- `make test`
- `make lint`
- `make coverage`
- `make gate`

当前 `make gate` 顺序为：

1. `cargo check --workspace`
2. `cargo fmt --all --check`
3. `cargo test --workspace -q`
4. `cargo clippy --workspace --all-targets --all-features -- -D warnings`
5. `cargo llvm-cov --package sl-core --package sl-parser --package sl-compiler --package sl-runtime --lib --fail-under-lines 99 --fail-under-functions 100`

这里的标准是硬门禁，不是参考值：

- `fmt` 必须过
- `clippy -D warnings` 必须过
- `coverage` 必须达到 line `90%` / function `90%`

## 3. 新增或修改代码时的测试要求

默认要求：

- 改动了某个非平凡源文件，就为该文件补直接测试
- 改动涉及 parser / compiler / runtime 交界行为时，同时补 example 集成用例
- 修 bug 时，优先补能稳定复现问题的回归测试，再修实现
- 集成测试只负责验证跨层行为，不替代对应文件自己的直接单元测试

实操上可以这样理解：

- 单元测试负责“这个文件自己的逻辑是否完整可证”
- examples 测试负责“从 XML 到运行结果的整条链路是否成立”

## 4. 禁止通过作弊方式过 Gate

禁止为了让 `test`、`clippy`、`coverage` 通过而引入规避手段。包括但不限于：

- 用宏、条件编译或包装层跳过真实逻辑执行
- 新增 `#[ignore]`、`#[allow(...)]`、`#[expect(...)]` 只为压住当前问题
- 排除应当被覆盖的代码
- 故意不跑 `fmt`，让大量逻辑挤在一行，借此规避 line coverage
- 把复杂逻辑塞进难以追踪的宏展开里，只让表面调用点计数好看
- 用测试专用分支、测试专用捷径、测试专用 runtime 行为替代真实实现
- 用间接覆盖冒充直接防御：A 文件的测试跑到了 B 文件，不算 B 已被充分测试

原则只有一个：

> gate 失败时，优先修设计、修实现、补测试；当前 gate 的 coverage 阈值是 line `90%` / function `90%`。

## 5. 测试代码与生产代码边界

测试相关代码和宏不能进入原始代码区域。

具体要求：

- 测试辅助代码只能放在 `#[cfg(test)]` 测试模块或独立测试 crate 中
- 不要把 test helper、test-only macro、test-only branch 暴露到正常编译路径
- 不要为了方便测试，在生产 API 上添加只服务测试的入口、参数或状态
- runtime / compiler / parser 的正式语义不能依赖“测试模式”才能成立

如果一段逻辑只有在测试里才需要存在，它就不应该进入生产构建产物。

## 6. 编写测试时的判断标准

可以参考老项目的测试纪律，但以当前仓库现状为准。当前仓库至少要满足：

- 每个非平凡文件有自己的直接测试
- 每个新增分支和错误路径都有明确断言
- 回归场景尽量落到 `crates/sl-integration-tests/examples`
- 断言具体结果、错误文本或状态变化，不写过于宽松的“只要失败就行”
- 任何改动完成后，以 `make gate` 作为最终验收标准

如果某段代码很难测试，默认先判断是不是设计出了问题，而不是先想怎么绕过 coverage。

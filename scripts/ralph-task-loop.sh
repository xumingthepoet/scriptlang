#!/usr/bin/env bash

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
cd "${REPO_ROOT}"

child_pid=""
interrupted=0
CLAUDE_LOOP_TEST_CMD="${CLAUDE_LOOP_TEST_CMD:-}"
BACKOFF_SLEEP_SECONDS="${BACKOFF_SLEEP_SECONDS:-300}"

TASK_DOC_PATH="${1:-}"
MAX_ROUNDS="${2:-10}"

usage() {
  cat <<'EOF'
Usage:
  scripts/ralph-task-loop.sh TASK_DOC_PATH [rounds>=1]

Behavior:
  - Run a Ralph-style loop toward the given task document.
  - Each round sends exactly two independent prompts to Claude (two separate `claude -p` calls, no conversation continuity).

Examples:
  scripts/ralph-task-loop.sh docs/tasks/repl-plan.md
  scripts/ralph-task-loop.sh docs/tasks/repl-plan.md 20
EOF
}

if [[ -z "${TASK_DOC_PATH}" ]] || [[ "${TASK_DOC_PATH}" == "-h" ]] || [[ "${TASK_DOC_PATH}" == "--help" ]]; then
  usage
  exit 0
fi

if ! [[ "${MAX_ROUNDS}" =~ ^[0-9]+$ ]] || [[ "${MAX_ROUNDS}" -lt 1 ]]; then
  echo "Usage: $0 TASK_DOC_PATH [rounds>=1]" >&2
  exit 1
fi

if [[ ! -f "${TASK_DOC_PATH}" ]]; then
  echo "error: task document not found: ${TASK_DOC_PATH}" >&2
  exit 1
fi

if [[ -z "${CLAUDE_LOOP_TEST_CMD}" ]] && ! command -v claude >/dev/null 2>&1; then
  echo "claude CLI is required for scripts/ralph-task-loop.sh" >&2
  exit 1
fi

TASK_DOC_ABS="$(cd "$(dirname "${TASK_DOC_PATH}")" && pwd)/$(basename "${TASK_DOC_PATH}")"
CURRENT_TIME="$(date '+%Y-%m-%d %H:%M:%S %Z')"

PROMPT_ROUND_1="当前时间：${CURRENT_TIME}。

## 任务
阅读 \`${TASK_DOC_ABS}\`，按文档中的当前进度执行下一条任务。不要跳步，不要空转，不要偏离任务做无关整理。

## 工作区状态（大循环中断恢复）
你在一个大循环里，每轮只完成一个具体步骤，然后暂停等待下一轮。

检查工作区状态：
- **如果 \`${REPO_ROOT}/current_task_actions.md\` 存在**：这是上一次中断留下的任务记录，先读取它了解上一步准备做什么，然后继续完成它。不要跳过或无视这个文件。
- **如果工作区有未提交变更**：用 git diff 和 git status 仔细评估：这些改动是否能接上本次任务？是部分有用还是完全无用？
  - **部分有用**：保留有用部分，commit 或 stash，继续在此基础上推进。
  - **完全无用或冲突**：git reset --hard 丢弃，回到干净状态。

## 提交
- 有实质进展时 git commit；如果本轮决定跳过（没有任何需要修改的内容），也要在 \`${REPO_ROOT}/current_task_actions.md\` 里记录"跳过原因"，然后视为本轮完成。
- 如果本轮改动涉及 crates/ 或会影响 crate 行为/接口/测试结果/编译流程，必须同步更新 IMPLEMENTATION.md，并在本轮结束前跑通 make gate。

## 绝对禁止
- **永远不要 git push**。本循环只做 local commit，不推送到任何远程仓库。

## 进度记录
本轮任务执行完成后，把本次工作进度追加到 \`${TASK_DOC_ABS}\` 末尾。包括：
- 当前步骤标题
- 本次做了什么
- 本次发现的问题、踩的坑、或对后续有价值的经验教训（不要记流水账，要记有营养的）
- 下一步方向

## 当前操作记录（断点恢复用）

**此文件永远不会进入 git 提交。永远不。**

中断是产生漂移的最主要原因：当 agent 因错误退出而新 agent 接手时，如果没有任何记录，接手的 agent 不知道上一步在做什么，容易跳步或重做已经做了一半的工作。

因此，在每次修改文件之前，必须先在 \`${REPO_ROOT}/current_task_actions.md\` 追加一条记录：

\`\`\`
### [YYYY-MM-DD HH:MM] Round N - 准备修改
- 文件路径:简要说明（如：crates/parser/src/lib.rs: 添加 expr_parser 函数）
\`\`\`

如果该文件不存在就先创建，如果已存在就追加。

**每次 git commit 之前，必须先删除此文件，然后再 commit。绝不能让它进入版本库。**

## 工具
优先用 TODO 工具分配任务给 subagent，分担上下文压力。"

PROMPT_ROUND_2="当前时间：${CURRENT_TIME}。

## 角色
你是大循环中的第二步（审查步），审查第 __ROUND_NUM__ 轮的 prompt1（执行步）。

**你最重要的职责是防止 prompt1 漂移**：确保它在做 task doc 里当前该做的事，而不是跳步、缩水、做无关的事。格式/记录检查只是兜底。

## Round 1 完整提示词原文
以下是你要检查的全部要求，逐一核对是否被执行：

---
\${PROMPT_ROUND_1}
---

## 检查清单

### 第一优先级：防漂移检查（最重要）

**步骤对照**：先读取 \`${TASK_DOC_ABS}\`，找出当前应该做的步骤。然后对照本轮 git commit 的内容：
- 本轮改动的文件是否属于当前步骤要做的内容？
- 是否跳到了 future step？是否倒退回了 previous step？
- commit 信息描述的工作内容是否与当前步骤一致？

**实质性检查**：用 \`git diff\` 审查本轮改动的实质内容：
- 是否在做 task 相关的工作，而不是做无关的整理、格式化、代码美化？
- 改动是否有实质内容，还是只改了空格、注释、import 顺序等表面内容？
- 是否在用 refactor/cleanup 等名义做 task doc 里没有要求的额外工作？

**发现漂移**：如果本轮 commit 的内容与当前步骤无关，或是无关的整理美化工作 → 瞎搞，reset。

### 第二优先级：流程合规检查

#### 工作区 + 提交检查
用 \`git status\` 判断实际状态：

- **工作区干净 + 有最近 commit**：prompt1 已完成工作，正常结束，下一轮继续。
- **工作区干净 + 无 commit**：检查 \`${TASK_DOC_ABS}\` 末尾是否有本轮进度记录：
  - **有进度记录**：本轮跳过但有说明，正常结束。
  - **无进度记录**：prompt1 什么都没做，reset，让下一轮重做。
- **工作区有未提交变更 + 有 commit**：prompt1 已完成但漏删了 current_task_actions.md，补删文件即可，正常结束。
- **工作区有未提交变更 + 无 commit**：这是 prompt1 中断留下的正常断点状态。**不做任何操作，直接结束**，让下一轮继续完成它。

#### 进度记录检查
- 是否在 \`${TASK_DOC_ABS}\` 末尾追加了本次工作进度记录？（跳过本轮的情况除外）

## 处理方式

- **无漂移 + 流程合规**：本轮完整正确。直接结束，不要做任何额外操作。
- **有漂移**（跳步、缩水、做了无关工作）：用 git reset --hard 丢弃，回到干净状态，让下一轮重做。不要救场，不要合理化。
- **无漂移 + 有格式问题**（漏删文件、commit message 格式不规范等）：直接修正，然后结束。
- **断点状态**（文件存在、无 commit）：不做任何操作，直接结束，下一轮继续。

## 不要做的事
- 不要分析代码逻辑的对错或实现质量。
- 无漂移时不要 reset；漂移了不要救场。
- 断点状态不要救场，直接结束让下一轮继续。
- 全部通过时不要做任何操作，直接结束即可。"

PROMPTS=(
  "${PROMPT_ROUND_1}"
  "${PROMPT_ROUND_2}"
)

command_exists() {
  command -v "$1" >/dev/null 2>&1
}

terminate_child() {
  local pid="${child_pid:-}"
  if [[ -z "${pid}" ]]; then
    return
  fi

  if kill -0 "${pid}" 2>/dev/null; then
    if command_exists pkill; then
      pkill -TERM -P "${pid}" 2>/dev/null || true
    fi
    kill -TERM "${pid}" 2>/dev/null || true
  fi

  child_pid=""
}

on_interrupt() {
  interrupted=1
  echo
  echo "Interrupted, stopping current child process..."
  terminate_child
}

cleanup() {
  terminate_child
}

trap on_interrupt INT TERM
trap cleanup EXIT

run_foreground_command() {
  if command_exists setsid; then
    setsid "$@" &
  else
    "$@" &
  fi
  child_pid=$!

  set +e
  wait "${child_pid}"
  local status=$?
  set -e

  child_pid=""
  return "${status}"
}

sleep_with_interrupt() {
  local seconds="$1"
  run_foreground_command sleep "${seconds}"
}

for ((round = 1; round <= MAX_ROUNDS; round++)); do
  if [[ "${interrupted}" -ne 0 ]]; then
    echo "Interrupted before round ${round}, exiting."
    exit 130
  fi

  echo "===== Round ${round}/${MAX_ROUNDS}: task ${TASK_DOC_ABS} (${#PROMPTS[@]} prompts) ====="
  claude_failed=0

  for ((i = 0; i < ${#PROMPTS[@]}; i++)); do
    if [[ "${interrupted}" -ne 0 ]]; then
      echo "Interrupted during round ${round}, exiting."
      exit 130
    fi

    prompt_idx=$((i + 1))
    prompt="${PROMPTS[$i]}"
    # Prompt 2 needs to know which round it is auditing
    if [[ "${i}" -eq 1 ]]; then
      prompt="${prompt//__ROUND_NUM__/${round}}"
    fi

    if [[ -n "${CLAUDE_LOOP_TEST_CMD}" ]]; then
      claude_cmd=(bash -lc "${CLAUDE_LOOP_TEST_CMD}")
    else
      claude_cmd=(claude -p --dangerously-skip-permissions "${prompt}")
    fi

    echo "----- Round ${round}: prompt ${prompt_idx}/${#PROMPTS[@]} -----"
    if run_foreground_command "${claude_cmd[@]}"; then
      claude_status=0
    else
      claude_status=$?
    fi
    echo "claude exit code: ${claude_status}"

    if [[ "${interrupted}" -ne 0 ]]; then
      echo "Interrupted while claude was running, exiting."
      exit 130
    fi

    if [[ "${claude_status}" -ne 0 ]]; then
      claude_failed=1
      break
    fi
  done

  if [[ "${claude_failed}" -ne 0 ]]; then
    echo "claude failed in round ${round}; sleeping 5 minutes (${BACKOFF_SLEEP_SECONDS}s) before retry."
    if ! sleep_with_interrupt "${BACKOFF_SLEEP_SECONDS}"; then
      if [[ "${interrupted}" -ne 0 ]]; then
        echo "Interrupted during backoff sleep, exiting."
        exit 130
      fi
    fi
    continue
  fi

  echo "Round ${round} finished; continuing."
done

echo "Reached max rounds (${MAX_ROUNDS})."
exit 0

#!/usr/bin/env bash

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
cd "${REPO_ROOT}"

child_pid=""
interrupted=0
CLAUDE_LOOP_TEST_CMD="${CLAUDE_LOOP_TEST_CMD:-}"
BACKOFF_SLEEP_SECONDS="${BACKOFF_SLEEP_SECONDS:-10}"

TASK_DOC_PATH="${1:-}"
MAX_ROUNDS="${2:-10}"

# Track consecutive no-progress rounds (decompose trigger: >= 2)
NO_PROGRESS_ROUNDS=0

# Duration threshold in seconds (5 minutes)
STUCK_MIN_DURATION_SECONDS=300

usage() {
  cat <<'EOF'
Usage:
  scripts/ralph-task-loop.sh TASK_DOC_PATH [rounds>=1]

Behavior:
  - Run a Ralph-style loop toward the given task document.
  - Each round sends exactly two independent prompts to Claude (two separate `claude -p` calls, no conversation continuity).
  - After 2 consecutive failed rounds that each took >= 5 minutes, a single "task decomposition" prompt runs instead:
    it identifies the stuck task and breaks it into smaller, actionable sub-steps.
  - Quick failures (< 5 minutes) are not counted as stuck (likely token/parse errors, not thinking failures).

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

## 退出
阅读 \`${TASK_DOC_ABS}\`，如果发现所有步骤都已完成，最后一行输出 **退出**，表示本轮为最后一轮工作。

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

## 退出

如果 prompt1 最后一行输出了"退出"：
1. 读取 \`${TASK_DOC_ABS}\` 确认是否所有步骤真的已完成。
2. 确认无剩余工作后，最后一行也输出 **退出**，表示审查通过，大循环可以结束。
3. 如果还有未完成的步骤，不输出退出，让大循环继续。

## 不要做的事
- 不要分析代码逻辑的对错或实现质量。
- 无漂移时不要 reset；漂移了不要救场。
- 断点状态不要救场，直接结束让下一轮继续。
- 全部通过时不要做任何操作，直接结束即可。"

PROMPT_DECOMPOSE="当前时间：${CURRENT_TIME}。

## 角色
你是任务分解专家。大循环已经连续 2 轮在同一个任务上没有任何实质性进展。

## 任务
识别卡点，将当前任务分解为更小、更可操作的步骤，然后提交你对任务文档的修改。

## 当前任务文档
读取 \`${TASK_DOC_ABS}\`，找到当前正在进行的步骤（Status: pending 或刚标记为进行中的步骤）。

## 分析工作区状态
用 \`git status\` 和 \`git diff\` 查看工作区的未提交变更。

**工作区必须完全干净**：先用 \`git reset --hard\` 丢弃所有未提交的变更（不需要 stash，Ralph loop 不会丢失有用的中间状态）。然后只改动任务文档 \`${TASK_DOC_ABS}\`，改完后 commit。

## 分解要求

**分析卡点原因**：
- 为什么连续 2 轮没有进展？列出可能的原因：
  1. 步骤本身定义不够具体？
  2. 依赖未满足（需要先完成其他前置工作）？
  3. 实现路径不清晰，agent 不知道从哪里下手？
  4. 步骤粒度太大，一轮做不完？

**将当前步骤拆解为 2-4 个更小的子步骤**：
- 每个子步骤必须能在 1-2 轮内完成
- 每个子步骤有明确的验收标准（什么算"完成"）
- 子步骤之间如果有依赖关系，明确标出

**更新任务文档（重要格式要求）**：
在当前卡住的步骤位置原地替换，用编号表示子步骤关系：
- 原来 3.2 卡住 → 原地拆成 3.2.1、3.2.2、3.2.3……，删除原来的 3.2 内容
- 原来 3.2.2 卡住 → 原地拆成 3.2.2.1、3.2.2.2、3.2.2.3……，删除原来的 3.2.2
- 以此类推，子步骤编号末尾数字递增（3.2.1.3 卡住 → 3.2.1.3.1、3.2.1.3.2……）

每个子步骤必须：
- 有标题和验收标准
- 能在 1-2 轮内完成
- 子步骤之间如有依赖关系，明确标出

在进度记录区（文档末尾）追加本轮分解记录：
- 原来卡在哪个步骤
- 分析的卡点原因

## 提交
只提交对任务文档 \`${TASK_DOC_ABS}\` 的修改。**此文件永远不会进入 git 提交**：\`${REPO_ROOT}/current_task_actions.md\`。

## 绝对禁止
- 不要修改与当前步骤无关的其他步骤
- 不要做代码实现，只做任务分解
- 不要 git push
"

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

# Run command, capture output to file, return exit code.
# Sets global LAST_OUTPUT_FILE with the temp file path.
run_and_capture() {
  local output_file
  output_file="$(mktemp)"
  LAST_OUTPUT_FILE="${output_file}"
  if command_exists setsid; then
    setsid "$@" > "${output_file}" 2>&1 &
  else
    "$@" > "${output_file}" 2>&1 &
  fi
  child_pid=$!

  set +e
  wait "${child_pid}"
  local status=$?
  set -e

  child_pid=""
  LAST_STATUS="${status}"
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

  echo "===== Round ${round}/${MAX_ROUNDS}: task ${TASK_DOC_ABS} ====="
  ROUND_START_SECONDS="${SECONDS}"

  # If stuck for 2+ rounds, run decompose first before normal P1+P2
  if [[ "${NO_PROGRESS_ROUNDS}" -ge 2 ]]; then
    echo "----- Running decompose (${NO_PROGRESS_ROUNDS} no-progress rounds) -----"

    while true; do
      if [[ "${interrupted}" -ne 0 ]]; then
        echo "Interrupted during decompose, exiting."
        exit 130
      fi

      if [[ -n "${CLAUDE_LOOP_TEST_CMD}" ]]; then
        claude_cmd=(bash -lc "${CLAUDE_LOOP_TEST_CMD}")
        if run_foreground_command "${claude_cmd[@]}"; then
          echo "Decompose (test mode) succeeded."
          NO_PROGRESS_ROUNDS=0
          break
        else
          echo "Decompose (test mode) failed; sleeping ${BACKOFF_SLEEP_SECONDS}s before retry."
          if ! sleep_with_interrupt "${BACKOFF_SLEEP_SECONDS}"; then
            [[ "${interrupted}" -ne 0 ]] && { echo "Interrupted during backoff sleep, exiting."; exit 130; }
          fi
        fi
      else
        prompt="${PROMPT_DECOMPOSE}"
        claude_cmd=(claude -p --dangerously-skip-permissions "${prompt}")
        run_and_capture "${claude_cmd[@]}"
        claude_status="${LAST_STATUS}"
        echo "claude exit code: ${claude_status}"
        if [[ "${claude_status}" -eq 0 ]]; then
          echo "Decompose succeeded."
          NO_PROGRESS_ROUNDS=0
          break
        fi
        echo "Decompose failed (exit ${claude_status}); sleeping ${BACKOFF_SLEEP_SECONDS}s before retry."
        if ! sleep_with_interrupt "${BACKOFF_SLEEP_SECONDS}"; then
          [[ "${interrupted}" -ne 0 ]] && { echo "Interrupted during backoff sleep, exiting."; exit 130; }
        fi
      fi
    done
    # After decompose succeeds, fall through to normal P1+P2 with counter reset
  fi

  # Normal P1+P2 loop
  claude_failed=0
  p2_said_exit=0

  for ((i = 0; i < ${#PROMPTS[@]}; i++)); do
    if [[ "${interrupted}" -ne 0 ]]; then
      echo "Interrupted during round ${round}, exiting."
      exit 130
    fi

    prompt_idx=$((i + 1))
    prompt="${PROMPTS[$i]}"
    if [[ "${i}" -eq 1 ]]; then
      prompt="${prompt//__ROUND_NUM__/${round}}"
    fi

    if [[ -n "${CLAUDE_LOOP_TEST_CMD}" ]]; then
      claude_cmd=(bash -lc "${CLAUDE_LOOP_TEST_CMD}")
      echo "----- Round ${round}: prompt ${prompt_idx}/${#PROMPTS[@]} -----"
      if run_foreground_command "${claude_cmd[@]}"; then
        claude_status=0
      else
        claude_status=$?
      fi
      echo "claude exit code: ${claude_status}"
    else
      claude_cmd=(claude -p --dangerously-skip-permissions "${prompt}")
      echo "----- Round ${round}: prompt ${prompt_idx}/${#PROMPTS[@]} -----"
      run_and_capture "${claude_cmd[@]}"
      claude_status="${LAST_STATUS}"
      echo "claude exit code: ${claude_status}"
      echo "--- output (last 5 lines) ---"
      tail -5 "${LAST_OUTPUT_FILE}" || true
      echo "--------------------------------"
      last_line="$(tail -1 "${LAST_OUTPUT_FILE}" 2>/dev/null || echo "")"
      if [[ "${last_line}" == "退出" ]]; then
        echo "Detected '退出' from prompt ${prompt_idx}"
        if [[ "${i}" -eq 0 ]]; then
          echo "P1 requested exit; running P2 to verify..."
        elif [[ "${i}" -eq 1 ]]; then
          echo "P2 confirmed exit; both prompts agree. Exiting loop."
          p2_said_exit=1
        fi
      fi
    fi

    if [[ "${interrupted}" -ne 0 ]]; then
      echo "Interrupted while claude was running, exiting."
      exit 130
    fi

    if [[ "${claude_status}" -ne 0 ]]; then
      claude_failed=1
      break
    fi
  done

  # Exit if P2 confirmed
  if [[ "${p2_said_exit}" -ne 0 ]]; then
    echo "Loop ended by mutual exit agreement."
    exit 0
  fi

  # Retry on failure: only count as stuck if it took long enough
  if [[ "${claude_failed}" -ne 0 ]]; then
    duration=$((SECONDS - ROUND_START_SECONDS))
    minutes=$((duration / 60))
    seconds=$((duration % 60))
    if [[ "${duration}" -ge "${STUCK_MIN_DURATION_SECONDS}" ]]; then
      NO_PROGRESS_ROUNDS=$((NO_PROGRESS_ROUNDS + 1))
      echo "claude failed in round ${round} (stuck: ${NO_PROGRESS_ROUNDS}/2, duration: ${minutes}m${seconds}s); sleeping ${BACKOFF_SLEEP_SECONDS}s before retry."
    else
      echo "claude failed in round ${round} but only took ${minutes}m${seconds}s (< 5m), not counting as stuck; sleeping ${BACKOFF_SLEEP_SECONDS}s before retry."
    fi
    if ! sleep_with_interrupt "${BACKOFF_SLEEP_SECONDS}"; then
      [[ "${interrupted}" -ne 0 ]] && { echo "Interrupted during backoff sleep, exiting."; exit 130; }
    fi
    continue
  fi

  # Success: reset counter
  NO_PROGRESS_ROUNDS=0
  echo "Round ${round} finished; continuing."
done

echo "Reached max rounds (${MAX_ROUNDS})."
exit 0


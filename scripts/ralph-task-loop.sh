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
  - Each round sends exactly two prompts to Claude in the same conversation.
  - Prompt 1: push the next concrete step toward the target.
  - Prompt 2: force a hard self-check on whether the step is complete or harmful.

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

## 工作区状态
- 如果工作区干净：直接开始本轮任务。
- 如果有未提交变更：用 git diff 和 git status 仔细评估：这些改动是否能接上本次任务？是部分有用还是完全无用？
  - **部分有用**：保留有用部分，commit 或 stash，继续在此基础上推进。
  - **完全无用或冲突**：git reset --hard 丢弃，回到干净状态。
- 如果本轮改动涉及 crates/ 或会影响 crate 行为/接口/测试结果/编译流程，必须同步更新 IMPLEMENTATION.md，并在本轮结束前跑通 make gate。

## 提交
任务有实质进展就 git commit，不要等到最后一起交。

## 进度记录
本轮任务执行完成后，把本次工作进度追加到 \`${TASK_DOC_ABS}\` 末尾。包括：当前步骤标题、本次做了什么、下一步方向。

## 工具
优先用 TODO 工具分配任务给 subagent，分担上下文压力。"

PROMPT_ROUND_2="当前时间：${CURRENT_TIME}。

## 审计
这是第二轮。你是一个严厉的 reviewer，必须指出 Round 1 的问题。

检查点：
- **git commit 规范吗**：commit 信息能清晰描述本次做了什么吗？还是"提交了个寂寞"？
- **工作有实质内容吗**：本轮是否真的推进了任务，还是做了无关整理、语法美化、重复造轮子？
- **任务真的完成了吗**：Round 1 设定的目标达成了吗？还是跳步了、缩水了、或根本没做？

处理方式：
- 如果以上都 OK：在 Round 1 的进度记录末尾追加"[自测通过]"，本轮结束。
- 如果有问题：直接用 git reset --soft / commit --amend / rebase -i 等工具纠正，不要等到下一轮。
- 如果本轮完全无意义：用 git reset --hard 丢弃，回到干净状态。

## 不要合理化
不要为 Round 1 的问题找借口。不要说\"差不多完成了\"。如果有问题，就纠正。"

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

    if [[ -n "${CLAUDE_LOOP_TEST_CMD}" ]]; then
      claude_cmd=(bash -lc "${CLAUDE_LOOP_TEST_CMD}")
    elif [[ "${i}" -eq 0 ]]; then
      claude_cmd=(claude -p --dangerously-skip-permissions "${prompt}")
    else
      claude_cmd=(claude -p -c --dangerously-skip-permissions "${prompt}")
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

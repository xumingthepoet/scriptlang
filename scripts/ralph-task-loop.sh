#!/usr/bin/env bash

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
cd "${REPO_ROOT}"

child_pid=""
interrupted=0
CLAUDE_LOOP_TEST_CMD="${CLAUDE_LOOP_TEST_CMD:-}"
BACKOFF_SLEEP_SECONDS="${BACKOFF_SLEEP_SECONDS:-600}"

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

PROMPT_ROUND_1="首先，reset当前修改让工作目录保持干净。阅读任务文档 \`${TASK_DOC_ABS}\`，把它作为唯一目标，以 ralph 模式持续推进。你在一个循环里工作：每一轮都按文档里的当前进度执行下一条任务，直接动手，不要跳步，不要空转，不要偏离任务去做无关整理。遵守仓库里的 AGENTS.md；如果本轮改动涉及 crates/ 或会影响 crate 行为、公共接口、测试结果、编译流程，就同步更新 IMPLEMENTATION.md，并在准备把这一轮视为完成前跑通 make gate。如果本次任务完成就可以git提交了. 如果本次任务实在太复杂，运行超过20分钟，可以尽快选择干净合适工作节点暂停工作，在目标文档里写清楚当前任务节点，指导后续循环接上继续工作，然后git提交本次的部分进展。"
PROMPT_ROUND_2="当前修改完成了既定步骤的任务吗？还是让事情更糟糕了？如果完成了，请把任务进度更新追加到任务文档 \`${TASK_DOC_ABS}\` 的最后，方便后续工作继续，并提交一次当前代码；如果让事情更糟糕，不要犹豫，reset当前修改让工作目录保持干净。"

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
    echo "claude failed in round ${round}; sleeping 10 minutes before retry."
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

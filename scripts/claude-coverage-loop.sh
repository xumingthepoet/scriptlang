#!/usr/bin/env bash

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
cd "${REPO_ROOT}"

child_pid=""
interrupted=0
CLAUDE_LOOP_TEST_CMD="${CLAUDE_LOOP_TEST_CMD:-}"
BACKOFF_SLEEP_SECONDS="${BACKOFF_SLEEP_SECONDS:-600}"

MAX_ROUNDS="${1:-10}"
PROMPT_ROUND_1="以通过 make gate 为目标推进当前项目。你在一个循环里工作：每一轮先专注解决一个最关键的 gate blocker，完成单点闭环后再看是否继续处理下一个。不要在多个问题之间来回切换，避免最后都没改好。优先用最小、可验证、符合仓库约束的改动通过 gate；如果问题在 coverage，就补真实测试或删除不可达代码，但不要为了通过而修改 gate 规则。"
PROMPT_ROUND_2="继续处理当前最关键的剩余 gate blocker，只做能直接推动 make gate 通过的改动，并在同一轮内完成实现、验证和必要测试。"
PROMPT_ROUND_3="继续收敛剩余问题，目标是 make gate 通过。不要追求额外指标，不要降低标准，也不要改 gate 规则。"

PROMPTS=(
  "$PROMPT_ROUND_1"
  "$PROMPT_ROUND_2"
  "$PROMPT_ROUND_3"
)

if ! [[ "$MAX_ROUNDS" =~ ^[0-9]+$ ]] || [[ "$MAX_ROUNDS" -lt 1 ]]; then
  echo "Usage: $0 [rounds>=1]"
  exit 1
fi

if [[ -z "$CLAUDE_LOOP_TEST_CMD" ]] && ! command -v claude >/dev/null 2>&1; then
  echo "claude CLI is required for scripts/claude-coverage-loop.sh" >&2
  exit 1
fi

command_exists() {
  command -v "$1" >/dev/null 2>&1
}

terminate_child() {
  local pid="${child_pid:-}"
  if [[ -z "$pid" ]]; then
    return
  fi

  if kill -0 "$pid" 2>/dev/null; then
    if command_exists pkill; then
      pkill -TERM -P "$pid" 2>/dev/null || true
    fi
    kill -TERM "$pid" 2>/dev/null || true
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
  wait "$child_pid"
  local status=$?
  set -e

  child_pid=""
  return "$status"
}

sleep_with_interrupt() {
  local seconds="$1"
  run_foreground_command sleep "$seconds"
}

for ((round = 1; round <= MAX_ROUNDS; round++)); do
  if [[ "$interrupted" -ne 0 ]]; then
    echo "Interrupted before round ${round}, exiting."
    exit 130
  fi

  echo "===== Round ${round}/${MAX_ROUNDS}: invoking claude (${#PROMPTS[@]} prompts) ====="
  claude_failed=0
  for ((i = 0; i < ${#PROMPTS[@]}; i++)); do
    if [[ "$interrupted" -ne 0 ]]; then
      echo "Interrupted during round ${round}, exiting."
      exit 130
    fi

    prompt_idx=$((i + 1))
    prompt="${PROMPTS[$i]}"
    if [[ -n "$CLAUDE_LOOP_TEST_CMD" ]]; then
      claude_cmd=(bash -lc "$CLAUDE_LOOP_TEST_CMD")
    elif [[ "$i" -eq 0 ]]; then
      claude_cmd=(claude -p --dangerously-skip-permissions "$prompt")
    else
      claude_cmd=(claude -p -c --dangerously-skip-permissions "$prompt")
    fi

    echo "----- Round ${round}: prompt ${prompt_idx}/${#PROMPTS[@]} -----"
    if run_foreground_command "${claude_cmd[@]}"; then
      claude_status=0
    else
      claude_status=$?
    fi
    echo "claude exit code: ${claude_status}"
    if [[ "$interrupted" -ne 0 ]]; then
      echo "Interrupted while claude was running, exiting."
      exit 130
    fi
    if [[ "$claude_status" -ne 0 ]]; then
      claude_failed=1
      break
    fi
  done

  if [[ "$claude_failed" -ne 0 ]]; then
    echo "claude failed in round ${round}; sleeping 10 minutes and skipping make gate for this round."
    if ! sleep_with_interrupt "$BACKOFF_SLEEP_SECONDS"; then
      if [[ "$interrupted" -ne 0 ]]; then
        echo "Interrupted during backoff sleep, exiting."
        exit 130
      fi
    fi
    continue
  fi

  echo "===== Round ${round}/${MAX_ROUNDS}: running make gate ====="
  if run_foreground_command make gate; then
    gate_status=0
  else
    gate_status=$?
  fi
  echo "make gate exit code: ${gate_status}"
  if [[ "$interrupted" -ne 0 ]]; then
    echo "Interrupted while make gate was running, exiting."
    exit 130
  fi

  if [[ "$gate_status" -eq 0 ]]; then
    echo "make gate succeeded; exiting loop."
    exit 0
  fi

  echo "make gate failed; continuing to next round."
done

echo "Reached max rounds (${MAX_ROUNDS}) without passing gate."
exit 1

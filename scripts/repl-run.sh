#!/usr/bin/env bash

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
DEFAULT_TARGET_PATH="${REPO_ROOT}/scripts/repl-target.xml"

usage() {
  cat <<'EOF'
Usage:
  scripts/repl-run.sh [PATH]

Behavior:
  - If PATH is omitted, run the hard-coded default XML file: scripts/repl-target.xml
  - Any target file is treated as a REPL-executable input file and passed to sl-repl --file.
  - XML files are not special-cased by root tag; <module>, <text>, <goto> and mixed top-level inputs
    are all interpreted under the same REPL file semantics.

Examples:
  scripts/repl-run.sh
  scripts/repl-run.sh scripts/repl-target.xml
  scripts/repl-run.sh path/to/any-repl-input.xml
EOF
}

target_path=""

while [[ $# -gt 0 ]]; do
  case "$1" in
    -h|--help)
      usage
      exit 0
      ;;
    -*)
      echo "error: unknown option: $1" >&2
      usage >&2
      exit 1
      ;;
    *)
      if [[ -n "${target_path}" ]]; then
        echo "error: only one PATH is supported" >&2
        usage >&2
        exit 1
      fi
      target_path="$1"
      shift
      ;;
  esac
done

if [[ -z "${target_path}" ]]; then
  target_path="${DEFAULT_TARGET_PATH}"
fi

if [[ ! -f "${target_path}" ]]; then
  echo "error: file not found: ${target_path}" >&2
  exit 1
fi

target_path="$(cd "$(dirname "${target_path}")" && pwd)/$(basename "${target_path}")"

cd "${REPO_ROOT}"
cargo run -p sl-repl -- --file "${target_path}"

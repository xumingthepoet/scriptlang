#!/usr/bin/env bash

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
DEFAULT_TARGET_PATH="${REPO_ROOT}/scripts/repl-target.xml"

usage() {
  cat <<'EOF'
Usage:
  scripts/repl-run.sh [--entry MODULE.SCRIPT] [PATH]

Behavior:
  - If PATH is omitted, run the hard-coded default XML file: scripts/repl-target.xml
  - If PATH ends with .xml, load PATH's parent directory in sl-repl and execute the entry script.
  - Otherwise, treat PATH as a repl transcript file and pass it to sl-repl --file.

Examples:
  scripts/repl-run.sh
  scripts/repl-run.sh crates/sl-integration-tests/examples/01-linear/xml/main.xml
  scripts/repl-run.sh --entry other.endcap crates/sl-integration-tests/examples/04-cross-module-goto/xml/main.xml
  scripts/repl-run.sh path/to/session.repl
EOF
}

entry_script="main.main"
target_path=""

while [[ $# -gt 0 ]]; do
  case "$1" in
    --entry)
      if [[ $# -lt 2 ]]; then
        echo "error: --entry requires MODULE.SCRIPT" >&2
        usage >&2
        exit 1
      fi
      entry_script="$2"
      shift 2
      ;;
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

if [[ "${target_path}" == *.xml ]]; then
  if grep -Eq '^[[:space:]]*<module(\s|>)' "${target_path}"; then
    if [[ "${entry_script}" != *.* ]]; then
      echo "error: --entry must be a qualified MODULE.SCRIPT reference" >&2
      exit 1
    fi

    xml_dir="$(cd "$(dirname "${target_path}")" && pwd)"
    module_name="${entry_script%.*}"
    cargo run -p sl-repl -- \
      --command ":load ${xml_dir}" \
      --command "<import name=\"${module_name}\"/>" \
      --command "<goto script=\"@${entry_script}\"/>"
  else
    cargo run -p sl-repl -- --file "${target_path}"
  fi
else
  cargo run -p sl-repl -- --file "${target_path}"
fi

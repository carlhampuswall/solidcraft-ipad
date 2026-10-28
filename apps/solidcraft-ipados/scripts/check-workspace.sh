#!/bin/sh
# Deterministic native presentation checks. Never launches an app or simulator.
set -eu
script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
check_dir=$(mktemp -d "${TMPDIR:-/tmp}/solidcraft-workspace-check.XXXXXX")
trap 'rm -rf "$check_dir"' EXIT HUP INT TERM
xcrun --sdk macosx swiftc \
  -module-cache-path "$check_dir/module-cache" \
  "$script_dir/../SolidCraft/WorkspaceLayout.swift" \
  "$script_dir/workspace-check.swift" \
  -o "$check_dir/workspace-check"
"$check_dir/workspace-check"

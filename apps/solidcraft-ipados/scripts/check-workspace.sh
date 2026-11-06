#!/bin/sh
# Deterministic native presentation checks. Never launches an app or simulator.
set -eu
script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
check_dir=$(mktemp -d "${TMPDIR:-/tmp}/solidcraft-workspace-check.XXXXXX")
trap 'rm -rf "$check_dir"' EXIT HUP INT TERM
xcrun --sdk macosx swiftc \
  -module-cache-path "$check_dir/module-cache" \
  "$script_dir/../SolidCraft/WorkspaceLayout.swift" \
  "$script_dir/../SolidCraft/QuickAccessLayout.swift" \
  "$script_dir/../SolidCraft/BrowserSwipeActions.swift" \
  "$script_dir/../SolidCraft/ModellingFormPresentation.swift" \
  "$script_dir/../SolidCraft/ProjectLibrary.swift" \
  "$script_dir/workspace-check.swift" \
  -o "$check_dir/workspace-check"
"$check_dir/workspace-check"
# Use an installed Python 3.11+ for the canonical TOML icon palette, scoped to
# this command. No global Python setting or runtime dependency is changed.
PATH="/opt/homebrew/bin:/usr/local/bin:$PATH" python3 -B "$script_dir/generate-icons.py" --check "$script_dir/../../.."

#!/usr/bin/env python3
"""Check the resolved normal-dependency boundary; feed Cargo metadata on stdin.

Usage (from the repository root):
    cargo +1.95.0 metadata --locked --format-version 1 \
        --filter-platform aarch64-apple-ios | python3 crates/ipados/check-dependencies.py

This is a repository validation tool, never an application dependency. Cargo's resolved
package IDs handle renamed dependencies and multiple versions without name guessing.
"""

import json
from pathlib import Path
import sys


def main() -> None:
    metadata = json.load(sys.stdin)
    root = Path(metadata["workspace_root"])
    packages = {package["id"]: package for package in metadata["packages"]}
    resolve = metadata.get("resolve")
    if resolve is None:
        raise SystemExit("dependency boundary FAILED: metadata needs the resolved graph (omit --no-deps)")
    nodes = {node["id"]: node for node in resolve["nodes"]}

    def normal_dependencies(package_id: str) -> list[str]:
        return [
            dependency["pkg"]
            for dependency in nodes[package_id]["deps"]
            if any(kind["kind"] is None for kind in dependency["dep_kinds"])
        ]

    failures = []
    shared = []
    for package_id in metadata["workspace_members"]:
        package = packages[package_id]
        manifest = Path(package["manifest_path"])
        if not manifest.is_relative_to(root / "crates") or manifest.is_relative_to(root / "crates" / "ipados"):
            continue
        shared.append(package_id)
        # Inspect declarations too, including inactive target-specific and optional edges.
        for dependency in package["dependencies"]:
            if dependency["kind"] is None and dependency["name"].startswith("solidcraft-ipados-"):
                failures.append(f"{package['name']} depends on platform adapter {dependency['name']}")

    protected = {"solidcraft-geom", "solidcraft-sketch", "solidcraft-kernel", "solidcraft-render", "solidcraft-doc", "solidcraft-io", "solidcraft-engine"}
    forbidden = {"cocoa", "cocoa-foundation", "eframe", "egui", "metal", "objc", "objc2", "rfd", "wgpu", "winit"}
    checked = set()
    pending = [package_id for package_id in shared if packages[package_id]["name"] in protected]
    while pending:
        package_id = pending.pop()
        if package_id in checked:
            continue
        checked.add(package_id)
        name = packages[package_id]["name"]
        if name in forbidden or name.startswith(("objc2-", "solidcraft-ipados-")):
            failures.append(f"shared CAD dependency graph reaches platform/frontend package {name}")
        pending.extend(normal_dependencies(package_id))

    if failures:
        raise SystemExit("dependency boundary FAILED:\n" + "\n".join(sorted(set(failures))))
    print(f"dependency boundary passed: {len(shared)} shared crates, {len(checked)} resolved CAD packages")


if __name__ == "__main__":
    main()

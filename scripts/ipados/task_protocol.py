#!/usr/bin/env python3
"""Shared-checkout batch checks and exclusive physical iPad sessions; no build queue."""

import argparse
from contextlib import contextmanager
from datetime import datetime, timezone
import fcntl
import hashlib
import json
import os
from pathlib import Path
import re
import shlex
import subprocess
import sys
import tempfile
import time


class ProtocolError(Exception):
    pass


def now():
    return datetime.now(timezone.utc).isoformat()


def atomic_json(path, value):
    path.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.NamedTemporaryFile(mode="w", dir=path.parent, delete=False) as stream:
        temporary = Path(stream.name)
        json.dump(value, stream, indent=2)
        stream.write("\n")
    try:
        os.replace(temporary, path)
    finally:
        temporary.unlink(missing_ok=True)


def slug(value):
    if not re.fullmatch(r"[a-z][a-z0-9]*(?:-[a-z0-9]+)*", value):
        raise argparse.ArgumentTypeError("Use a lowercase task name with optional hyphens.")
    return value


def check_plan(paths, full=False):
    """Select checks for a finished batch; shared or unfamiliar code retains full CI."""
    def routine(path):
        return path.endswith(".md") or path.startswith((
            "docs/", "skills/", "scripts/ipados/", "apps/solidcraft-ipados/",
            "crates/ipados/", "assets/", "attribution/pending/"))

    if full or any(not routine(path) for path in paths):
        return "full", [["cargo", "+1.95.0", "xtask", "ci"]]
    commands = []
    if any(path.endswith(".rs") for path in paths):
        commands.append(["cargo", "+1.95.0", "fmt", "--all", "--", "--check"])
    if any(Path(path).name == "AGENTS.md" or path.startswith("skills/") for path in paths):
        commands.append(["cargo", "+1.95.0", "xtask", "skills"])
    if any(path.startswith(("assets/", "attribution/")) for path in paths):
        commands += [["cargo", "+1.95.0", "xtask", name] for name in ("assets", "icons", "lists")]
    if any(path in ("scripts/ipados/task_protocol.py", "scripts/ipados/test_task_protocol.py") for path in paths):
        commands.append([sys.executable, "-I", "scripts/ipados/test_task_protocol.py"])
    rust = any(path.startswith("crates/ipados/") and not path.endswith(".md") for path in paths)
    native = rust or any(
        (path.startswith("apps/solidcraft-ipados/") and not path.endswith(".md"))
        or path.startswith("assets/locales/pending/ipados-") for path in paths)
    if rust:
        packages = ["-p", "solidcraft-ipados-session", "-p", "solidcraft-ipados-bridge"]
        commands += [
            ["cargo", "+1.95.0", "test", "--locked", "--profile", "ci", *packages, "--lib"],
            ["cargo", "+1.95.0", "clippy", "--locked", *packages, "--all-targets", "--no-deps", "--", "-D", "warnings"],
        ]
    if native:
        commands += [["sh", "apps/solidcraft-ipados/scripts/build.sh"],
                     ["sh", "apps/solidcraft-ipados/scripts/check-workspace.sh"]]
    return "affected", commands


class Repository:
    def __init__(self, root="."):
        self.root = Path(root).resolve()
        self.root = Path(self.git("rev-parse", "--show-toplevel").strip())
        self.common = Path(self.git("rev-parse", "--path-format=absolute", "--git-common-dir").strip())
        # Preserve the existing iPad lock inode, including for older checkouts.
        self.state = self.common / "solidcraft-tasks"

    def git(self, *args):
        result = subprocess.run(["git", "-C", str(self.root), *args], text=True, capture_output=True)
        if result.returncode:
            raise ProtocolError(result.stderr.strip() or result.stdout.strip())
        return result.stdout

    def sha(self, revision="HEAD"):
        return self.git("rev-parse", "--verify", "--end-of-options", revision + "^{commit}").strip()

    def shared_checkout(self):
        primary = self.git("worktree", "list", "--porcelain", "-z").split("\0")[0]
        branch = self.git("symbolic-ref", "--quiet", "--short", "HEAD").strip()
        if primary != "worktree " + str(self.root) or branch != "develop":
            raise ProtocolError("Use the primary shared checkout on develop; do not switch another task's branch.")
        for name in ("MERGE_HEAD", "CHERRY_PICK_HEAD", "REVERT_HEAD", "rebase-merge", "rebase-apply"):
            path = Path(self.git("rev-parse", "--path-format=absolute", "--git-path", name).strip())
            if path.exists():
                raise ProtocolError(f"Finish or explicitly abandon the existing Git operation: {path}")

    def fingerprint(self):
        """Hash working files without writing the index; a commit alone doesn't invalidate tests."""
        digest = hashlib.sha256()
        names = self.git("ls-files", "--cached", "--others", "--exclude-standard", "-z").split("\0")
        for name in sorted(set(names) - {""}):
            path = self.root / name
            digest.update(name.encode() + b"\0")
            if path.is_symlink():
                content = b"link\0" + os.readlink(path).encode()
            elif path.is_file():
                content = b"file\0" + str(path.stat().st_mode & 0o111).encode() + b"\0" + path.read_bytes()
            elif path.exists():
                raise ProtocolError(f"Cannot verify non-file input: {name}")
            else:
                content = b"deleted\0"
            digest.update(hashlib.sha256(content).digest())
        return digest.hexdigest()

    def check(self, args):
        self.shared_checkout()
        base = self.sha(args.base)
        paths = args.paths
        if paths is None:
            paths = self.git("diff", "--name-only", "-z", base, "--").split("\0")
            paths += self.git("ls-files", "--others", "--exclude-standard", "-z").split("\0")
        paths = sorted(set(paths) - {""})
        for path in paths:
            if Path(path).is_absolute() or ".." in Path(path).parts or path.startswith(":"):
                raise ProtocolError("Check paths must be literal repository-relative files.")
        if not paths and not args.full and not args.check:
            print("No changed paths; no tests run. Use --base or --paths for an already committed batch.")
            return
        self.git("--literal-pathspecs", "diff", "--check", base, "--", *paths)
        scope, commands = check_plan(paths, args.full)
        commands += [shlex.split(c) for c in args.check]
        if any(not command for command in commands):
            raise ProtocolError("Empty check command.")
        before = self.fingerprint()
        environment = dict(os.environ)
        environment.setdefault("SOLIDCRAFT_TEST_JOBS", "2")
        environment.setdefault("CARGO_BUILD_JOBS", "2")
        key = hashlib.sha256(json.dumps([before, paths, commands, sorted(environment.items())]).encode()).hexdigest()
        receipt = self.state / "checks" / (key + ".json")
        if receipt.exists() and not args.force and not args.full and json.loads(receipt.read_text()).get("passed"):
            print("Reusing passing checks for identical working files, commands and environment; no tests rerun.")
            return
        result = {"fingerprint": before, "paths": paths, "scope": scope, "passed": False, "checks": []}
        atomic_json(receipt, result)
        print(f"Validation scope: {scope}; {len(commands)} checks for this batch.", flush=True)
        for command in commands:
            completed = subprocess.run(command, cwd=self.root, env=environment)
            result["checks"].append({"command": command, "exit_code": completed.returncode, "at": now()})
            atomic_json(receipt, result)
            if completed.returncode:
                raise ProtocolError(f"Required check failed ({completed.returncode}): {shlex.join(command)}")
            self.shared_checkout()
            if self.fingerprint() != before:
                raise ProtocolError("Working files changed during verification; review and rerun affected checks.")
        self.shared_checkout()
        if self.fingerprint() != before:
            raise ProtocolError("Working files changed during verification; review and rerun affected checks.")
        result.update(passed=True, at=now())
        atomic_json(receipt, result)
        print("Batch checks passed. Review and commit only owned paths; no integration gate is needed.")

    @contextmanager
    def ipad_lock(self, task, wait):
        self.state.mkdir(parents=True, exist_ok=True)
        # Never unlink lock files: every contender must lock the same inode.
        with (self.state / "ipad.lock").open("a+") as stream:
            deadline = time.monotonic() + wait
            while True:
                try:
                    fcntl.flock(stream, fcntl.LOCK_EX | fcntl.LOCK_NB)
                    break
                except BlockingIOError:
                    if time.monotonic() >= deadline:
                        raise ProtocolError("iPad is busy; retry when the other session finishes.")
                    time.sleep(min(0.2, max(0, deadline - time.monotonic())))
            atomic_json(self.state / "ipad.owner.json",
                        {"task": task, "pid": os.getpid(), "worktree": str(self.root), "acquired": now()})
            # Closing our descriptor leaves an inherited child's reservation intact.
            yield stream.fileno()

    def ipad(self, args):
        self.shared_checkout()
        if self.git("status", "--porcelain", "--untracked-files=all").strip():
            raise ProtocolError("Prepare a clean committed device candidate; preserve other tasks' edits.")
        tip = self.sha()
        if args.candidate and self.sha(args.candidate) != tip:
            raise ProtocolError("The prepared device candidate must match the current shared checkout.")
        if bool(args.command) == args.hold:
            raise ProtocolError("Supply either --hold or a command after -- for the complete device session.")
        evidence = Path(args.evidence).resolve()
        if evidence.exists():
            raise ProtocolError("Use a fresh evidence path so an old result cannot count as this test.")
        with self.ipad_lock(args.task, args.wait) as fd:
            self.shared_checkout()
            if self.sha() != tip or self.git("status", "--porcelain", "--untracked-files=all").strip():
                raise ProtocolError("Device candidate changed while waiting for the iPad.")
            before = self.fingerprint()
            receipt = self.state / "device-sessions" / (args.task + ".json")
            atomic_json(receipt, {"candidate": tip, "device": args.device, "passed": False,
                                  "evidence": str(evidence), "at": now(), "manual": args.hold})
            print(f"IPAD RESERVED: task={args.task} device={args.device} candidate={tip}", flush=True)
            environment = dict(os.environ, SOLIDCRAFT_CANDIDATE=tip, SOLIDCRAFT_TASK=args.task,
                               SOLIDCRAFT_DEVICE=args.device)
            if args.hold:
                print("Keep this process open through install, test and evidence capture. Type pass or fail to release.", flush=True)
                passed = sys.stdin.readline().strip() == "pass"
            else:
                passed = subprocess.run(args.command, cwd=self.root, env=environment, pass_fds=(fd,)).returncode == 0
            self.shared_checkout()
            if self.sha() != tip or self.fingerprint() != before:
                raise ProtocolError("Device candidate changed; no successful test was recorded.")
            passed = passed and evidence.is_file() and evidence.stat().st_size > 0
            atomic_json(receipt,
                        {"candidate": tip, "device": args.device, "passed": passed,
                         "evidence": str(evidence), "at": now(), "manual": args.hold})
            if not passed:
                raise ProtocolError("Device session failed or has no new nonempty evidence file.")
            print("Device result recorded; iPad released.")

    def status(self):
        busy = False
        path = self.state / "ipad.lock"
        if path.exists():
            with path.open("r") as stream:
                try:
                    fcntl.flock(stream, fcntl.LOCK_EX | fcntl.LOCK_NB)
                    fcntl.flock(stream, fcntl.LOCK_UN)
                except BlockingIOError:
                    busy = True
        resource = {"busy": busy}
        owner = self.state / "ipad.owner.json"
        if busy and owner.exists():
            resource["owner"] = json.loads(owner.read_text())
        print(json.dumps({"workflow": "shared-checkout", "resources": {"ipad": resource}}, indent=2))


def parser():
    result = argparse.ArgumentParser(description=__doc__)
    sub = result.add_subparsers(dest="action", required=True)
    check = sub.add_parser("check", help="Validate a finished batch once; reuse unchanged passing results.")
    check.add_argument("--paths", nargs="+", help="Owned repository-relative files; default: all changed files.")
    check.add_argument("--base", default="HEAD", help="Compare working files with this committed batch baseline.")
    check.add_argument("--check", action="append", default=[], help="Extra command, parsed without a shell.")
    check.add_argument("--full", action="store_true", help="Always run full CI for main/releases.")
    check.add_argument("--force", action="store_true", help="Rerun despite identical passing results.")
    ipad = sub.add_parser("ipad", help="Reserve a complete physical device session.")
    ipad.add_argument("task", type=slug)
    ipad.add_argument("--candidate", help="Expected prepared commit; defaults to HEAD.")
    ipad.add_argument("--device", required=True)
    ipad.add_argument("--evidence", required=True)
    ipad.add_argument("--hold", action="store_true")
    ipad.add_argument("--wait", type=float, default=0, help="Wait at most 60 seconds.")
    sub.add_parser("status")
    return result


def main(argv=None):
    arguments = list(sys.argv[1:] if argv is None else argv)
    if arguments and arguments[0] in ("start", "adopt", "integrate", "finish", "note", "build"):
        raise ProtocolError("Task worktrees and the build queue are retired. Work directly in the primary develop checkout; "
                            "run builds directly and use check only for a finished batch.")
    separator = arguments.index("--") if "--" in arguments else len(arguments)
    args = parser().parse_args(arguments[:separator])
    if separator < len(arguments) and args.action != "ipad":
        raise ProtocolError("Only ipad accepts a command after --.")
    if args.action == "ipad":
        args.command = arguments[separator + 1:]
        if not 0 <= args.wait <= 60:
            raise ProtocolError("--wait must be between 0 and 60 seconds.")
    repo = Repository()
    if args.action == "status":
        repo.status()
    else:
        getattr(repo, args.action)(args)
    return 0


if __name__ == "__main__":
    try:
        sys.exit(main())
    except (ProtocolError, OSError, ValueError) as error:
        print(f"task-protocol: {error}", file=sys.stderr)
        sys.exit(1)
